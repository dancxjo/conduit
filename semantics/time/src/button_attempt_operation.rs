//! Shared kernel operation for a finite pressed-button timing attempt.
use alloc::vec::Vec;
use conduit_kernel::{
    scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
    ValueRef,
};
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pending {
    Observe,
    Deadline,
}

pub struct TimedButtonAttemptOperation {
    maximum_input_bytes: u32,
    durations: Vec<ValueRef>,
    next_duration: usize,
    next_request: u32,
    pending: Option<(RequestId, Pending)>,
    cancellation: Option<RequestId>,
    accepted_transitions: u64,
    maximum_transitions: u64,
    completed_attempt: bool,
    input_closed: bool,
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
        if self.input_closed {
            if !self.completed_attempt || self.accepted_transitions != 0 {
                return attempt_step_fail(FailureCode::InvalidInput, 2);
            }
            if let Some(value) = self.durations.pop() {
                io.discard(value)
                    .expect("one bounded button-attempt duration release");
                return StepOutcome::Progress;
            }
            return StepOutcome::Complete;
        }
        if let Some(value) = io.input(PortId(0)) {
            if self.accepted_transitions >= self.maximum_transitions {
                return attempt_step_fail(FailureCode::StorageExhausted, 1);
            }
            match self.pending {
                None => {
                    io.consume(PortId(0)).expect("present button transition");
                    self.accepted_transitions += 1;
                    if let Err(outcome) = self.request_observation_step(io, value) {
                        return outcome;
                    }
                    return StepOutcome::Progress;
                }
                Some((request, Pending::Deadline)) => {
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
            self.input_closed = true;
            return if self.completed_attempt && self.accepted_transitions == 0 {
                StepOutcome::Progress
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
    ) -> Result<(), StepOutcome> {
        let request = self.next_request()?;
        io.request_host_call(
            request,
            HostCallId(1),
            BoundedValueRef::new(value, self.maximum_input_bytes)
                .expect("button transition is bounded by its exact port"),
        )
        .expect("button observation Host Call");
        self.pending = Some((request, Pending::Observe));
        Ok(())
    }

    fn request_deadline_step<const PORTS: usize>(
        &mut self,
        io: &mut StepIo<PORTS>,
    ) -> Result<(), StepOutcome> {
        let Some(value) = self.durations.first().copied() else {
            return Err(attempt_step_fail(FailureCode::StorageExhausted, 276));
        };
        self.next_duration = 1;
        let request = self.next_request()?;
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
            next_duration: 0,
            next_request: 0,
            pending: None,
            cancellation: None,
            accepted_transitions: 0,
            maximum_transitions,
            completed_attempt: false,
            input_closed: false,
        }
    }

    pub fn cancel(&mut self) {
        self.pending = None;
        self.cancellation = None;
    }

    pub fn allocation_capacity(&self) -> usize {
        self.durations.capacity()
    }

    pub fn retains_host_call_input(&self, value: ValueRef) -> bool {
        self.durations.contains(&value)
    }

    fn next_request(&mut self) -> Result<RequestId, StepOutcome> {
        let request = RequestId(self.next_request);
        self.next_request = self
            .next_request
            .checked_add(1)
            .ok_or_else(|| attempt_step_fail(FailureCode::IdentityCapacityExhausted, 277))?;
        Ok(request)
    }
}

#[cfg(test)]
#[path = "button_attempt_operation_tests.rs"]
mod tests;
