//! Transactional unary pure operation; no sampling loop or hardware access.
use super::*;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};
pub struct ObservationBack {
    codec: PreparedObservationCodec,
    staged: bool,
    cancelled: bool,
}
impl ObservationBack {
    pub fn prepare(contract: &ObservationContract) -> Result<Self, StructuredInfoRefusal> {
        Ok(Self {
            codec: PreparedObservationCodec::prepare(contract)?,
            staged: false,
            cancelled: false,
        })
    }
    pub fn retained_backing_bytes(&self) -> usize {
        self.codec.retained_backing_bytes()
    }
}
impl<const P: usize> StepBack<P> for ObservationBack {
    fn step(&mut self, io: &mut StepIo<P>, inputs: &StepInputBytes<'_, P>) -> StepOutcome {
        self.staged = false;
        if self.cancelled {
            return StepOutcome::Complete;
        }
        if io.input_abnormal(PortId(0)).is_some() {
            return fail(1);
        }
        let Some(reference) = io.input(PortId(0)) else {
            return if io.input_closed(PortId(0)) {
                if io.consume_closed(PortId(0)).is_err() {
                    fail(2)
                } else {
                    StepOutcome::Complete
                }
            } else {
                StepOutcome::Await
            };
        };
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        let Some(input) = inputs.input(PortId(0)) else {
            return fail(3);
        };
        if input.len() != reference.byte_len as usize {
            return fail(4);
        }
        // One finite native conversion, as for existing observation Backs.
        // This is cooperative operation fuel, not a claim about MCU clock cycles.
        if io.consume_fuel(1).is_err() {
            return fail(5);
        }
        let length = match self.codec.convert(input) {
            Ok(output) => output.len(),
            Err(CodecError::Canonical(_)) => return fail(6),
            Err(CodecError::Arithmetic) => return fail(7),
        };
        if io.send_prepared(PortId(0), length as u32).is_err() || io.consume(PortId(0)).is_err() {
            return fail(8);
        }
        self.staged = true;
        StepOutcome::Progress
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (self.staged && port == PortId(0)).then(|| self.codec.output())
    }
    fn step_committed(&mut self) {
        self.staged = false;
        self.codec.clear();
    }
    fn cancel(&mut self) {
        self.cancelled = true;
        self.staged = false;
        self.codec.clear();
    }
}
fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail: 1800 + detail,
    })
}
