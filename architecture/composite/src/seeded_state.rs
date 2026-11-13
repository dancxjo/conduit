//! One Source-provided seed followed by exact finite state replacements.
//! One admitted kernel-pool reference retains Current between observations.
//! Replacements retire that reference atomically. Canonical boundary copies use
//! the scheduler-owned workspace, never a second private retained State.
use conduit_core::{
    BorrowedStructuredValueValidator, CheckedValueContract, PreparedStructuredValueValidator,
    StructuredInfoType, StructuredInfoTypeShape,
};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId, ValueRef,
};

mod schema_input;

enum Validator {
    Primitive(CheckedValueContract),
    Structured(PreparedStructuredValueValidator),
    Static(BorrowedStructuredValueValidator<'static>),
}

pub struct SeededStateBack {
    validator: Validator,
    seeded: bool,
    seed_closed: bool,
    staged_seed: bool,
    staged_close: bool,
    held: Option<ValueRef>,
    staged: Option<ValueRef>,
    terminal: bool,
    flow: bool,
    until: bool,
    staged_terminal: bool,
    #[cfg(all(feature = "execution-workspace", feature = "schema-value-storage"))]
    workspace_output_bytes: u32,
}

impl SeededStateBack {
    /// Preparation retains the exact selected schema. Initialization is input
    /// meaning supplied by Source, never a host-generated default value.
    pub fn prepare(
        contract: &CheckedValueContract,
        schema: &StructuredInfoType,
    ) -> Result<Self, &'static str> {
        Self::prepare_schema(contract, schema, None)
    }

    fn prepare_schema(
        contract: &CheckedValueContract,
        schema: &StructuredInfoType,
        canonical: Option<&'static [u8]>,
    ) -> Result<Self, &'static str> {
        if let Some(canonical) = canonical {
            if schema
                .canonical_bytes()
                .map_err(|_| "invalid state schema")?
                != canonical
            {
                return Err("state schema differs from linked canonical bytes");
            }
        }
        contract
            .validate_definition()
            .map_err(|_| "invalid state value contract")?;
        let validator = match schema.shape() {
            StructuredInfoTypeShape::Leaf(kind) => {
                if kind != &contract.value_kind {
                    return Err("state schema differs from the selected value contract");
                }
                Validator::Primitive(contract.clone())
            }
            _ => {
                if schema
                    .profile()
                    .map_err(|_| "invalid state schema")?
                    .value_kind()
                    != &contract.value_kind
                    || !contract.constraints.is_empty()
                {
                    return Err("unsupported state schema or additional value constraints");
                }
                if let Some(canonical) = canonical {
                    Validator::Static(
                        BorrowedStructuredValueValidator::new(
                            canonical,
                            contract.maximum_bytes as usize,
                        )
                        .map_err(|_| "state schema exceeds its admitted envelope")?,
                    )
                } else {
                    Validator::Structured(
                        PreparedStructuredValueValidator::new(
                            schema,
                            contract.maximum_bytes as usize,
                        )
                        .map_err(|_| "state schema exceeds its admitted envelope")?,
                    )
                }
            }
        };
        Ok(Self {
            validator,
            seeded: false,
            seed_closed: false,
            staged_seed: false,
            staged_close: false,
            held: None,
            staged: None,
            terminal: false,
            flow: false,
            until: false,
            staged_terminal: false,
            #[cfg(all(feature = "execution-workspace", feature = "schema-value-storage"))]
            workspace_output_bytes: if canonical.is_some() {
                crate::value_backing::bytes(schema, contract.maximum_bytes) as u32
            } else {
                0
            },
        })
    }

    pub fn prepare_flow(
        contract: &CheckedValueContract,
        schema: &StructuredInfoType,
    ) -> Result<Self, &'static str> {
        let mut back = Self::prepare(contract, schema)?;
        back.flow = true;
        Ok(back)
    }

    pub fn prepare_until(
        contract: &CheckedValueContract,
        schema: &StructuredInfoType,
    ) -> Result<Self, &'static str> {
        Self::prepare_until_schema(contract, schema, None)
    }

    /// Borrow an exact linked schema for the checked terminal-state lifetime.
    /// Initialization, retained kernel references and terminal laws are unchanged.
    pub fn prepare_until_static(
        contract: &CheckedValueContract,
        schema: &StructuredInfoType,
        canonical: &'static [u8],
    ) -> Result<Self, &'static str> {
        Self::prepare_until_schema(contract, schema, Some(canonical))
    }

    fn prepare_until_schema(
        contract: &CheckedValueContract,
        schema: &StructuredInfoType,
        canonical: Option<&'static [u8]>,
    ) -> Result<Self, &'static str> {
        let StructuredInfoTypeShape::Record { fields, .. } = schema.shape() else {
            return Err("terminal state requires a record");
        };
        if !fields.iter().any(|field| field.name() == "terminal" && matches!(field.value_type().shape(), StructuredInfoTypeShape::Leaf(kind) if kind.as_str() == conduit_core::BOOL_INFO_ID)) {
            return Err("terminal state requires an exact terminal Boolean");
        }
        let mut back = Self::prepare_schema(contract, schema, canonical)?;
        back.flow = true;
        back.until = true;
        Ok(back)
    }

    fn valid(&self, bytes: &[u8]) -> bool {
        match &self.validator {
            Validator::Primitive(contract) => contract.validate(bytes).is_ok(),
            Validator::Structured(validator) => validator.validate(bytes).is_ok(),
            Validator::Static(validator) => validator.validate(bytes).is_ok(),
        }
    }
}

