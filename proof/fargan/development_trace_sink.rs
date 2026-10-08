//! Development sink using ordinary staged StepBack transactions. Not a product
//! placement factory, Native admission or recurrent feedback acknowledgement.
use super::recorder::{DevelopmentTraceRecorder, TraceRefusal};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};

pub struct DevelopmentTraceSink {
    recorder: DevelopmentTraceRecorder,
    staging: Vec<u8>,
    candidate: Option<usize>,
    paused: bool,
    cancelled: bool,
}
impl DevelopmentTraceSink {
    pub fn prepare(
        recorder: DevelopmentTraceRecorder,
        maximum: usize,
    ) -> Result<Self, TraceRefusal> {
        if maximum == 0 || maximum > 16384 {
            return Err(TraceRefusal::Preparation);
        }
        Ok(Self {
            recorder,
            staging: vec![0; maximum],
            candidate: None,
            paused: false,
            cancelled: false,
        })
    }
    pub fn pause(&mut self, paused: bool) {
        self.paused = paused;
    }
    pub fn recorder(&self) -> &DevelopmentTraceRecorder {
        &self.recorder
    }
}
impl<const PORTS: usize> StepBack<PORTS> for DevelopmentTraceSink {
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.cancelled {
            return fail(FailureCode::Cancelled);
        }
        self.candidate = None;
        if self.paused {
            return StepOutcome::Await;
        }
        let Some(input) = inputs.input(PortId(0)) else {
            return if io.input_closed(PortId(0)) {
                if self.recorder.finish().is_ok() {
                    StepOutcome::Complete
                } else {
                    fail(FailureCode::InvalidInput)
                }
            } else {
                StepOutcome::Await
            };
        };
        if input.len() > self.staging.len() || self.recorder.validate_next(input).is_err() {
            return fail(FailureCode::InvalidInput);
        }
        self.staging[..input.len()].copy_from_slice(input);
        self.candidate = Some(input.len());
        io.consume(PortId(0)).expect("present trace input");
        StepOutcome::Progress
    }
    fn step_committed(&mut self) {
        if let Some(length) = self.candidate.take() {
            self.recorder
                .record(&self.staging[..length])
                .expect("already validated immutable staged row");
        }
    }
    fn cancel(&mut self) {
        self.cancelled = true;
        self.candidate = None;
    }
}
fn fail(code: FailureCode) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail: 966 })
}
