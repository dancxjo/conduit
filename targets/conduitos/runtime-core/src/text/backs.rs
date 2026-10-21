//! Fixed Backs for the ordinary bounded text pipeline.

use conduit_kernel::scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome};
use conduit_kernel::{
    BoundedValueRef, HostCallDisposition, HostCallId, PortId, RequestId, ValueRef,
};

const PORTS: usize = crate::PORTS;
const UPPER_REQUEST: RequestId = RequestId(1);
const PRESENT_REQUEST: RequestId = RequestId(2);

pub struct StepDetails {
    pub input: u16,
    pub output: Option<u16>,
    pub cancelled: u16,
    pub failed: Option<u16>,
    pub lifecycle: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiteralState {
    Emitting,
    Complete,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiteralBack {
    pub text: ValueRef,
    pub state: LiteralState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UpperBack {
    pub pending: bool,
    pub emitted: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FaceBack {
    pub pending: bool,
    pub complete: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlannedBack {
    Literal(LiteralBack),
    Upper(UpperBack),
    Face(FaceBack),
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
        }
    }
    fn cancel(&mut self) {
        match self {
            Self::Literal(back) => back.state = LiteralState::Cancelled,
            Self::Upper(back) => back.pending = false,
            Self::Face(back) => back.pending = false,
        }
    }
}

impl StepBack<PORTS> for LiteralBack {
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

impl StepBack<PORTS> for FaceBack {
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

impl LiteralBack {
    pub fn step(&mut self, io: &mut StepIo<PORTS>) -> StepOutcome {
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
    pub fn step(&mut self, io: &mut StepIo<PORTS>) -> StepOutcome {
        step_transform(
            &mut self.pending,
            &mut self.emitted,
            UPPER_REQUEST,
            super::MAX_VALUE_BYTES as u32,
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

impl FaceBack {
    pub fn step(&mut self, io: &mut StepIo<PORTS>) -> StepOutcome {
        step_sink(
            &mut self.pending,
            &mut self.complete,
            PRESENT_REQUEST,
            super::MAX_VALUE_BYTES as u32,
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

pub fn step_transform(
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

pub fn step_sink(
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

pub const fn invalid(detail: u16) -> StepOutcome {
    failure(conduit_kernel::FailureCode::InvalidLifecycle, detail)
}
pub const fn failure(code: conduit_kernel::FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(conduit_kernel::Failure { code, detail })
}
