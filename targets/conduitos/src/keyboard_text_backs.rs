//! Production Backs for the ordinary keyboard-text Play.

use conduit_kernel::scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
    ValueRef,
};

const PORTS: usize = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;

#[derive(Clone, Copy)]
pub(crate) struct KeyboardBack {
    pub(crate) empty: ValueRef,
    pub(crate) pending: Option<RequestId>,
    pub(crate) next: u32,
    pub(crate) maximum: Option<u32>,
}

#[derive(Clone, Copy)]
pub(crate) struct StreamTransformBack {
    pending: Option<RequestId>,
    next: u32,
    allows_empty_output: bool,
}

impl StreamTransformBack {
    pub(crate) const fn new(allows_empty_output: bool) -> Self {
        Self {
            pending: None,
            next: 0,
            allows_empty_output,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct PresentationBack {
    pub(crate) pending: Option<RequestId>,
    pub(crate) next: u32,
}

#[derive(Clone, Copy)]
pub(crate) struct ApplicationBack {
    pub(crate) pending: Option<RequestId>,
    pub(crate) next: u32,
    pub(crate) initial: bool,
    pub(crate) empty: ValueRef,
}

#[derive(Clone, Copy)]
pub(crate) enum PlannedBack {
    Keyboard(KeyboardBack),
    Keymap(StreamTransformBack),
    Upper(StreamTransformBack),
    TextEdit(StreamTransformBack),
    Presentation(PresentationBack),
    Application(ApplicationBack),
}

impl StepOperation<PORTS> for PlannedBack {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        _input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        match self {
            Self::Keyboard(back) => back.step(io),
            Self::Keymap(back) | Self::Upper(back) | Self::TextEdit(back) => back.step(io),
            Self::Presentation(back) => back.step(io),
            Self::Application(back) => back.step(io),
        }
    }

    fn cancel(&mut self) {
        match self {
            Self::Keyboard(back) => back.pending = None,
            Self::Keymap(back) | Self::Upper(back) | Self::TextEdit(back) => back.pending = None,
            Self::Presentation(back) => back.pending = None,
            Self::Application(back) => back.pending = None,
        }
    }
}

impl KeyboardBack {
    fn step(&mut self, io: &mut StepIo<PORTS>) -> StepOutcome {
        if let Some(request) = self.pending {
            let Some((completed, outcome)) = io.host_completion() else {
                return StepOutcome::Await;
            };
            let Some(output) = outcome.output else {
                return fail(60);
            };
            if completed != request
                || outcome.disposition != HostCallDisposition::Completed
                || outcome.failure.is_some()
                || !io.output_ready(PortId(0))
                || io.consume_host_completion().is_err()
                || io.send(PortId(0), output.value).is_err()
            {
                return fail(60);
            }
            self.pending = None;
            let Some(next) = self.next.checked_add(1) else {
                return exhausted(66);
            };
            self.next = next;
            return if self.maximum == Some(self.next) {
                StepOutcome::Complete
            } else {
                StepOutcome::Progress
            };
        }
        if self.maximum == Some(self.next) {
            return StepOutcome::Complete;
        }
        let request = RequestId(self.next);
        if io
            .request_host_call(
                request,
                HostCallId(0),
                BoundedValueRef::new(self.empty, 0).expect("empty input is admitted"),
            )
            .is_err()
        {
            return fail(60);
        }
        self.pending = Some(request);
        StepOutcome::Progress
    }
}

impl StreamTransformBack {
    fn step(&mut self, io: &mut StepIo<PORTS>) -> StepOutcome {
        if let Some(request) = self.pending {
            let Some((completed, outcome)) = io.host_completion() else {
                return StepOutcome::Await;
            };
            if completed != request
                || outcome.disposition != HostCallDisposition::Completed
                || outcome.failure.is_some()
            {
                return fail(63);
            }
            if outcome.output.is_some() && !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            if io.consume_host_completion().is_err() {
                return fail(63);
            }
            match outcome.output {
                Some(output) if io.send(PortId(0), output.value).is_err() => return fail(63),
                None if !self.allows_empty_output => return fail(62),
                _ => {}
            }
            self.pending = None;
            let Some(next) = self.next.checked_add(1) else {
                return exhausted(67);
            };
            self.next = next;
            return StepOutcome::Progress;
        }
        if let Some(value) = io.input(PortId(0)) {
            let Ok(input) = BoundedValueRef::new(value, value.byte_len) else {
                return fail(61);
            };
            let request = RequestId(self.next);
            if io.consume(PortId(0)).is_err()
                || io.request_host_call(request, HostCallId(0), input).is_err()
            {
                return fail(63);
            }
            self.pending = Some(request);
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) {
            if io.consume_closed(PortId(0)).is_err() {
                return fail(63);
            }
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }
}

impl PresentationBack {
    fn step(&mut self, io: &mut StepIo<PORTS>) -> StepOutcome {
        if let Some(request) = self.pending {
            let Some((completed, outcome)) = io.host_completion() else {
                return StepOutcome::Await;
            };
            if completed != request
                || outcome.disposition != HostCallDisposition::Completed
                || outcome.output.is_some()
                || outcome.failure.is_some()
                || io.consume_host_completion().is_err()
            {
                return fail(65);
            }
            self.pending = None;
            let Some(next) = self.next.checked_add(1) else {
                return exhausted(68);
            };
            self.next = next;
            return StepOutcome::Progress;
        }
        if let Some(value) = io.input(PortId(0)) {
            let Ok(input) = BoundedValueRef::new(value, value.byte_len) else {
                return fail(64);
            };
            let request = RequestId(self.next);
            if io.consume(PortId(0)).is_err()
                || io.request_host_call(request, HostCallId(0), input).is_err()
            {
                return fail(65);
            }
            self.pending = Some(request);
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) {
            if io.consume_closed(PortId(0)).is_err() {
                return fail(65);
            }
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }
}

impl ApplicationBack {
    fn step(&mut self, io: &mut StepIo<PORTS>) -> StepOutcome {
        if let Some(request) = self.pending {
            let Some((completed, outcome)) = io.host_completion() else {
                return StepOutcome::Await;
            };
            let Some(output) = outcome.output else {
                return fail(69);
            };
            if completed != request
                || outcome.disposition != HostCallDisposition::Completed
                || outcome.failure.is_some()
                || !io.output_ready(PortId(0))
                || io.consume_host_completion().is_err()
                || io.send(PortId(0), output.value).is_err()
            {
                return fail(69);
            }
            self.pending = None;
            self.initial = false;
            return StepOutcome::Progress;
        }
        if self.initial {
            return self.request(self.empty, io);
        }
        if let Some(value) = io.input(PortId(0)) {
            return self.request_input(value, io);
        }
        if io.input_closed(PortId(0)) {
            if io.consume_closed(PortId(0)).is_err() {
                return fail(69);
            }
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }

    fn request_input(&mut self, value: ValueRef, io: &mut StepIo<PORTS>) -> StepOutcome {
        if io.consume(PortId(0)).is_err() {
            return fail(69);
        }
        self.request(value, io)
    }

    fn request(&mut self, value: ValueRef, io: &mut StepIo<PORTS>) -> StepOutcome {
        let request = RequestId(self.next);
        let Some(next) = self.next.checked_add(1) else {
            return exhausted(70);
        };
        let Ok(input) = BoundedValueRef::new(value, value.byte_len) else {
            return fail(70);
        };
        if io.request_host_call(request, HostCallId(0), input).is_err() {
            return fail(70);
        }
        self.next = next;
        self.pending = Some(request);
        StepOutcome::Progress
    }
}

const fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidLifecycle,
        detail,
    })
}

const fn exhausted(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::StorageExhausted,
        detail,
    })
}
