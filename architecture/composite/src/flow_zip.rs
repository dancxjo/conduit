//! Allocation-prepared, host-independent realization of exact `flow/zip`.
use crate::prelude::*;
use conduit_core::{
    CheckedValueContract, PreparedTuplePairEncoder, PreparedTypedTuplePairEncoder,
    StructuredInfoRefusal, StructuredInfoType, StructuredInfoTypeShape,
};
mod encoder;
mod exact;
mod schema_input;
#[cfg(feature = "execution-workspace")]
mod workspace_encoder;
use conduit_kernel::{
    scheduler::{
        AssignedAbnormalTransduction, AssignedCancellationTransduction,
        AssignedFiniteTerminalEmission, AssignedNormalCloseTransduction,
        AssignedTerminalTransduction, StepBack, StepInputBytes, StepIo, StepOutcome,
    },
    Failure, FailureCode, PortId,
};
use encoder::PairEncoder;
pub use exact::FlowZipExactBack;

pub struct FlowZipBack {
    left: Vec<u8>,
    left_len: Option<usize>,
    right: Vec<u8>,
    right_len: Option<usize>,
    candidate_side: Option<(usize, usize)>,
    encoder: PairEncoder,
    output_staged: bool,
    finish_after_commit: bool,
    closing: bool,
    drain_right_until_close: bool,
    terminal: bool,
    output_maximum: u32,
    finite: bool,
    feedback: bool,
    return_owed: bool,
    right_closed: bool,
}

impl<const PORTS: usize> StepBack<PORTS> for FlowZipBack {
    #[cfg(feature = "schema-value-storage")]
    fn canonical_input_schema(&self, port: PortId) -> Option<&[u8]> {
        self.encoder.input_schema(usize::from(port.0))
    }
    #[cfg(feature = "schema-value-storage")]
    fn canonical_output_schema(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0))
            .then(|| self.encoder.output_schema())
            .flatten()
    }
    fn terminal_transductions(&self) -> [Option<AssignedTerminalTransduction>; PORTS] {
        let mut contracts = [None; PORTS];
        let flush =
            AssignedNormalCloseTransduction::FlushThenPropagate(AssignedFiniteTerminalEmission {
                maximum_items: 1,
                maximum_bytes: self.output_maximum,
            });
        for (input, contract) in contracts.iter_mut().enumerate().take(2) {
            *contract = Some(AssignedTerminalTransduction {
                input: PortId(input as u16),
                output: PortId(0),
                normal_close: flush,
                abnormal: if self.finite {
                    AssignedAbnormalTransduction::NotAccepted
                } else {
                    AssignedAbnormalTransduction::PropagateAfterDrain
                },
                cancellation: AssignedCancellationTransduction::NotCancellable,
            });
        }
        contracts
    }

    #[cfg(feature = "execution-workspace")]
    fn workspace_requirements(
        &self,
    ) -> Option<conduit_kernel::scheduler::StepWorkspaceRequirements> {
        self.encoder.workspace_maximum().map(|output_bytes| {
            conduit_kernel::scheduler::StepWorkspaceRequirements {
                output_bytes,
                scratch_bytes: 0,
            }
        })
    }
    #[cfg(feature = "execution-workspace")]
    fn step_with_workspace(
        &mut self,
        io: &mut StepIo<PORTS>,
        inputs: &StepInputBytes<'_, PORTS>,
        workspace: &mut conduit_kernel::scheduler::StepWorkspace<'_>,
    ) -> StepOutcome {
        if self.encoder.workspace_maximum().is_some() {
            let (output, _) = workspace.buffers();
            self.step_inner(io, inputs, Some(output))
        } else {
            self.step_inner(io, inputs, None)
        }
    }
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        self.step_inner(io, inputs, None)
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        #[cfg(feature = "execution-workspace")]
        if self.encoder.workspace_maximum().is_some() {
            return None;
        }
        (port == PortId(0) && self.output_staged).then(|| {
            // The last encode length is the exact staged length. The encoder
            // exposes it through the prepared slice without allocating.
            self.encoder.encoded()
        })
    }

    fn step_committed(&mut self) {
        if let Some((side, len)) = self.candidate_side.take() {
            if side == 0 {
                self.left_len = Some(len);
                self.return_owed = false;
            } else {
                self.right_len = Some(len);
            }
        }
        if self.output_staged {
            self.return_owed = self.feedback;
            self.left_len = None;
            self.right_len = None;
            self.output_staged = false;
        }
        if self.finish_after_commit {
            self.finish_after_commit = false;
            if self.finite {
                self.closing = true;
            } else {
                self.terminal = true;
            }
        }
    }

    fn cancel(&mut self) {
        self.left_len = None;
        self.right_len = None;
        self.candidate_side = None;
        self.output_staged = false;
        self.finish_after_commit = false;
        self.terminal = true;
    }
}

