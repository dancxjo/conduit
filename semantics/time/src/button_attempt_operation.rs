//! Shared kernel operation for a finite pressed-button timing attempt.
use alloc::vec::Vec;
use conduit_kernel::{
    scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, HostCallOutcome,
    OperationAction, OperationInput, PortId, RequestId, ValueRef,
};
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pending {
    Observe,
    Deadline,
}

pub struct TimedButtonAttemptOperation {
    maximum_input_bytes: u32,
    durations: Vec<ValueRef>,
    released: Vec<ValueRef>,
    next_duration: usize,
    next_request: u32,
    pending: Option<(RequestId, Pending)>,
    cancellation: Option<RequestId>,
    queued_transition: Option<ValueRef>,
    accepted_transitions: u64,
    maximum_transitions: u64,
    retain_resumed: bool,
    emitted_attempt: bool,
    completed_attempt: bool,
}

impl<const PORTS: usize> StepOperation<PORTS> for TimedButtonAttemptOperation {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            let Some((expected, pending)) = self.pending else {
                return attempt_step_fail(FailureCode::InvalidLifecycle, 272);
            };
            if request != expected {
                return attempt_step_fail(FailureCode::InvalidLifecycle, 272);
            }
            match pending {
                Pending::Deadline => match (outcome.disposition, outcome.output, outcome.failure) {
                    (HostCallDisposition::Cancelled, None, None) => {
                        io.consume_host_completion()
                            .expect("observed button deadline cancellation");
                        self.pending = None;
                        self.cancellation = None;
                        return StepOutcome::Progress;
                    }
                    (HostCallDisposition::Completed, None, None) => {
                        return attempt_step_fail(FailureCode::HostCallFailed, 4)
                    }
                    (HostCallDisposition::Failed, None, Some(failure)) => {
                        return StepOutcome::Fail(failure)
                    }
                    _ => return attempt_step_fail(FailureCode::InvalidLifecycle, 275),
                },
                Pending::Observe => {
                    match (
                        outcome.disposition,
                        outcome.output,
                        outcome.failure,
                        input_bytes.host_output(),
                    ) {
                        (HostCallDisposition::Completed, None, None, None)
                            if self.next_duration > 0 =>
                        {
                            io.consume_host_completion()
                                .expect("observed empty button observation");
                            self.pending = None;
                            if let Err(outcome) = self.request_deadline_step(io) {
                                return outcome;
                            }
                            return StepOutcome::Progress;
                        }
                        (HostCallDisposition::Completed, None, None, None) => {
                            io.consume_host_completion()
                                .expect("observed empty button observation");
                            self.pending = None;
                            return StepOutcome::Progress;
                        }
                        (HostCallDisposition::Completed, Some(_), None, Some([0])) => {
                            io.consume_host_completion()
                                .expect("observed incomplete button attempt");
                            self.pending = None;
                            if let Err(outcome) = self.request_deadline_step(io) {
                                return outcome;
                            }
                            return StepOutcome::Progress;
                        }
                        (HostCallDisposition::Completed, Some(output), None, Some(_)) => {
                            if !io.output_ready(PortId(0)) {
                                return StepOutcome::Await;
                            }
                            io.consume_host_completion()
                                .expect("observed completed button attempt");
                            io.send(PortId(0), output.value)
                                .expect("ready button-attempt output");
                            self.pending = None;
                            self.completed_attempt = true;
                            self.accepted_transitions = 0;
                            self.next_duration = 0;
                            return StepOutcome::Progress;
                        }
                        (HostCallDisposition::Cancelled, _, _, _) => {
                            return attempt_step_fail(FailureCode::Cancelled, 0)
                        }
                        (HostCallDisposition::Failed, None, Some(failure), _) => {
                            return StepOutcome::Fail(failure)
                        }
                        _ => return attempt_step_fail(FailureCode::InvalidLifecycle, 273),
                    }
                }
            }
        }
        if let Some(value) = io.input(PortId(0)) {
            if self.accepted_transitions >= self.maximum_transitions {
                return attempt_step_fail(FailureCode::StorageExhausted, 1);
            }
            match self.pending {
                None => {
                    io.consume(PortId(0)).expect("present button transition");
                    self.accepted_transitions += 1;
                    self.request_observation_step(io, value);
                    return StepOutcome::Progress;
                }
                Some((request, Pending::Deadline)) if self.queued_transition.is_none() => {
                    if self.cancellation == Some(request) {
                        return StepOutcome::Await;
                    }
                    io.cancel_host_call(request)
                        .expect("cancel superseded button deadline");
                    self.cancellation = Some(request);
                    return StepOutcome::Progress;
                }
                _ => return attempt_step_fail(FailureCode::InvalidLifecycle, 271),
            }
        }
        if io.input_closed(PortId(0)) && self.pending.is_none() {
            io.consume_closed(PortId(0))
                .expect("observed button-attempt closure");
            while let Some(value) = self.durations.pop() {
                io.discard(value)
                    .expect("bounded button-attempt duration release");
            }
            return if self.completed_attempt && self.accepted_transitions == 0 {
                StepOutcome::Complete
            } else {
                attempt_step_fail(FailureCode::InvalidInput, 2)
            };
        }
        StepOutcome::Await
    }

    fn accepts_input_while_host_call_pending(&self) -> bool {
        true
    }

    fn retains_host_call_input(&self, _request: RequestId, value: ValueRef) -> bool {
        self.durations.contains(&value)
    }

    fn cancel(&mut self) {
        TimedButtonAttemptOperation::cancel(self);
    }
}

