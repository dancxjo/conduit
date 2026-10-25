//! Ordinary Crèche Kind and generic kernel Back. The selected Host owns the
//! admitted encounter, canonical Face/MaskShow validation, and make effects.
//! This Back owns no UI, artifact inventory, worker, retry, or scheduler.
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
};

pub const CRECHE_KIND: &str = "creche/prepare";
pub const CRECHE_REQUEST_KIND: &str = "conduit.creche/request@1";
pub const CRECHE_OUTCOME_KIND: &str = "conduit.creche/outcome@1";
pub const CRECHE_REQUEST_BYTES: usize = 32;
pub const CRECHE_OUTCOME_BYTES: usize = 65;

/// Digest of the owner's exact admitted encounter request, including producing
/// Body/Plan/Play, catalogue, selection, authority and work/storage bounds.
/// It is a reference to retained admitted state, never authority by itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CrecheRequest(pub [u8; CRECHE_REQUEST_BYTES]);

/// References retained canonical receipts rather than duplicating artifact
/// truth. `Prepared` proves image preparation only; `Realized` additionally
/// references actual carrier receipts. Neither proves boot, membership or join.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrecheDisposition {
    Prepared,
    Realized,
    Refused,
    Cancelled,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CrecheOutcome {
    pub request: CrecheRequest,
    pub disposition: CrecheDisposition,
    /// Digest of retained exact canonical artifact/carrier receipts or retained
    /// classified refusal/cancellation evidence. Owner verifies custody before
    /// completing the Host Call. Not a pathname or a claim of machine boot.
    pub evidence_digest: [u8; 32],
}
impl CrecheOutcome {
    pub fn encode(self) -> [u8; CRECHE_OUTCOME_BYTES] {
        let mut bytes = [0; CRECHE_OUTCOME_BYTES];
        bytes[..32].copy_from_slice(&self.request.0);
        bytes[32] = match self.disposition {
            CrecheDisposition::Prepared => 0,
            CrecheDisposition::Realized => 1,
            CrecheDisposition::Refused => 2,
            CrecheDisposition::Cancelled => 3,
        };
        bytes[33..].copy_from_slice(&self.evidence_digest);
        bytes
    }
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != CRECHE_OUTCOME_BYTES {
            return None;
        }
        let disposition = match bytes[32] {
            0 => CrecheDisposition::Prepared,
            1 => CrecheDisposition::Realized,
            2 => CrecheDisposition::Refused,
            3 => CrecheDisposition::Cancelled,
            _ => return None,
        };
        Some(Self {
            request: CrecheRequest(bytes[..32].try_into().ok()?),
            disposition,
            evidence_digest: bytes[33..].try_into().ok()?,
        })
    }
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
pub struct CrecheBack {
    call: HostCallId,
    request: Option<CrecheRequest>,
    staged_request: Option<CrecheRequest>,
    phase: Phase,
    staged: Option<Phase>,
}
impl CrecheBack {
    /// Installed preparation cannot know the actual producing Play yet. Pin the
    /// first exact typed request at successful kernel commit. The selected Host
    /// owner must validate it against actual admitted execution before effects.
    pub fn for_admitted_host(call: HostCallId) -> Self {
        Self {
            call,
            request: None,
            staged_request: None,
            phase: Phase::Ready,
            staged: None,
        }
    }
    pub fn new(call: HostCallId, request: CrecheRequest) -> Self {
        Self {
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
impl<const PORTS: usize> StepBack<PORTS> for CrecheBack {
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
                let Ok(request_bytes) = <[u8; CRECHE_REQUEST_BYTES]>::try_from(encoded) else {
                    return fail(FailureCode::InvalidInput, 1);
                };
                let request = CrecheRequest(request_bytes);
                if input.byte_len as usize != CRECHE_REQUEST_BYTES
                    || self.request.is_some_and(|expected| expected != request)
                {
                    return fail(FailureCode::InvalidInput, 1);
                }
                let Ok(input) = BoundedValueRef::new(input, CRECHE_REQUEST_BYTES as u32) else {
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
                if output.value.byte_len as usize != CRECHE_OUTCOME_BYTES
                    || !CrecheOutcome::decode(encoded)
                        .is_some_and(|v| Some(v.request) == self.request)
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
