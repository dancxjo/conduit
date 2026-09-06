//! Shared kernel operation for a finite pressed-button timing attempt.
use alloc::vec::Vec;
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    HostOperationOutcome, OperationAction, OperationInput, PortId, RequestId, ValueRef,
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
    completed: bool,
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
            completed: false,
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
            } if !self.completed && self.accepted_transitions < self.maximum_transitions => {
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
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some((request, Pending::Deadline)) =>
            {
                self.resume_deadline(request, outcome)
            }
            OperationInput::Closed { port: PortId(0) } if self.pending.is_none() => {
                self.release_unused_durations();
                fail(FailureCode::InvalidInput, 2)
            }
            _ => fail(FailureCode::InvalidLifecycle, 272),
        }
    }

    pub fn resume_host_operation(
        &mut self,
        request: RequestId,
        outcome: HostOperationOutcome,
        canonical: Option<&[u8]>,
    ) -> OperationAction {
        if self.pending != Some((request, Pending::Observe)) {
            return self.resume(OperationInput::HostOperationCompleted { request, outcome });
        }
        self.pending = None;
        match (
            outcome.disposition,
            outcome.output,
            outcome.failure,
            canonical,
        ) {
            (HostOperationDisposition::Completed, None, None, None) => OperationAction::Await,
            (HostOperationDisposition::Completed, Some(_), None, Some([0])) => {
                self.request_deadline()
            }
            (HostOperationDisposition::Completed, Some(output), None, Some(_)) => {
                self.completed = true;
                self.release_unused_durations();
                OperationAction::Emit {
                    port: PortId(0),
                    value: output.value,
                }
            }
            (HostOperationDisposition::Cancelled, _, _, _) => fail(FailureCode::Cancelled, 0),
            (HostOperationDisposition::Failed, None, Some(failure), _) => {
                OperationAction::Fail(failure)
            }
            _ => fail(FailureCode::InvalidLifecycle, 273),
        }
    }

    pub fn advance(&mut self) -> OperationAction {
        if self.completed {
            OperationAction::Complete
        } else {
            OperationAction::Await
        }
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

    pub fn take_host_operation_cancellation(&mut self) -> Option<RequestId> {
        self.cancellation.take()
    }

    pub fn allocation_capacity(&self) -> usize {
        self.durations.capacity() + self.released.capacity()
    }

    fn resume_deadline(
        &mut self,
        request: RequestId,
        outcome: HostOperationOutcome,
    ) -> OperationAction {
        self.pending = None;
        match (outcome.disposition, outcome.output, outcome.failure) {
            (HostOperationDisposition::Cancelled, None, None) => {
                self.queued_transition.take().map_or_else(
                    || fail(FailureCode::InvalidLifecycle, 274),
                    |value| self.request_observation(value),
                )
            }
            (HostOperationDisposition::Completed, None, None) => {
                self.release_unused_durations();
                fail(FailureCode::HostOperationFailed, 4)
            }
            (HostOperationDisposition::Failed, None, Some(failure)) => {
                OperationAction::Fail(failure)
            }
            _ => {
                let _ = request;
                fail(FailureCode::InvalidLifecycle, 275)
            }
        }
    }

    fn request_observation(&mut self, value: ValueRef) -> OperationAction {
        let request = self.next_request();
        self.pending = Some((request, Pending::Observe));
        OperationAction::RequestHostOperation {
            request,
            operation: HostOperationId(1),
            input: BoundedValueRef::new(value, self.maximum_input_bytes)
                .expect("button transition is bounded by its exact port"),
        }
    }

    fn request_deadline(&mut self) -> OperationAction {
        let Some(value) = self.durations.get(self.next_duration).copied() else {
            return fail(FailureCode::StorageExhausted, 276);
        };
        self.next_duration += 1;
        let request = self.next_request();
        self.pending = Some((request, Pending::Deadline));
        OperationAction::RequestHostOperation {
            request,
            operation: HostOperationId(0),
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
        self.released
            .extend(self.durations.drain(self.next_duration..));
    }
}

impl conduit_kernel::Operation for TimedButtonAttemptOperation {
    fn start(&mut self) -> OperationAction {
        Self::start(self)
    }
    fn resume(&mut self, input: OperationInput) -> OperationAction {
        Self::resume(self, input)
    }
    fn resume_host_operation(
        &mut self,
        request: RequestId,
        outcome: HostOperationOutcome,
        canonical: Option<&[u8]>,
    ) -> OperationAction {
        Self::resume_host_operation(self, request, outcome, canonical)
    }
    fn advance(&mut self) -> OperationAction {
        Self::advance(self)
    }
    fn cancel(&mut self) {
        Self::cancel(self)
    }
    fn take_host_operation_cancellation(&mut self) -> Option<RequestId> {
        Self::take_host_operation_cancellation(self)
    }
    fn take_released_value(&mut self) -> Option<ValueRef> {
        Self::take_released_value(self)
    }
    fn retains_resumed_value(&self) -> bool {
        Self::retains_resumed_value(self)
    }
    fn accepts_input_while_host_operation_pending(&self) -> bool {
        true
    }
}
fn fail(code: FailureCode, detail: u16) -> OperationAction {
    OperationAction::Fail(Failure { code, detail })
}

#[cfg(test)]
#[path = "button_attempt_operation_tests.rs"]
mod tests;
