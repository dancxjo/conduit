//! Pure, finite representation conversion under the existing kernel.
use super::{ObservationContract, ObservationRefusal, PreparedObservationCodec};
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
    pub fn prepare(
        contract: &ObservationContract,
    ) -> Result<Self, conduit_core::StructuredInfoRefusal> {
        Ok(Self {
            codec: PreparedObservationCodec::prepare(contract)?,
            staged: false,
            cancelled: false,
        })
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
        // One bounded representation conversion, separate from IO fuel.
        if io.consume_fuel(1).is_err() {
            return fail(9);
        }
        let output = match self.codec.convert(input) {
            Ok(output) => output,
            Err(ObservationRefusal::Canonical(_)) => return fail(5),
            Err(ObservationRefusal::CoordinateOutOfRange) => return fail(6),
            Err(ObservationRefusal::Form(_)) => return fail(7),
        };
        if io.send_prepared(PortId(0), output.len() as u32).is_err()
            || io.consume(PortId(0)).is_err()
        {
            return fail(8);
        }
        self.staged = true;
        StepOutcome::Progress
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0) && self.staged).then(|| self.codec.output())
    }
    fn step_committed(&mut self) {
        self.staged = false;
    }
    fn cancel(&mut self) {
        self.cancelled = true;
        self.staged = false;
    }
}
fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail: 1700 + detail,
    })
}
