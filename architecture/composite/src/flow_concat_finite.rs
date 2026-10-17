//! Prepared ordered concatenation; right payload waits for normal left close.
use crate::prelude::*;
use conduit_core::{
    KindId, PreparedStructuredValueValidator, StructuredInfoRefusal, StructuredInfoType,
    StructuredInfoTypeShape,
};
use conduit_kernel::{
    scheduler::{
        AssignedAbnormalTransduction, AssignedCancellationTransduction,
        AssignedNormalCloseTransduction, AssignedTerminalTransduction, StepBack, StepInputBytes,
        StepIo, StepOutcome,
    },
    Failure, FailureCode, PortId,
};

pub struct FlowConcatFiniteBack {
    validator: Option<PreparedStructuredValueValidator>,
    leaf: Option<KindId>,
    output: Vec<u8>,
    staged: Option<usize>,
    committed_close: [bool; 2],
    staged_close: Option<usize>,
    cancelled: bool,
}
impl FlowConcatFiniteBack {
    pub fn prepare(
        schema: &StructuredInfoType,
        maximum_bytes: u32,
    ) -> Result<Self, StructuredInfoRefusal> {
        Ok(Self {
            validator: if matches!(schema.shape(), StructuredInfoTypeShape::Leaf(_)) {
                None
            } else {
                Some(PreparedStructuredValueValidator::new(
                    schema,
                    maximum_bytes as usize,
                )?)
            },
            leaf: if let StructuredInfoTypeShape::Leaf(kind) = schema.shape() {
                Some(kind.clone())
            } else {
                None
            },
            output: vec![0; maximum_bytes as usize],
            staged: None,
            committed_close: [false; 2],
            staged_close: None,
            cancelled: false,
        })
    }
}
impl<const PORTS: usize> StepBack<PORTS> for FlowConcatFiniteBack {
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
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.cancelled || self.committed_close.iter().all(|closed| *closed) {
            return StepOutcome::Complete;
        }
        for side in [0, 1] {
            if self.committed_close[side] {
                continue;
            }
            let port = PortId(side as u16);
            if io.input_abnormal(port).is_some() {
                return fail(1080);
            }
            if io.input_closed(port) {
                io.consume_closed(port).expect("finite concat input close");
                self.staged_close = Some(side);
                return if self.committed_close[1 - side] {
                    StepOutcome::Complete
                } else {
                    StepOutcome::Progress
                };
            }
        }
        // An exposed terminal must be consumed before unrelated payload work.
        // Ordered payload work cannot postpone the selected terminal contract.
        for side in [if self.committed_close[0] { 1 } else { 0 }] {
            if self.committed_close[side] {
                continue;
            }
            let port = PortId(side as u16);
            let Some(reference) = io.input(port) else {
                continue;
            };
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            let Some(bytes) = inputs.input(port) else {
                return fail(1081);
            };
            if bytes.len() != reference.byte_len as usize
                || bytes.len() > self.output.len()
                || self
                    .validator
                    .as_ref()
                    .is_some_and(|validator| validator.validate(bytes).is_err())
                || self.leaf.as_ref().is_some_and(|kind| {
                    conduit_core::validate_primitive_info(kind.as_str(), bytes).is_err()
                })
            {
                return fail(1082);
            }
            self.output[..bytes.len()].copy_from_slice(bytes);
            self.staged = Some(bytes.len());
            io.consume(port).expect("finite concat input");
            io.send_prepared(PortId(0), bytes.len() as u32)
                .expect("finite concat admitted output");
            return StepOutcome::Progress;
        }
        StepOutcome::Await
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        if port != PortId(0) {
            return None;
        }
        self.staged.map(|len| &self.output[..len])
    }
    fn step_committed(&mut self) {
        if let Some(side) = self.staged_close.take() {
            self.committed_close[side] = true;
        }
        self.staged = None;
    }
    fn cancel(&mut self) {
        self.cancelled = true;
        self.staged = None;
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