impl<const PORTS: usize> StepBack<PORTS> for SeededStateBack {
    fn terminal_transduction(
        &self,
    ) -> Option<conduit_kernel::scheduler::AssignedTerminalTransduction> {
        use conduit_kernel::scheduler::*;
        self.flow.then_some(AssignedTerminalTransduction {
            input: PortId(1),
            output: PortId(0),
            normal_close: AssignedNormalCloseTransduction::PropagateAfterDrain,
            abnormal: AssignedAbnormalTransduction::NotAccepted,
            cancellation: AssignedCancellationTransduction::NotCancellable,
        })
    }

    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        self.step_inner(
            io,
            inputs,
            #[cfg(feature = "execution-workspace")]
            None,
        )
    }
    #[cfg(feature = "execution-workspace")]
    fn workspace_requirements(
        &self,
    ) -> Option<conduit_kernel::scheduler::StepWorkspaceRequirements> {
        self.state_workspace_requirements()
    }
    #[cfg(feature = "execution-workspace")]
    fn step_with_workspace(
        &mut self,
        io: &mut StepIo<PORTS>,
        inputs: &StepInputBytes<'_, PORTS>,
        workspace: &mut conduit_kernel::scheduler::StepWorkspace<'_>,
    ) -> StepOutcome {
        self.step_inner(io, inputs, Some(workspace))
    }
    #[cfg(all(feature = "execution-workspace", feature = "schema-value-storage"))]
    fn canonical_input_schema(&self, port: PortId) -> Option<&[u8]> {
        (port.0 < 2).then(|| self.linked_schema()).flatten()
    }
    #[cfg(all(feature = "execution-workspace", feature = "schema-value-storage"))]
    fn canonical_output_schema(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0)).then(|| self.linked_schema()).flatten()
    }

    fn step_committed(&mut self) {
        if let Some(value) = self.staged.take() {
            self.held = Some(value);
        }
        self.terminal |= self.staged_terminal;
        self.staged_terminal = false;
        self.seeded |= self.staged_seed;
        self.seed_closed |= self.staged_close;
        self.staged_seed = false;
        self.staged_close = false;
    }

    fn cancel(&mut self) {
        self.staged_terminal = false;
        self.staged_seed = false;
        self.staged_close = false;
        self.held = None;
        self.staged = None;
        self.terminal = true;
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

#[cfg(test)]
mod terminal_tests;

impl SeededStateBack {
    fn step_inner<const PORTS: usize>(
        &mut self,
        io: &mut StepIo<PORTS>,
        inputs: &StepInputBytes<'_, PORTS>,
        #[cfg(feature = "execution-workspace")] workspace: Option<
            &mut conduit_kernel::scheduler::StepWorkspace<'_>,
        >,
    ) -> StepOutcome {
        if self.terminal {
            for port in [PortId(0), PortId(1)] {
                if io.input_closed(port) && io.input(port).is_none() {
                    io.consume_closed(port)
                        .expect("exposed terminal state input");
                }
            }
            if let Some(held) = self.held.take() {
                io.discard(held).expect("one terminal retained state");
            }
            return StepOutcome::Complete;
        }
        if io.input_abnormal(PortId(0)).is_some() || io.input_abnormal(PortId(1)).is_some() {
            return fail(930);
        }
        if !self.seeded && io.input_closed(PortId(0)) {
            return fail(931);
        }
        if self.seeded && io.input(PortId(0)).is_some() {
            return fail(932);
        }
        if self.seeded && !self.seed_closed && io.input_closed(PortId(0)) {
            io.consume_closed(PortId(0)).expect("observed seed closure");
            self.staged_close = true;
            return StepOutcome::Progress;
        }
        if self.seeded && io.input_closed(PortId(1)) {
            io.consume_closed(PortId(1))
                .expect("observed replacement closure");
            if let Some(held) = self.held.take() {
                io.discard(held).expect("one retained state value");
            }
            self.terminal = true;
            return StepOutcome::Complete;
        }
        let port = PortId(if self.seeded { 1 } else { 0 });
        let Some(reference) = io.input(port) else {
            return StepOutcome::Await;
        };
        let (terminal, parts) = match self.checked_input(inputs, port, reference.byte_len as usize)
        {
            Ok(input) => input,
            Err(detail) => return fail(detail),
        };
        if !io.output_ready_for(PortId(0), reference.byte_len) {
            return StepOutcome::Await;
        }
        #[cfg(feature = "execution-workspace")]
        if let Some(parts) = parts {
            let Some(workspace) = workspace else {
                return fail(938);
            };
            let (output, _) = workspace.buffers();
            let Some(output) = output.get_mut(..reference.byte_len as usize) else {
                return fail(938);
            };
            let (schema, node) = output.split_at_mut(parts.schema.len());
            schema.copy_from_slice(parts.schema);
            node.copy_from_slice(parts.node);
            if io
                .send_workspace_output(PortId(0), reference.byte_len)
                .is_err()
            {
                return fail(938);
            }
        }
        let value = io.take_input(port).expect("present state value");
        if let Some(previous) = self.held {
            io.discard(previous)
                .expect("one retained previous state value");
        }
        if parts.is_none() {
            io.send(PortId(0), value).expect("ready exact state output");
        }
        self.staged_terminal = terminal;
        self.staged = Some(value);
        self.staged_seed = !self.seeded;
        StepOutcome::Progress
    }
}

#[cfg(all(
    test,
    feature = "schema-value-storage",
    feature = "execution-workspace"
))]
mod schema_tests;
