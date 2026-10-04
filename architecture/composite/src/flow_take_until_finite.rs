//! Finite typed forwarding with a Source-provided Boolean completion signal.
use crate::prelude::*;
use conduit_core::{
    KindId, PreparedStructuredValueValidator, StructuredInfoRefusal, StructuredInfoType,
    StructuredInfoTypeShape,
};
use conduit_kernel::{scheduler::*, Failure, FailureCode, PortId};
pub struct FlowTakeUntilFiniteBack {
    validator: Option<PreparedStructuredValueValidator>,
    leaf: Option<KindId>,
    output: Vec<u8>,
    staged: Option<usize>,
    stopped: bool,
    staged_stop: bool,
    stop_closed: bool,
    staged_close: bool,
    prefer_values: bool,
}
impl FlowTakeUntilFiniteBack {
    pub fn prepare(
        schema: &StructuredInfoType,
        maximum: u32,
    ) -> Result<Self, StructuredInfoRefusal> {
        let leaf = if let StructuredInfoTypeShape::Leaf(kind) = schema.shape() {
            Some(kind.clone())
        } else {
            None
        };
        Ok(Self {
            validator: if leaf.is_none() {
                Some(PreparedStructuredValueValidator::new(
                    schema,
                    maximum as usize,
                )?)
            } else {
                None
            },
            leaf,
            output: vec![0; maximum as usize],
            staged: None,
            stopped: false,
            staged_stop: false,
            stop_closed: false,
            staged_close: false,
            prefer_values: false,
        })
    }
    fn valid(&self, bytes: &[u8]) -> bool {
        bytes.len() <= self.output.len()
            && self
                .validator
                .as_ref()
                .is_none_or(|validator| validator.validate(bytes).is_ok())
            && self.leaf.as_ref().is_none_or(|kind| {
                conduit_core::validate_primitive_info(kind.as_str(), bytes).is_ok()
            })
    }
}
impl<const PORTS: usize> StepBack<PORTS> for FlowTakeUntilFiniteBack {
    fn terminal_transductions(&self) -> [Option<AssignedTerminalTransduction>; PORTS] {
        let mut contracts = [None; PORTS];
        for (input, close) in [
            (0, AssignedNormalCloseTransduction::PropagateAfterDrain),
            (1, AssignedNormalCloseTransduction::Consume),
        ] {
            contracts[input] = Some(AssignedTerminalTransduction {
                input: PortId(input as u16),
                output: PortId(0),
                normal_close: close,
                abnormal: AssignedAbnormalTransduction::NotAccepted,
                cancellation: AssignedCancellationTransduction::NotCancellable,
            });
        }
        contracts
    }
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.stopped {
            return StepOutcome::Complete;
        }
        if (0..2).any(|port| io.input_abnormal(PortId(port)).is_some()) {
            return fail(1090);
        }
        let signal = if let Some(reference) = io.input(PortId(1)) {
            match inputs.input(PortId(1)) {
                Some([value @ (0 | 1)]) if reference.byte_len == 1 => Some(*value == 1),
                _ => return fail(1091),
            }
        } else {
            None
        };
        if !self.stop_closed && io.input_closed(PortId(1)) {
            io.consume_closed(PortId(1))
                .expect("observed control closure");
            self.staged_close = true;
            return StepOutcome::Progress;
        }
        if signal == Some(false) && (!self.prefer_values || io.input(PortId(0)).is_none()) {
            io.consume(PortId(1))
                .expect("observed false completion signal");
            self.prefer_values = true;
            return StepOutcome::Progress;
        }
        if let Some(reference) = io.input(PortId(0)) {
            let Some(bytes) = inputs.input(PortId(0)) else {
                return fail(1092);
            };
            if bytes.len() != reference.byte_len as usize || !self.valid(bytes) {
                return fail(1093);
            }
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            self.output[..bytes.len()].copy_from_slice(bytes);
            self.staged = Some(bytes.len());
            io.consume(PortId(0)).expect("observed finite value");
            io.send_prepared(PortId(0), bytes.len() as u32)
                .expect("admitted finite output");
            if signal == Some(true) {
                io.consume(PortId(1))
                    .expect("observed true completion signal");
                self.staged_stop = true;
            }
            self.prefer_values = false;
            return StepOutcome::Progress;
        }
        if signal == Some(true) {
            io.consume(PortId(1))
                .expect("observed true completion signal");
            self.staged_stop = true;
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) {
            io.consume_closed(PortId(0))
                .expect("observed value closure");
            self.staged_stop = true;
            return StepOutcome::Progress;
        }
        StepOutcome::Await
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0))
            .then(|| self.staged.map(|len| &self.output[..len]))
            .flatten()
    }
    fn step_committed(&mut self) {
        self.stopped |= self.staged_stop;
        self.stop_closed |= self.staged_close;
        self.staged_stop = false;
        self.staged_close = false;
        self.staged = None;
    }
    fn cancel(&mut self) {
        self.stopped = true;
        self.staged = None;
        self.staged_stop = false;
    }
}
fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}
