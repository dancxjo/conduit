//! Prepared one-shot pairing; both Value inputs commit atomically with output.
use conduit_core::{PreparedTypedTuplePairEncoder, StructuredInfoRefusal, StructuredInfoType};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};
pub struct ValuePairBack {
    encoder: PreparedTypedTuplePairEncoder,
    maximum_left: u32,
    maximum_right: u32,
    staged: bool,
    complete: bool,
}
impl ValuePairBack {
    pub fn prepare(
        left: StructuredInfoType,
        maximum_left: u32,
        right: StructuredInfoType,
        maximum_right: u32,
    ) -> Result<Self, StructuredInfoRefusal> {
        Ok(Self {
            encoder: PreparedTypedTuplePairEncoder::new(left, maximum_left, right, maximum_right)?,
            maximum_left,
            maximum_right,
            staged: false,
            complete: false,
        })
    }
    pub fn output_maximum(&self) -> u32 {
        self.encoder.maximum_bytes()
    }
}
impl<const PORTS: usize> StepBack<PORTS> for ValuePairBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        self.staged = false;
        if self.complete {
            return StepOutcome::Complete;
        }
        if [PortId(0), PortId(1)]
            .iter()
            .any(|p| io.input_abnormal(*p).is_some())
        {
            return fail(1650);
        }
        let (Some(left), Some(right)) = (io.input(PortId(0)), io.input(PortId(1))) else {
            return if [PortId(0), PortId(1)].iter().any(|p| {
                io.input_abnormal(*p).is_some() || (io.input(*p).is_none() && io.input_closed(*p))
            }) {
                fail(1650)
            } else {
                StepOutcome::Await
            };
        };
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        let (Some(left_bytes), Some(right_bytes)) =
            (inputs.input(PortId(0)), inputs.input(PortId(1)))
        else {
            return fail(1651);
        };
        if left_bytes.len() != left.byte_len as usize
            || right_bytes.len() != right.byte_len as usize
            || left.byte_len > self.maximum_left
            || right.byte_len > self.maximum_right
        {
            return fail(1652);
        }
        let Ok(encoded) = self.encoder.encode(left_bytes, right_bytes) else {
            return fail(1653);
        };
        if io.send_prepared(PortId(0), encoded.len() as u32).is_err()
            || io.consume(PortId(0)).is_err()
            || io.consume(PortId(1)).is_err()
        {
            return fail(1654);
        }
        self.staged = true;
        StepOutcome::Progress
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0) && self.staged).then(|| self.encoder.encoded())
    }
    fn step_committed(&mut self) {
        self.complete |= self.staged;
        self.staged = false;
    }
    fn cancel(&mut self) {
        self.complete = true;
        self.staged = false;
    }
}
fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}