impl TimedButtonAttemptOperation {
    fn request_observation_step<const PORTS: usize>(
        &mut self,
        io: &mut StepIo<PORTS>,
        value: ValueRef,
    ) {
        let request = self.next_request();
        io.request_host_call(
            request,
            HostCallId(1),
            BoundedValueRef::new(value, self.maximum_input_bytes)
                .expect("button transition is bounded by its exact port"),
        )
        .expect("button observation Host Call");
        self.pending = Some((request, Pending::Observe));
    }

    fn request_deadline_step<const PORTS: usize>(
        &mut self,
        io: &mut StepIo<PORTS>,
    ) -> Result<(), StepOutcome> {
        let Some(value) = self.durations.first().copied() else {
            return Err(attempt_step_fail(FailureCode::StorageExhausted, 276));
        };
        self.next_duration = 1;
        let request = self.next_request();
        io.request_host_call(
            request,
            HostCallId(0),
            BoundedValueRef::new(value, 8).expect("deadline duration is exactly eight bytes"),
        )
        .expect("button deadline Host Call");
        self.pending = Some((request, Pending::Deadline));
        Ok(())
    }
}

const fn attempt_step_fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}

impl TimedButtonAttemptOperation {
    /// Construct before Play from one admitted duration value per transition.
    /// The caller validates the finite transition limit and owns value admission.
    pub fn from_prepared_durations(
        durations: Vec<ValueRef>,
        maximum_transitions: u64,
        maximum_input_bytes: u32,
    ) -> Self {
        TimedButtonAttemptOperation {
            maximum_input_bytes,
            durations,
            released: Vec::with_capacity(maximum_transitions as usize + 1),
            next_duration: 0,
            next_request: 0,
            pending: None,
            cancellation: None,
            queued_transition: None,
            accepted_transitions: 0,
            maximum_transitions,
            retain_resumed: false,
            emitted_attempt: false,
            completed_attempt: false,
        }
    }

