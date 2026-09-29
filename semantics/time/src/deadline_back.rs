//! One-shot semantic deadline producing a cancellation request.

use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
    ValueRef,
};

pub struct CancellationDeadlineBack {
    duration: Option<ValueRef>,
    request_value: Option<ValueRef>,
    pending: bool,
    armed: bool,
    input_closed: bool,
    closing_unarmed: bool,
}

impl CancellationDeadlineBack {
    pub const fn prepare(duration: ValueRef, request_value: ValueRef) -> Self {
        Self {
            duration: Some(duration),
            request_value: Some(request_value),
            pending: false,
            armed: false,
            input_closed: false,
            closing_unarmed: false,
        }
    }
    pub const fn allocation_capacity(&self) -> usize {
        0
    }
    fn finish_unarmed<const PORTS: usize>(&mut self, io: &mut StepIo<PORTS>) -> StepOutcome {
        if let Some(value) = self.duration.take() {
            io.discard(value).expect("unused deadline duration");
            return StepOutcome::Progress;
        }
        if let Some(value) = self.request_value.take() {
            io.discard(value).expect("unused cancellation request");
            return StepOutcome::Progress;
        }
        self.closing_unarmed = false;
        StepOutcome::Complete
    }
}

impl<const PORTS: usize> StepBack<PORTS> for CancellationDeadlineBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.closing_unarmed {
            return self.finish_unarmed(io);
        }
        if let Some((request, outcome)) = io.host_completion() {
            if !self.pending
                || request != RequestId(1)
                || outcome.disposition != HostCallDisposition::Completed
                || outcome.output.is_some()
                || outcome.failure.is_some()
            {
                return outcome.failure.map_or_else(|| fail(783), StepOutcome::Fail);
            }
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            let Some(value) = self.request_value.take() else {
                return fail(784);
            };
            io.consume_host_completion()
                .expect("observed time/deadline completion");
            io.send(PortId(0), value)
                .expect("ready cancellation-request output");
            self.pending = false;
            self.duration = None;
            return StepOutcome::Complete;
        }
        if io.input(PortId(0)).is_some() {
            if self.armed || self.input_closed || inputs.input(PortId(0)) != Some(&[]) {
                return fail(785);
            }
            io.consume(PortId(0)).expect("present Unit deadline arm");
            let Some(duration) = self.duration else {
                return fail(786);
            };
            io.request_host_call(
                RequestId(1),
                HostCallId(0),
                BoundedValueRef::new(duration, 8).expect("exact deadline duration"),
            )
            .expect("deadline Host Call");
            self.armed = true;
            self.pending = true;
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) && !self.input_closed {
            io.consume_closed(PortId(0))
                .expect("observed deadline arm closure");
            self.input_closed = true;
            if !self.armed {
                self.closing_unarmed = true;
                return self.finish_unarmed(io);
            }
            return StepOutcome::Progress;
        }
        StepOutcome::Await
    }
    fn accepts_input_while_host_call_pending(&self) -> bool {
        true
    }
    fn cancel(&mut self) {
        self.pending = false;
        self.duration = None;
        self.request_value = None;
    }
}

const fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidLifecycle,
        detail,
    })
}