impl FlowZipBack {
    fn stage_output<const P: usize>(io: &mut StepIo<P>, length: usize, workspace: bool) {
        #[cfg(feature = "execution-workspace")]
        if workspace {
            io.send_workspace_output(PortId(0), length as u32)
                .expect("admitted flow/zip workspace output");
            return;
        }
        let _ = workspace;
        io.send_prepared(PortId(0), length as u32)
            .expect("admitted flow/zip output");
    }
    fn encode_pair(&mut self) -> Option<usize> {
        self.encode_pair_into(None)
    }
    fn encode_pair_into(&mut self, output: Option<&mut [u8]>) -> Option<usize> {
        let left = &self.left[..self.left_len?];
        let right = &self.right[..self.right_len?];
        #[cfg(feature = "execution-workspace")]
        if let Some(output) = output {
            return self
                .encoder
                .encode_into(left, right, output)
                .ok()
                .map(<[u8]>::len);
        }
        let _ = output;
        self.encoder.encode(left, right).ok().map(<[u8]>::len)
    }
}

impl FlowZipBack {
    /// Prepare typed pairing for payload-only finite streams. Normal closure
    /// flushes at most one already-formed pair, exactly as the shared zip law.
    pub fn prepare_typed_finite(
        left: &CheckedValueContract,
        left_type: StructuredInfoType,
        right: &CheckedValueContract,
        right_type: StructuredInfoType,
    ) -> Result<Self, StructuredInfoRefusal> {
        let mut back = Self::prepare_typed(left, left_type, right, right_type)?;
        back.finite = true;
        Ok(back)
    }

    /// Pair events with successive returned state generations. Event closure
    /// drains a pending event and waits for the final published pair's return.
    pub fn prepare_typed_feedback(
        left: &CheckedValueContract,
        left_type: StructuredInfoType,
        right: &CheckedValueContract,
        right_type: StructuredInfoType,
    ) -> Result<Self, StructuredInfoRefusal> {
        let mut back = Self::prepare_typed_finite(left, left_type, right, right_type)?;
        back.feedback = true;
        back.return_owed = true;
        Ok(back)
    }

