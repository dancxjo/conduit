//! Same-Body member Seed preparation over the ordinary kernel boundary.
//! The selected owner retains checked configuration, current invitation,
//! authority, bounded make effects and evidence. This Back cannot create a Body,
//! issue an invitation, admit a Part, deploy an artifact or observe a Boot.
use crate::preparation_call::{DigestCallBack, DigestCompletion};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    HostCallId,
};

pub const STEM_KIND: &str = "stem/prepare";
pub const STEM_REQUEST_KIND: &str = "conduit.stem/request@1";
pub const STEM_OUTCOME_KIND: &str = "conduit.stem/outcome@1";
pub const STEM_REQUEST_BYTES: usize = 32;
pub const STEM_OUTCOME_BYTES: usize = 65;

/// Exact reference to retained admission state bound to the actual producing
/// Body/Plan/Play, reviewed Host selection, invitation and finite effect grants.
/// A digest is neither possession of authority nor proof that state is current.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StemRequest(pub [u8; STEM_REQUEST_BYTES]);

/// Member Seed preparation and classified refusal only. There is deliberately no
/// Boot, membership or deployment-success disposition on this operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum StemDisposition {
    Prepared = 0,
    UnavailableBuilder = 1,
    UnsupportedTarget = 2,
    StaleInvitation = 3,
    /// Preparation authority refused; target admission is a later decision.
    Denied = 4,
    Cancelled = 5,
    Failed = 6,
    InvalidSelection = 7,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StemOutcome {
    pub request: StemRequest,
    pub disposition: StemDisposition,
    /// Retained make receipts or classified refusal/cancellation evidence.
    /// The owner must independently verify custody before completing the call.
    pub evidence_digest: [u8; 32],
}

impl StemOutcome {
    pub fn encode(self) -> [u8; STEM_OUTCOME_BYTES] {
        let mut bytes = [0; STEM_OUTCOME_BYTES];
        bytes[..32].copy_from_slice(&self.request.0);
        bytes[32] = self.disposition as u8;
        bytes[33..].copy_from_slice(&self.evidence_digest);
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != STEM_OUTCOME_BYTES {
            return None;
        }
        let disposition = match bytes[32] {
            0 => StemDisposition::Prepared,
            1 => StemDisposition::UnavailableBuilder,
            2 => StemDisposition::UnsupportedTarget,
            3 => StemDisposition::StaleInvitation,
            4 => StemDisposition::Denied,
            5 => StemDisposition::Cancelled,
            6 => StemDisposition::Failed,
            7 => StemDisposition::InvalidSelection,
            _ => return None,
        };
        Some(Self {
            request: StemRequest(bytes[..32].try_into().ok()?),
            disposition,
            evidence_digest: bytes[33..].try_into().ok()?,
        })
    }
}

struct StemCompletion;
impl DigestCompletion for StemCompletion {
    const OUTCOME_BYTES: usize = STEM_OUTCOME_BYTES;
    fn matches(bytes: &[u8], request: [u8; 32]) -> bool {
        StemOutcome::decode(bytes).is_some_and(|outcome| outcome.request.0 == request)
    }
}

/// Uses the same commit, pressure and cancellation mechanism as Crèche, with a
/// distinct protocol. Installed owners validate exact operation identity and
/// current authority before effects; a well-shaped completion cannot do that.
pub struct StemBack(DigestCallBack<StemCompletion>);
impl StemBack {
    pub fn for_admitted_host(call: HostCallId) -> Self {
        Self(DigestCallBack::for_admitted_host(call))
    }
    pub fn new(call: HostCallId, request: StemRequest) -> Self {
        Self(DigestCallBack::new(call, request.0))
    }
}
impl<const PORTS: usize> StepBack<PORTS> for StemBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        self.0.step(io, bytes)
    }
    fn step_committed(&mut self) {
        <DigestCallBack<StemCompletion> as StepBack<PORTS>>::step_committed(&mut self.0)
    }
    fn cancel(&mut self) {
        <DigestCallBack<StemCompletion> as StepBack<PORTS>>::cancel(&mut self.0)
    }
}
