//! Allocation-free temporal orchestration for one profiled final window.
//! Hosted work is pure bounded computation; no equipment authority is created.
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
    ValueRef,
};

pub const MEASUREMENT_WINDOW_FINALIZE: &[u8] = b"conduit.measurement-window/final@1";
#[derive(Clone, Copy)]
enum Operation {
    Profile,
    Sample,
    Finalize,
}
#[derive(Clone, Copy)]
struct State {
    profile: bool,
    profile_closed: bool,
    samples_closed: bool,
    pending: Option<(RequestId, Operation)>,
    next: u32,
}
pub struct MeasurementWindowStep {
    state: State,
    staged: Option<State>,
    committed: Option<RequestId>,
    committing: Option<RequestId>,
    finalize: ValueRef,
    profile_call: HostCallId,
    sample_call: HostCallId,
    profile_bytes: u32,
    sample_bytes: u32,
    window_bytes: u32,
    cancelled: bool,
}
impl MeasurementWindowStep {
    pub fn new(
        finalize: ValueRef,
        profile_call: HostCallId,
        sample_call: HostCallId,
        bounds: [u32; 3],
    ) -> Self {
        Self {
            state: State {
                profile: false,
                profile_closed: false,
                samples_closed: false,
                pending: None,
                next: 0,
            },
            staged: None,
            committed: None,
            committing: None,
            finalize,
            profile_call,
            sample_call,
            profile_bytes: bounds[0],
            sample_bytes: bounds[1],
            window_bytes: bounds[2],
            cancelled: false,
        }
    }
    /// A wrapper must commit its staged pure provider only after this callback.
    pub fn take_committed_request(&mut self) -> Option<RequestId> {
        self.committed.take()
    }
    fn stage(&mut self, next: State, commit: Option<RequestId>) -> StepOutcome {
        self.staged = Some(next);
        self.committing = commit;
        StepOutcome::Progress
    }
}
fn failure(detail: u16) -> Failure {
    Failure {
        code: FailureCode::InvalidInput,
        detail,
    }
}
impl<const PORTS: usize> StepBack<PORTS> for MeasurementWindowStep {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        self.staged = None;
        self.committing = None;
        if self.cancelled {
            return StepOutcome::Fail(failure(38));
        }
        let mut next = self.state;
        if let Some((request, outcome)) = io.host_completion() {
            let Some((expected, operation)) = next.pending else {
                return StepOutcome::Fail(failure(30));
            };
            if request != expected {
                return StepOutcome::Fail(failure(30));
            }
            if let (HostCallDisposition::Failed, None, Some(reason)) =
                (outcome.disposition, outcome.output, outcome.failure)
            {
                return StepOutcome::Fail(reason);
            }
            if outcome.disposition != HostCallDisposition::Completed || outcome.failure.is_some() {
                return StepOutcome::Fail(failure(30));
            }
            match operation {
                Operation::Finalize => {
                    let Some(output) = outcome.output else {
                        return StepOutcome::Fail(failure(30));
                    };
                    if output.admitted_bytes != self.window_bytes {
                        return StepOutcome::Fail(failure(30));
                    }
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    if io.consume_host_completion().is_err()
                        || io.send(PortId(0), output.value).is_err()
                    {
                        return StepOutcome::Fail(failure(30));
                    }
                    next.pending = None;
                    self.stage(next, Some(request));
                    return StepOutcome::Complete;
                }
                Operation::Profile | Operation::Sample => {
                    if outcome.output.is_some() || io.consume_host_completion().is_err() {
                        return StepOutcome::Fail(failure(30));
                    }
                    if matches!(operation, Operation::Profile) {
                        next.profile = true
                    }
                    next.pending = None;
                    return self.stage(next, Some(request));
                }
            }
        }
        if next.pending.is_some() {
            return StepOutcome::Await;
        }
        let request = RequestId(next.next);
        // Reserve an exact finite node-wide request identity before any input.
        let Some(following) = next.next.checked_add(1) else {
            return StepOutcome::Fail(Failure {
                code: FailureCode::StorageExhausted,
                detail: 35,
            });
        };
        if let Some(value) = io.input(PortId(0)) {
            if next.profile {
                return StepOutcome::Fail(failure(34));
            }
            let Ok(input) = BoundedValueRef::new(value, self.profile_bytes) else {
                return StepOutcome::Fail(failure(31));
            };
            if io.consume(PortId(0)).is_err()
                || io
                    .request_host_call(request, self.profile_call, input)
                    .is_err()
            {
                return StepOutcome::Fail(failure(31));
            }
            next.pending = Some((request, Operation::Profile));
            next.next = following;
            return self.stage(next, None);
        }
        if io.input_closed(PortId(0)) && !next.profile_closed {
            if !next.profile || io.consume_closed(PortId(0)).is_err() {
                return StepOutcome::Fail(failure(34));
            }
            next.profile_closed = true;
            return self.stage(next, None);
        }
        if let Some(value) = io.input(PortId(1)) {
            if !next.profile || next.samples_closed {
                return StepOutcome::Fail(failure(34));
            }
            let Ok(input) = BoundedValueRef::new(value, self.sample_bytes) else {
                return StepOutcome::Fail(failure(32));
            };
            if io.consume(PortId(1)).is_err()
                || io
                    .request_host_call(request, self.sample_call, input)
                    .is_err()
            {
                return StepOutcome::Fail(failure(32));
            }
            next.pending = Some((request, Operation::Sample));
            next.next = following;
            return self.stage(next, None);
        }
        if io.input_closed(PortId(1)) && !next.samples_closed && next.profile && next.profile_closed
        {
            let Ok(input) = BoundedValueRef::new(self.finalize, self.sample_bytes) else {
                return StepOutcome::Fail(failure(33));
            };
            if io.consume_closed(PortId(1)).is_err()
                || io
                    .request_host_call(request, self.sample_call, input)
                    .is_err()
            {
                return StepOutcome::Fail(failure(33));
            }
            next.samples_closed = true;
            next.pending = Some((request, Operation::Finalize));
            next.next = following;
            return self.stage(next, None);
        }
        StepOutcome::Await
    }
    fn step_committed(&mut self) {
        if let Some(next) = self.staged.take() {
            self.state = next;
            self.committed = self.committing.take()
        }
    }
    fn cancel(&mut self) {
        self.cancelled = true;
        self.staged = None;
        self.committed = None;
        self.committing = None;
    }
}

#[cfg(test)]
#[path = "measurement_window_step/tests.rs"]
mod tests;
