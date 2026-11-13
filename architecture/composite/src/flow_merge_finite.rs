//! Prepared two-input payload merge. Each input retains its own finite lifetime.
use crate::prelude::*;
use conduit_core::{
    BorrowedStructuredValueValidator, KindId, PreparedStructuredValueValidator,
    StructuredInfoRefusal, StructuredInfoType, StructuredInfoTypeShape,
};
use conduit_kernel::{
    scheduler::{
        AssignedAbnormalTransduction, AssignedCancellationTransduction,
        AssignedNormalCloseTransduction, AssignedTerminalTransduction, StepBack, StepInputBytes,
        StepIo, StepOutcome,
    },
    Failure, FailureCode, PortId,
};

enum Validator {
    Owned(PreparedStructuredValueValidator),
    Static(BorrowedStructuredValueValidator<'static>),
}
impl Validator {
    fn validate(&self, bytes: &[u8]) -> Result<(), StructuredInfoRefusal> {
        match self {
            Self::Owned(validator) => validator.validate(bytes),
            Self::Static(validator) => validator.validate(bytes).map(|_| ()),
        }
    }
}

#[path = "flow_merge_finite/schema_input.rs"]
mod schema_input;

pub struct FlowMergeFiniteBack {
    validator: Option<Validator>,
    leaf: Option<KindId>,
    output: Vec<u8>,
    maximum_bytes: usize,
    #[cfg(feature = "execution-workspace")]
    workspace_output_bytes: u32,
    staged: Option<usize>,
    committed_close: [bool; 2],
    staged_close: Option<usize>,
    preferred: usize,
    staged_side: Option<usize>,
    cancelled: bool,
}
impl FlowMergeFiniteBack {
    /// Retain immutable linked schema bytes rather than a cloned inspection Type.
    /// The checked Type must match exactly; limits and runtime laws are unchanged.
    pub fn prepare_static(
        schema: &StructuredInfoType,
        canonical: &'static [u8],
        maximum_bytes: u32,
    ) -> Result<Self, StructuredInfoRefusal> {
        if schema.canonical_bytes()? != canonical {
            return Err(StructuredInfoRefusal::WrongType);
        }
        if matches!(schema.shape(), StructuredInfoTypeShape::Leaf(_)) {
            return Self::prepare(schema, maximum_bytes);
        }
        let validator = BorrowedStructuredValueValidator::new(canonical, maximum_bytes as usize)?;
        Ok(Self {
            maximum_bytes: crate::value_backing::bytes(schema, maximum_bytes),
            validator: Some(Validator::Static(validator)),
            leaf: None,
            output: {
                #[cfg(all(feature = "execution-workspace", feature = "schema-value-storage"))]
                {
                    Vec::new()
                }
                #[cfg(not(all(
                    feature = "execution-workspace",
                    feature = "schema-value-storage"
                )))]
                {
                    if cfg!(feature = "schema-value-storage") {
                        vec![0; crate::value_backing::bytes(schema, maximum_bytes)]
                    } else {
                        Vec::new()
                    }
                }
            },
            #[cfg(feature = "execution-workspace")]
            workspace_output_bytes: if cfg!(feature = "schema-value-storage") {
                crate::value_backing::bytes(schema, maximum_bytes) as u32
            } else {
                0
            },
            staged: None,
            committed_close: [false; 2],
            staged_close: None,
            preferred: 0,
            staged_side: None,
            cancelled: false,
        })
    }

    pub fn prepare(
        schema: &StructuredInfoType,
        maximum_bytes: u32,
    ) -> Result<Self, StructuredInfoRefusal> {
        Ok(Self {
            validator: if matches!(schema.shape(), StructuredInfoTypeShape::Leaf(_)) {
                None
            } else {
                Some(Validator::Owned(PreparedStructuredValueValidator::new(
                    schema,
                    maximum_bytes as usize,
                )?))
            },
            leaf: if let StructuredInfoTypeShape::Leaf(kind) = schema.shape() {
                Some(kind.clone())
            } else {
                None
            },
            output: Vec::new(),
            maximum_bytes: crate::value_backing::bytes(schema, maximum_bytes),
            #[cfg(feature = "execution-workspace")]
            workspace_output_bytes: 0,
            staged: None,
            committed_close: [false; 2],
            staged_close: None,
            preferred: 0,
            staged_side: None,
            cancelled: false,
        })
    }
}
impl<const PORTS: usize> StepBack<PORTS> for FlowMergeFiniteBack {
    #[cfg(feature = "schema-value-storage")]
    fn canonical_input_schema(&self, port: PortId) -> Option<&[u8]> {
        (port.0 < 2).then(|| self.linked_schema()).flatten()
    }
    #[cfg(feature = "schema-value-storage")]
    fn canonical_output_schema(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0)).then(|| self.linked_schema()).flatten()
    }
    fn terminal_transductions(&self) -> [Option<AssignedTerminalTransduction>; PORTS] {
        let mut contracts = [None; PORTS];
        for (side, contract) in contracts.iter_mut().enumerate().take(2) {
            *contract = Some(AssignedTerminalTransduction {
                input: PortId(side as u16),
                output: PortId(0),
                normal_close: AssignedNormalCloseTransduction::PropagateWhenAllClose,
                abnormal: AssignedAbnormalTransduction::NotAccepted,
                cancellation: AssignedCancellationTransduction::NotCancellable,
            });
        }
        contracts
    }
    #[cfg(feature = "execution-workspace")]
    fn workspace_requirements(
        &self,
    ) -> Option<conduit_kernel::scheduler::StepWorkspaceRequirements> {
        (self.workspace_output_bytes != 0).then_some(
            conduit_kernel::scheduler::StepWorkspaceRequirements {
                output_bytes: self.workspace_output_bytes,
                scratch_bytes: 0,
            },
        )
    }
    #[cfg(feature = "execution-workspace")]
    fn step_with_workspace(
        &mut self,
        io: &mut StepIo<PORTS>,
        inputs: &StepInputBytes<'_, PORTS>,
        workspace: &mut conduit_kernel::scheduler::StepWorkspace<'_>,
    ) -> StepOutcome {
        if self.workspace_output_bytes == 0 {
            return self.step_inner(io, inputs, None);
        }
        let (output, _) = workspace.buffers();
        self.step_inner(io, inputs, Some(output))
    }
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        self.step_inner(io, inputs, None)
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        if port != PortId(0) {
            return None;
        }
        self.staged.and_then(|len| self.output.get(..len))
    }
    fn step_committed(&mut self) {
        if let Some(side) = self.staged_close.take() {
            self.committed_close[side] = true;
        }
        if let Some(side) = self.staged_side.take() {
            self.preferred = 1 - side;
        }
        self.staged = None;
    }
    fn cancel(&mut self) {
        self.cancelled = true;
        self.staged = None;
        self.staged_side = None;
        self.staged_close = None;
    }
}
fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}
#[cfg(test)]
mod tests;

