//! Fixed Backs for the ordinary bounded text pipeline.

use conduit_kernel::scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome};
use conduit_kernel::{
    BoundedValueRef, HostCallDisposition, HostCallId, PortId, RequestId, ValueRef,
};

const PORTS: usize = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const TIMER_REQUEST: RequestId = RequestId(3);
const TICK_PRESENT_REQUEST: RequestId = RequestId(4);

pub(crate) use conduitos_runtime_core::text::backs::{
    FaceBack, LiteralBack, LiteralState, StepDetails, UpperBack, failure, invalid, step_sink,
};
const _: () = assert!(PORTS == conduitos_runtime_core::PORTS);
const _: () =
    assert!(conduit_text::MAX_TEXT_BYTES as usize == conduitos_runtime_core::text::MAX_VALUE_BYTES);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TimerState {
    Waiting,
    Requested,
    Complete,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct TimerBack {
    pub wait: BoundedValueRef,
    pub tick: ValueRef,
    pub state: TimerState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct TickFaceBack {
    pub pending: bool,
    pub complete: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PlannedBack {
    Literal(LiteralBack),
    Upper(UpperBack),
    Face(FaceBack),
    Timer(TimerBack),
    TickFace(TickFaceBack),
}

impl StepBack<PORTS> for PlannedBack {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        _input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        match self {
            Self::Literal(back) => back.step(io),
            Self::Upper(back) => back.step(io),
            Self::Face(back) => back.step(io),
            Self::Timer(back) => back.step(io),
            Self::TickFace(back) => back.step(io),
        }
    }
    fn cancel(&mut self) {
        match self {
            Self::Literal(back) => back.state = LiteralState::Cancelled,
            Self::Upper(back) => back.pending = false,
            Self::Face(back) => back.pending = false,
            Self::Timer(back) => back.state = TimerState::Cancelled,
            Self::TickFace(back) => back.pending = false,
        }
    }
}

impl TimerBack {
    pub(super) fn step(&mut self, io: &mut StepIo<PORTS>) -> StepOutcome {
        match self.state {
            TimerState::Waiting => {
                if io
                    .request_host_call(TIMER_REQUEST, HostCallId(0), self.wait)
                    .is_err()
                {
                    return failure(conduit_kernel::FailureCode::HostCallFailed, 41);
                }
                self.state = TimerState::Requested;
                StepOutcome::Progress
            }
            TimerState::Requested => {
                let Some((request, outcome)) = io.host_completion() else {
                    return StepOutcome::Await;
                };
                if request == TIMER_REQUEST && outcome.disposition == HostCallDisposition::Cancelled
                {
                    return failure(conduit_kernel::FailureCode::Cancelled, 40);
                }
                if request != TIMER_REQUEST
                    || outcome.disposition != HostCallDisposition::Completed
                    || outcome.output.is_some()
                    || outcome.failure.is_some()
                {
                    return failure(conduit_kernel::FailureCode::HostCallFailed, 41);
                }
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                if io.consume_host_completion().is_err() || io.send(PortId(0), self.tick).is_err() {
                    return invalid(41);
                }
                self.state = TimerState::Complete;
                StepOutcome::Complete
            }
            TimerState::Complete => StepOutcome::Complete,
            TimerState::Cancelled => failure(conduit_kernel::FailureCode::Cancelled, 40),
        }
    }
}

impl TickFaceBack {
    pub(super) fn step(&mut self, io: &mut StepIo<PORTS>) -> StepOutcome {
        step_sink(
            &mut self.pending,
            &mut self.complete,
            TICK_PRESENT_REQUEST,
            conduit_time::TICK_ENCODED_LEN,
            io,
            StepDetails {
                input: 50,
                output: None,
                cancelled: 51,
                failed: None,
                lifecycle: 52,
            },
        )
    }
}
