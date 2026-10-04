//! One Source-provided seed followed by exact finite state replacements.
//! One admitted kernel-pool reference retains Current between observations.
//! Replacements retire that reference atomically; there is no private byte copy.
use conduit_core::{
    CheckedValueContract, PreparedStructuredValueValidator, StructuredInfoType,
    StructuredInfoTypeShape,
};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId, ValueRef,
};

enum Validator {
    Primitive(CheckedValueContract),
    Structured(PreparedStructuredValueValidator),
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
}

impl SeededStateBack {
    /// Preparation retains the exact selected schema. Initialization is input
    /// meaning supplied by Source, never a host-generated default value.
    pub fn prepare(
        contract: &CheckedValueContract,
        schema: &StructuredInfoType,
    ) -> Result<Self, &'static str> {
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
                Validator::Structured(
                    PreparedStructuredValueValidator::new(schema, contract.maximum_bytes as usize)
                        .map_err(|_| "state schema exceeds its admitted envelope")?,
                )
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

    fn valid(&self, bytes: &[u8]) -> bool {
        match &self.validator {
            Validator::Primitive(contract) => contract.validate(bytes).is_ok(),
            Validator::Structured(validator) => validator.validate(bytes).is_ok(),
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
        if self.terminal {
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
        let Some(bytes) = inputs.input(port) else {
            return fail(933);
        };
        if bytes.len() != reference.byte_len as usize || !self.valid(bytes) {
            return fail(934);
        }
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        let value = io.take_input(port).expect("present state value");
        if let Some(previous) = self.held {
            io.discard(previous)
                .expect("one retained previous state value");
        }
        io.send(PortId(0), value).expect("ready exact state output");
        self.staged = Some(value);
        self.staged_seed = !self.seeded;
        StepOutcome::Progress
    }

    fn step_committed(&mut self) {
        if let Some(value) = self.staged.take() {
            self.held = Some(value);
        }
        self.seeded |= self.staged_seed;
        self.seed_closed |= self.staged_close;
        self.staged_seed = false;
        self.staged_close = false;
    }

    fn cancel(&mut self) {
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