#[cfg(all(test, feature = "schema-value-storage"))]
#[path = "flow_merge_finite/schema_tests.rs"]
mod schema_tests;

impl FlowMergeFiniteBack {
    fn step_inner<const PORTS: usize>(
        &mut self,
        io: &mut StepIo<PORTS>,
        inputs: &StepInputBytes<'_, PORTS>,
        mut output: Option<&mut [u8]>,
    ) -> StepOutcome {
        self.staged = None;
        self.staged_side = None;
        self.staged_close = None;
        #[cfg(feature = "execution-workspace")]
        if output
            .as_ref()
            .is_some_and(|bytes| bytes.len() < self.workspace_output_bytes as usize)
        {
            return fail(1073);
        }
        if self.cancelled || self.committed_close.iter().all(|closed| *closed) {
            return StepOutcome::Complete;
        }
        for side in [self.preferred, 1 - self.preferred] {
            if self.committed_close[side] {
                continue;
            }
            let port = PortId(side as u16);
            if io.input_abnormal(port).is_some() {
                return fail(1070);
            }
            if io.input_closed(port) {
                io.consume_closed(port).expect("finite merge input close");
                self.staged_close = Some(side);
                return if self.committed_close[1 - side] {
                    StepOutcome::Complete
                } else {
                    StepOutcome::Progress
                };
            }
        }
        // An exposed terminal must be consumed before unrelated payload work.
        // Payload fairness cannot postpone the selected terminal contract.
        for side in [self.preferred, 1 - self.preferred] {
            if self.committed_close[side] {
                continue;
            }
            let port = PortId(side as u16);
            let Some(reference) = io.input(port) else {
                continue;
            };
            if !io.output_ready_for(PortId(0), reference.byte_len) {
                return StepOutcome::Await;
            }
            let parts = match schema_input::checked_input(
                self.validator.as_ref(),
                self.leaf.as_ref(),
                self.maximum_bytes,
                inputs,
                port,
                reference.byte_len as usize,
            ) {
                Ok(parts) => parts,
                Err(detail) => return fail(detail),
            };
            if let Some(parts) = parts {
                let workspace = output.is_some();
                let destination = output.as_deref_mut().unwrap_or(&mut self.output);
                let Some(destination) = destination.get_mut(..reference.byte_len as usize) else {
                    return fail(1073);
                };
                let (schema, node) = destination.split_at_mut(parts.schema.len());
                schema.copy_from_slice(parts.schema);
                node.copy_from_slice(parts.node);
                self.staged = Some(reference.byte_len as usize);
                #[cfg(feature = "execution-workspace")]
                if workspace {
                    io.send_workspace_output(PortId(0), reference.byte_len)
                        .expect("finite merge admitted workspace output");
                } else {
                    io.send_prepared(PortId(0), reference.byte_len)
                        .expect("finite merge admitted output");
                }
                #[cfg(not(feature = "execution-workspace"))]
                {
                    let _ = workspace;
                    io.send_prepared(PortId(0), reference.byte_len)
                        .expect("finite merge admitted output");
                }
            } else {
                // The ordinary transaction transfers this consumed reference to
                // the output routes, retaining only additional fanout owners.
                io.send(PortId(0), reference)
                    .expect("finite merge exact forwarded input");
            }
            self.staged_side = Some(side);
            io.consume(port).expect("finite merge input");
            return StepOutcome::Progress;
        }
        StepOutcome::Await
    }
}