    pub fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub fn resume(&mut self, input: OperationInput) -> OperationAction {
        self.retain_resumed = false;
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if !self.emitted_attempt && self.accepted_transitions < self.maximum_transitions => {
                self.accepted_transitions += 1;
                match self.pending {
                    None => self.request_observation(value),
                    Some((request, Pending::Deadline)) if self.queued_transition.is_none() => {
                        self.retain_resumed = true;
                        self.queued_transition = Some(value);
                        self.cancellation = Some(request);
                        OperationAction::Await
                    }
                    _ => fail(FailureCode::InvalidLifecycle, 271),
                }
            }
            OperationInput::Value {
                port: PortId(0), ..
            } if self.accepted_transitions >= self.maximum_transitions => {
                fail(FailureCode::StorageExhausted, 1)
            }
            OperationInput::HostCallCompleted { request, outcome }
                if self.pending == Some((request, Pending::Deadline)) =>
            {
                self.resume_deadline(request, outcome)
            }
            OperationInput::Closed { port: PortId(0) }
                if self.pending.is_none()
                    && self.completed_attempt
                    && self.accepted_transitions == 0 =>
            {
                self.release_unused_durations();
                OperationAction::Complete
            }
            OperationInput::Closed { port: PortId(0) } if self.pending.is_none() => {
                self.release_unused_durations();
                fail(FailureCode::InvalidInput, 2)
            }
            _ => fail(FailureCode::InvalidLifecycle, 272),
        }
    }

    pub fn resume_host_call(
        &mut self,
        request: RequestId,
        outcome: HostCallOutcome,
        canonical: Option<&[u8]>,
    ) -> OperationAction {
        if self.pending != Some((request, Pending::Observe)) {
            return self.resume(OperationInput::HostCallCompleted { request, outcome });
        }
        self.pending = None;
        match (
            outcome.disposition,
            outcome.output,
            outcome.failure,
            canonical,
        ) {
            (HostCallDisposition::Completed, None, None, None) if self.next_duration > 0 => {
                self.request_deadline()
            }
            (HostCallDisposition::Completed, None, None, None) => OperationAction::Await,
            (HostCallDisposition::Completed, Some(_), None, Some([0])) => self.request_deadline(),
            (HostCallDisposition::Completed, Some(output), None, Some(_)) => {
                self.emitted_attempt = true;
                OperationAction::Emit {
                    port: PortId(0),
                    value: output.value,
                }
            }
            (HostCallDisposition::Cancelled, _, _, _) => fail(FailureCode::Cancelled, 0),
            (HostCallDisposition::Failed, None, Some(failure), _) => OperationAction::Fail(failure),
            _ => fail(FailureCode::InvalidLifecycle, 273),
        }
    }

    pub fn advance(&mut self) -> OperationAction {
        if self.emitted_attempt {
            self.emitted_attempt = false;
            self.completed_attempt = true;
            self.accepted_transitions = 0;
            self.next_duration = 0;
        }
        OperationAction::Await
    }

    pub fn cancel(&mut self) {
        self.pending = None;
        self.cancellation = None;
        self.queued_transition = None;
    }

    pub fn retains_resumed_value(&self) -> bool {
        self.retain_resumed
    }

    pub fn take_released_value(&mut self) -> Option<ValueRef> {
        self.released.pop()
    }

    pub fn take_host_call_cancellation(&mut self) -> Option<RequestId> {
        self.cancellation.take()
    }

    pub fn allocation_capacity(&self) -> usize {
        self.durations.capacity() + self.released.capacity()
    }

    pub fn retains_host_call_input(&self, value: ValueRef) -> bool {
        self.durations.contains(&value)
    }

    fn resume_deadline(&mut self, request: RequestId, outcome: HostCallOutcome) -> OperationAction {
        self.pending = None;
        match (outcome.disposition, outcome.output, outcome.failure) {
            (HostCallDisposition::Cancelled, None, None) => {
                self.queued_transition.take().map_or_else(
                    || fail(FailureCode::InvalidLifecycle, 274),
                    |value| self.request_observation(value),
                )
            }
            (HostCallDisposition::Completed, None, None) => {
                self.release_unused_durations();
                fail(FailureCode::HostCallFailed, 4)
            }
            (HostCallDisposition::Failed, None, Some(failure)) => OperationAction::Fail(failure),
            _ => {
                let _ = request;
                fail(FailureCode::InvalidLifecycle, 275)
            }
        }
    }

    fn request_observation(&mut self, value: ValueRef) -> OperationAction {
        let request = self.next_request();
        self.pending = Some((request, Pending::Observe));
        OperationAction::RequestHostCall {
            request,
            operation: HostCallId(1),
            input: BoundedValueRef::new(value, self.maximum_input_bytes)
                .expect("button transition is bounded by its exact port"),
        }
    }

    fn request_deadline(&mut self) -> OperationAction {
        let Some(value) = self.durations.first().copied() else {
            return fail(FailureCode::StorageExhausted, 276);
        };
        self.next_duration = 1;
        let request = self.next_request();
        self.pending = Some((request, Pending::Deadline));
        OperationAction::RequestHostCall {
            request,
            operation: HostCallId(0),
            input: BoundedValueRef::new(value, 8)
                .expect("deadline duration is exactly eight bytes"),
        }
    }

    fn next_request(&mut self) -> RequestId {
        let request = RequestId(self.next_request);
        self.next_request = self.next_request.saturating_add(1);
        request
    }

    fn release_unused_durations(&mut self) {
        self.released.append(&mut self.durations);
    }
}

impl conduit_kernel::Operation for TimedButtonAttemptOperation {
    fn start(&mut self) -> OperationAction {
        Self::start(self)
    }
    fn resume(&mut self, input: OperationInput) -> OperationAction {
        Self::resume(self, input)
    }
    fn resume_host_call(
        &mut self,
        request: RequestId,
        outcome: HostCallOutcome,
        canonical: Option<&[u8]>,
    ) -> OperationAction {
        Self::resume_host_call(self, request, outcome, canonical)
    }
    fn advance(&mut self) -> OperationAction {
        Self::advance(self)
    }
    fn cancel(&mut self) {
        Self::cancel(self)
    }
    fn take_host_call_cancellation(&mut self) -> Option<RequestId> {
        Self::take_host_call_cancellation(self)
    }
    fn take_released_value(&mut self) -> Option<ValueRef> {
        Self::take_released_value(self)
    }
    fn retains_resumed_value(&self) -> bool {
        Self::retains_resumed_value(self)
    }
    fn accepts_input_while_host_call_pending(&self) -> bool {
        true
    }
    fn retains_host_call_input(&self, _request: RequestId, value: ValueRef) -> bool {
        Self::retains_host_call_input(self, value)
    }
}
fn fail(code: FailureCode, detail: u16) -> OperationAction {
    OperationAction::Fail(Failure { code, detail })
}

#[cfg(test)]
#[path = "button_attempt_operation_tests.rs"]
mod tests;
