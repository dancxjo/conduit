//! Fixed Backs for the ordinary bounded text pipeline.

use conduit_kernel::scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome};
use conduit_kernel::{
    BoundedValueRef, HostCallDisposition, HostCallId, PortId, RequestId, ValueRef,
};

const PORTS: usize = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const UPPER_REQUEST: RequestId = RequestId(1);
const PRESENT_REQUEST: RequestId = RequestId(2);
const TIMER_REQUEST: RequestId = RequestId(3);
const TICK_PRESENT_REQUEST: RequestId = RequestId(4);

pub(super) struct StepDetails {
    pub input: u16,
    pub output: Option<u16>,
    pub cancelled: u16,
    pub failed: Option<u16>,
    pub lifecycle: u16,
}

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
pub(super) struct TickPresentationBack {
    pub pending: bool,
    pub complete: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LiteralState {
    Emitting,
    Complete,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct LiteralBack {
    pub text: ValueRef,
    pub state: LiteralState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct UpperBack {
    pub(super) pending: bool,
    pub emitted: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PresentationBack {
    pub(super) pending: bool,
    pub complete: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PlannedBack {
    Literal(LiteralBack),
    Upper(UpperBack),
    Presentation(PresentationBack),
    Timer(TimerBack),
    TickPresentation(TickPresentationBack),
}

impl StepOperation<PORTS> for PlannedBack {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        _input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        match self {
            Self::Literal(back) => back.step(io),
            Self::Upper(back) => back.step(io),
            Self::Presentation(back) => back.step(io),
            Self::Timer(back) => back.step(io),
            Self::TickPresentation(back) => back.step(io),
        }
    }
    fn cancel(&mut self) {
        match self {
            Self::Literal(back) => back.state = LiteralState::Cancelled,
            Self::Upper(back) => back.pending = false,
            Self::Presentation(back) => back.pending = false,
            Self::Timer(back) => back.state = TimerState::Cancelled,
            Self::TickPresentation(back) => back.pending = false,
        }
    }
}

impl StepOperation<PORTS> for LiteralBack {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        _input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        self.step(io)
    }

    fn cancel(&mut self) {
        self.state = LiteralState::Cancelled;
    }
}

impl StepOperation<PORTS> for PresentationBack {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        _input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        self.step(io)
    }

    fn cancel(&mut self) {
        self.pending = false;
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

impl LiteralBack {
    pub(super) fn step(&mut self, io: &mut StepIo<PORTS>) -> StepOutcome {
        match self.state {
            LiteralState::Emitting => {
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                if io.send(PortId(0), self.text).is_err() {
                    return invalid(11);
                }
                self.state = LiteralState::Complete;
                StepOutcome::Complete
            }
            LiteralState::Complete => StepOutcome::Complete,
            LiteralState::Cancelled => failure(conduit_kernel::FailureCode::Cancelled, 11),
        }
    }
}

impl UpperBack {
    pub(super) fn step(&mut self, io: &mut StepIo<PORTS>) -> StepOutcome {
        step_transform(
            &mut self.pending,
            &mut self.emitted,
            UPPER_REQUEST,
            conduit_text::MAX_TEXT_BYTES,
            io,
            StepDetails {
                input: 30,
                output: Some(31),
                cancelled: 32,
                failed: Some(33),
                lifecycle: 34,
            },
        )
    }
}

impl PresentationBack {
    pub(super) fn step(&mut self, io: &mut StepIo<PORTS>) -> StepOutcome {
        step_sink(
            &mut self.pending,
            &mut self.complete,
            PRESENT_REQUEST,
            conduit_text::MAX_TEXT_BYTES,
            io,
            StepDetails {
                input: 20,
                output: None,
                cancelled: 22,
                failed: Some(23),
                lifecycle: 21,
            },
        )
    }
}

impl TickPresentationBack {
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

pub(super) fn step_transform(
    pending: &mut bool,
    emitted: &mut bool,
    request: RequestId,
    maximum: u32,
    io: &mut StepIo<PORTS>,
    details: StepDetails,
) -> StepOutcome {
    if *pending {
        let Some((completed, outcome)) = io.host_completion() else {
            return StepOutcome::Await;
        };
        if completed == request && outcome.disposition == HostCallDisposition::Cancelled {
            return failure(conduit_kernel::FailureCode::Cancelled, details.cancelled);
        }
        if completed == request && outcome.disposition == HostCallDisposition::Failed {
            return details.failed.map_or_else(
                || invalid(details.lifecycle),
                |detail| failure(conduit_kernel::FailureCode::HostCallFailed, detail),
            );
        }
        let Some(output) = outcome.output else {
            return invalid(details.output.unwrap_or(details.lifecycle));
        };
        if completed != request
            || outcome.disposition != HostCallDisposition::Completed
            || outcome.failure.is_some()
            || !io.output_ready(PortId(0))
            || io.consume_host_completion().is_err()
            || io.send(PortId(0), output.value).is_err()
        {
            return invalid(details.lifecycle);
        }
        *pending = false;
        *emitted = true;
        return StepOutcome::Progress;
    }
    if !*emitted && let Some(value) = io.input(PortId(0)) {
        let Ok(input) = BoundedValueRef::new(value, maximum) else {
            return invalid(details.input);
        };
        if io.consume(PortId(0)).is_err()
            || io.request_host_call(request, HostCallId(0), input).is_err()
        {
            return invalid(details.lifecycle);
        }
        *pending = true;
        return StepOutcome::Progress;
    }
    if io.input_closed(PortId(0)) {
        if io.consume_closed(PortId(0)).is_err() {
            return invalid(details.lifecycle);
        }
        return StepOutcome::Complete;
    }
    StepOutcome::Await
}

pub(super) fn step_sink(
    pending: &mut bool,
    complete: &mut bool,
    request: RequestId,
    maximum: u32,
    io: &mut StepIo<PORTS>,
    details: StepDetails,
) -> StepOutcome {
    if *pending {
        let Some((completed, outcome)) = io.host_completion() else {
            return StepOutcome::Await;
        };
        if completed == request && outcome.disposition == HostCallDisposition::Cancelled {
            return failure(conduit_kernel::FailureCode::Cancelled, details.cancelled);
        }
        if completed == request && outcome.disposition == HostCallDisposition::Failed {
            return details.failed.map_or_else(
                || invalid(details.lifecycle),
                |detail| failure(conduit_kernel::FailureCode::HostCallFailed, detail),
            );
        }
        if completed != request
            || outcome.disposition != HostCallDisposition::Completed
            || outcome.output.is_some()
            || outcome.failure.is_some()
            || io.consume_host_completion().is_err()
        {
            return invalid(details.lifecycle);
        }
        *pending = false;
        *complete = true;
        return StepOutcome::Progress;
    }
    if let Some(value) = io.input(PortId(0)) {
        let Ok(input) = BoundedValueRef::new(value, maximum) else {
            return invalid(details.input);
        };
        if io.consume(PortId(0)).is_err()
            || io.request_host_call(request, HostCallId(0), input).is_err()
        {
            return invalid(details.lifecycle);
        }
        *pending = true;
        return StepOutcome::Progress;
    }
    if *complete && io.input_closed(PortId(0)) {
        if io.consume_closed(PortId(0)).is_err() {
            return invalid(details.lifecycle);
        }
        return StepOutcome::Complete;
    }
    StepOutcome::Await
}

pub(super) const fn invalid(detail: u16) -> StepOutcome {
    failure(conduit_kernel::FailureCode::InvalidLifecycle, detail)
}
pub(super) const fn failure(code: conduit_kernel::FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(conduit_kernel::Failure { code, detail })
}
