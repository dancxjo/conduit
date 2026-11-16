//! One finite digest-referenced Host Call transaction, shared by preparation
//! operations. Protocols own completion meaning; this Back owns only correlation,
//! commit, pressure and cancellation. It never dispatches or retries effects.
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
};

pub(crate) const REQUEST_BYTES: usize = 32;
pub(crate) trait DigestCompletion {
    const OUTCOME_BYTES: usize;
    fn matches(bytes: &[u8], request: [u8; REQUEST_BYTES]) -> bool;
}

#[derive(Clone, Copy)]
enum Phase {
    Ready,
    Waiting,
    Finished,
    Cancelled,
}
/// Prepared with an admitted call binding. The Host owner must validate the
/// exact request before effects. State advances after transactional commit;
/// pressure cannot repeat a call.
pub(crate) struct DigestCallBack<P: DigestCompletion> {
    protocol: core::marker::PhantomData<P>,
    call: HostCallId,
    request: Option<[u8; 32]>,
    staged_request: Option<[u8; 32]>,
    phase: Phase,
    staged: Option<Phase>,
}
impl<P: DigestCompletion> DigestCallBack<P> {
    /// Installed preparation cannot know the actual producing Play yet. Pin the
    /// first exact typed request at successful kernel commit. The selected Host
    /// owner must validate it against actual admitted execution before effects.
    pub fn for_admitted_host(call: HostCallId) -> Self {
        Self {
            protocol: core::marker::PhantomData,
            call,
            request: None,
            staged_request: None,
            phase: Phase::Ready,
            staged: None,
        }
    }
    pub fn new(call: HostCallId, request: [u8; 32]) -> Self {
        Self {
            protocol: core::marker::PhantomData,
            call,
            request: Some(request),
            staged_request: None,
            phase: Phase::Ready,
            staged: None,
        }
    }
}
fn fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}
impl<P: DigestCompletion, const PORTS: usize> StepBack<PORTS> for DigestCallBack<P> {
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        match self.phase {
            Phase::Cancelled => fail(FailureCode::Cancelled, 0),
            Phase::Finished => StepOutcome::Complete,
            Phase::Ready => {
                let Some(input) = io.input(PortId(0)) else {
                    return if io.input_closed(PortId(0)) {
                        fail(FailureCode::InvalidInput, 8)
                    } else {
                        StepOutcome::Await
                    };
                };
                let Some(encoded) = bytes.input(PortId(0)) else {
                    return fail(FailureCode::InvalidInput, 1);
                };
                let Ok(request_bytes) = <[u8; REQUEST_BYTES]>::try_from(encoded) else {
                    return fail(FailureCode::InvalidInput, 1);
                };
                let request = request_bytes;
                if input.byte_len as usize != REQUEST_BYTES
                    || self.request.is_some_and(|expected| expected != request)
                {
                    return fail(FailureCode::InvalidInput, 1);
                }
                let Ok(input) = BoundedValueRef::new(input, REQUEST_BYTES as u32) else {
                    return fail(FailureCode::InvalidInput, 2);
                };
                if io.consume(PortId(0)).is_err()
                    || io
                        .request_host_call(RequestId(0), self.call, input)
                        .is_err()
                {
                    return fail(FailureCode::InvalidLifecycle, 3);
                }
                self.staged_request = Some(request);
                self.staged = Some(Phase::Waiting);
                StepOutcome::Progress
            }
            Phase::Waiting => {
                let Some((request, outcome)) = io.host_completion() else {
                    return StepOutcome::Await;
                };
                if request != RequestId(0) {
                    return fail(FailureCode::InvalidLifecycle, 4);
                }
                if outcome.disposition != HostCallDisposition::Completed {
                    return StepOutcome::Fail(outcome.failure.unwrap_or(Failure {
                        code: match outcome.disposition {
                            HostCallDisposition::Denied => FailureCode::HostCallDenied,
                            HostCallDisposition::Cancelled => FailureCode::Cancelled,
                            _ => FailureCode::HostCallFailed,
                        },
                        detail: 0,
                    }));
                }
                if let Some(failure) = outcome.failure {
                    return StepOutcome::Fail(failure);
                }
                let (Some(output), Some(encoded)) = (outcome.output, bytes.host_output()) else {
                    return fail(FailureCode::InvalidInput, 5);
                };
                if output.value.byte_len as usize != P::OUTCOME_BYTES
                    || !self
                        .request
                        .is_some_and(|request| P::matches(encoded, request))
                {
                    return fail(FailureCode::InvalidInput, 6);
                }
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                if io.consume_host_completion().is_err()
                    || io.send(PortId(0), output.value).is_err()
                {
                    return fail(FailureCode::InvalidLifecycle, 7);
                }
                self.staged = Some(Phase::Finished);
                StepOutcome::Progress
            }
        }
    }
    fn step_committed(&mut self) {
        if let Some(request) = self.staged_request.take() {
            self.request = Some(request);
        }
        if let Some(phase) = self.staged.take() {
            self.phase = phase;
        }
    }
    fn cancel(&mut self) {
        self.staged_request = None;
        self.staged = None;
        self.phase = Phase::Cancelled;
    }
}