    /// Prepare all pair buffers before play. The caller must validate the
    /// selected semantic and implementation contracts separately.
    pub fn prepare_typed(
        left: &CheckedValueContract,
        left_type: StructuredInfoType,
        right: &CheckedValueContract,
        right_type: StructuredInfoType,
    ) -> Result<Self, StructuredInfoRefusal> {
        Self::prepare_typed_mode(left, left_type, right, right_type, false)
    }
    #[cfg(feature = "execution-workspace")]
    pub fn prepare_typed_workspace(
        left: &CheckedValueContract,
        left_type: StructuredInfoType,
        right: &CheckedValueContract,
        right_type: StructuredInfoType,
    ) -> Result<Self, StructuredInfoRefusal> {
        Self::prepare_typed_mode(left, left_type, right, right_type, true)
    }
    #[cfg(feature = "execution-workspace")]
    pub fn prepare_typed_finite_workspace(
        left: &CheckedValueContract,
        left_type: StructuredInfoType,
        right: &CheckedValueContract,
        right_type: StructuredInfoType,
    ) -> Result<Self, StructuredInfoRefusal> {
        let mut back = Self::prepare_typed_workspace(left, left_type, right, right_type)?;
        back.finite = true;
        Ok(back)
    }
    #[cfg(feature = "execution-workspace")]
    pub fn prepare_typed_feedback_workspace(
        left: &CheckedValueContract,
        left_type: StructuredInfoType,
        right: &CheckedValueContract,
        right_type: StructuredInfoType,
    ) -> Result<Self, StructuredInfoRefusal> {
        let mut back = Self::prepare_typed_finite_workspace(left, left_type, right, right_type)?;
        back.feedback = true;
        back.return_owed = true;
        Ok(back)
    }
    fn prepare_typed_mode(
        left: &CheckedValueContract,
        left_type: StructuredInfoType,
        right: &CheckedValueContract,
        right_type: StructuredInfoType,
        workspace: bool,
    ) -> Result<Self, StructuredInfoRefusal> {
        let transport_kind = |ty: &StructuredInfoType| match ty.shape() {
            StructuredInfoTypeShape::Leaf(kind) => Ok(kind.clone()),
            _ => ty.profile().map(|profile| profile.value_kind().clone()),
        };
        if transport_kind(&left_type)? != left.value_kind
            || transport_kind(&right_type)? != right.value_kind
        {
            return Err(StructuredInfoRefusal::WrongType);
        }
        let left_backing = crate::value_backing::bytes(&left_type, left.maximum_bytes);
        let right_backing = crate::value_backing::bytes(&right_type, right.maximum_bytes);
        let encoder = PreparedTypedTuplePairEncoder::new(
            left_type,
            left.maximum_bytes,
            right_type,
            right.maximum_bytes,
        )?;
        Ok(Self {
            left: vec![0; left_backing],
            left_len: None,
            right: vec![0; right_backing],
            right_len: None,
            candidate_side: None,
            output_maximum: encoder.maximum_encoded_bytes(),
            encoder: {
                #[cfg(feature = "execution-workspace")]
                if workspace {
                    PairEncoder::Workspace(Box::new(workspace_encoder::WorkspacePair::prepare(
                        &encoder,
                        [left.maximum_bytes, right.maximum_bytes],
                    )?))
                } else {
                    PairEncoder::Typed(Box::new(encoder.into_runtime()))
                }
                #[cfg(not(feature = "execution-workspace"))]
                {
                    let _ = workspace;
                    PairEncoder::Typed(Box::new(encoder.into_runtime()))
                }
            },
            output_staged: false,
            finish_after_commit: false,
            closing: false,
            drain_right_until_close: false,
            terminal: false,
            finite: false,
            feedback: false,
            return_owed: false,
            right_closed: false,
        })
    }

    /// Prepare primitive-leaf pairing with separately validated selected contracts.
    pub fn prepare(
        left: &CheckedValueContract,
        right: &CheckedValueContract,
    ) -> Result<Self, conduit_core::StructuredInfoRefusal> {
        let encoder = PreparedTuplePairEncoder::new(
            left.value_kind.clone(),
            left.maximum_bytes,
            right.value_kind.clone(),
            right.maximum_bytes,
        )?;
        Ok(Self {
            left: vec![0; left.maximum_bytes as usize],
            left_len: None,
            right: vec![0; right.maximum_bytes as usize],
            right_len: None,
            candidate_side: None,
            output_maximum: encoder.maximum_bytes(),
            encoder: PairEncoder::Leaf(Box::new(encoder)),
            output_staged: false,
            finish_after_commit: false,
            closing: false,
            drain_right_until_close: false,
            terminal: false,
            finite: false,
            feedback: false,
            return_owed: false,
            right_closed: false,
        })
    }
}

fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}

#[cfg(test)]
mod exact_tests;
#[cfg(test)]
mod tests;

#[cfg(all(test, feature = "schema-value-storage"))]
mod schema_tests;

#[path = "flow_zip/step.rs"]
mod step;
