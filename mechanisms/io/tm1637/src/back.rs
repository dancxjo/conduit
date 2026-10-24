//! Finite packet sink under the ordinary kernel. Pin effects remain in the
//! selected Host Call; this Back owns only its committed protocol phase.
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
};
#[derive(Clone, Copy)]
enum Phase {
    Ready(u32),
    Pending { request: RequestId, next: u32 },
    Cancelled,
}
pub struct WriteBack {
    phase: Phase,
    staged: Option<Phase>,
    maximum_operations: u32,
}
impl WriteBack {
    /// Root admits this finite operation ceiling alongside the selected output
    /// provider and Host Call scope. Construction starts no electrical work.
    pub const fn new(maximum_operations: u32) -> Self {
        Self {
            phase: Phase::Ready(0),
            staged: None,
            maximum_operations,
        }
    }
}
impl<const P: usize> StepBack<P> for WriteBack {
    fn step(&mut self, io: &mut StepIo<P>, _: &StepInputBytes<'_, P>) -> StepOutcome {
        self.staged = None;
        match self.phase {
            Phase::Cancelled => fail(FailureCode::Cancelled, 0),
            Phase::Ready(next) => {
                if io.host_completion().is_some() {
                    return fail(FailureCode::InvalidLifecycle, 1);
                }
                if io.input_abnormal(PortId(0)).is_some() {
                    return fail(FailureCode::InvalidInput, 2);
                }
                if let Some(value) = io.input(PortId(0)) {
                    if next >= self.maximum_operations {
                        return fail(FailureCode::StorageExhausted, 3);
                    }
                    let Ok(input) = BoundedValueRef::new(value, crate::MAXIMUM_BYTES) else {
                        return fail(FailureCode::InvalidInput, 4);
                    };
                    let Some(after) = next.checked_add(1) else {
                        return fail(FailureCode::IdentityCapacityExhausted, 5);
                    };
                    let request = RequestId(next);
                    if io.consume(PortId(0)).is_err()
                        || io.request_host_call(request, HostCallId(0), input).is_err()
                    {
                        return fail(FailureCode::InvalidLifecycle, 6);
                    }
                    self.staged = Some(Phase::Pending {
                        request,
                        next: after,
                    });
                    return StepOutcome::Progress;
                }
                if io.input_closed(PortId(0)) {
                    if io.consume_closed(PortId(0)).is_err() {
                        return fail(FailureCode::InvalidLifecycle, 7);
                    }
                    return StepOutcome::Complete;
                }
                StepOutcome::Await
            }
            Phase::Pending { request, next } => {
                let Some((observed, outcome)) = io.host_completion() else {
                    return StepOutcome::Await;
                };
                if request != observed {
                    return fail(FailureCode::InvalidLifecycle, 8);
                }
                match (outcome.disposition, outcome.output, outcome.failure) {
                    (HostCallDisposition::Completed, None, None) => {
                        if io.consume_host_completion().is_err() {
                            return fail(FailureCode::InvalidLifecycle, 9);
                        }
                        self.staged = Some(Phase::Ready(next));
                        StepOutcome::Progress
                    }
                    (
                        HostCallDisposition::Failed | HostCallDisposition::Denied,
                        None,
                        Some(reason),
                    ) => StepOutcome::Fail(reason),
                    (HostCallDisposition::Cancelled, None, None) => fail(FailureCode::Cancelled, 0),
                    _ => fail(FailureCode::InvalidLifecycle, 10),
                }
            }
        }
    }
    fn step_committed(&mut self) {
        if let Some(phase) = self.staged.take() {
            self.phase = phase;
        }
    }
    fn cancel(&mut self) {
        self.staged = None;
        self.phase = Phase::Cancelled;
    }
}
const fn fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}
