//! Same-Body member Seed preparation over the ordinary kernel boundary.
//! The selected owner retains checked configuration, current invitation,
//! authority, bounded make effects and evidence. This Back cannot create a Body,
//! issue an invitation, admit a Part, deploy an artifact or observe a Boot.
use crate::preparation_call::{DigestCallBack, DigestCompletion};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    HostCallId,
};

pub const WOMB_KIND: &str = "womb/prepare";
pub const WOMB_REQUEST_KIND: &str = "conduit.womb/request@1";
pub const WOMB_OUTCOME_KIND: &str = "conduit.womb/outcome@1";
pub const WOMB_REQUEST_BYTES: usize = 32;
pub const WOMB_OUTCOME_BYTES: usize = 65;

/// Exact reference to retained admission state bound to the actual producing
/// Body/Plan/Play, reviewed Host selection, invitation and finite effect grants.
/// A digest is neither possession of authority nor proof that state is current.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WombRequest(pub [u8; WOMB_REQUEST_BYTES]);

/// Member Seed preparation and classified refusal only. There is deliberately no
/// Boot, membership or deployment-success disposition on this operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum WombDisposition {
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
pub struct WombOutcome {
    pub request: WombRequest,
    pub disposition: WombDisposition,
    /// Retained make receipts or classified refusal/cancellation evidence.
    /// The owner must independently verify custody before completing the call.
    pub evidence_digest: [u8; 32],
}

impl WombOutcome {
    pub fn encode(self) -> [u8; WOMB_OUTCOME_BYTES] {
        let mut bytes = [0; WOMB_OUTCOME_BYTES];
        bytes[..32].copy_from_slice(&self.request.0);
        bytes[32] = self.disposition as u8;
        bytes[33..].copy_from_slice(&self.evidence_digest);
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != WOMB_OUTCOME_BYTES {
            return None;
        }
        let disposition = match bytes[32] {
            0 => WombDisposition::Prepared,
            1 => WombDisposition::UnavailableBuilder,
            2 => WombDisposition::UnsupportedTarget,
            3 => WombDisposition::StaleInvitation,
            4 => WombDisposition::Denied,
            5 => WombDisposition::Cancelled,
            6 => WombDisposition::Failed,
            7 => WombDisposition::InvalidSelection,
            _ => return None,
        };
        Some(Self {
            request: WombRequest(bytes[..32].try_into().ok()?),
            disposition,
            evidence_digest: bytes[33..].try_into().ok()?,
        })
    }
}

struct WombCompletion;
impl DigestCompletion for WombCompletion {
    const OUTCOME_BYTES: usize = WOMB_OUTCOME_BYTES;
    fn matches(bytes: &[u8], request: [u8; 32]) -> bool {
        WombOutcome::decode(bytes).is_some_and(|outcome| outcome.request.0 == request)
    }
}

/// Uses the same commit, pressure and cancellation mechanism as Crèche, with a
/// distinct protocol. Installed owners validate exact operation identity and
/// current authority before effects; a well-shaped completion cannot do that.
pub struct WombBack(DigestCallBack<WombCompletion>);
impl WombBack {
    pub fn for_admitted_host(call: HostCallId) -> Self {
        Self(DigestCallBack::for_admitted_host(call))
    }
    pub fn new(call: HostCallId, request: WombRequest) -> Self {
        Self(DigestCallBack::new(call, request.0))
    }
}
impl<const PORTS: usize> StepBack<PORTS> for WombBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        self.0.step(io, bytes)
    }
    fn step_committed(&mut self) {
        <DigestCallBack<WombCompletion> as StepBack<PORTS>>::step_committed(&mut self.0)
    }
    fn cancel(&mut self) {
        <DigestCallBack<WombCompletion> as StepBack<PORTS>>::cancel(&mut self.0)
    }
}
