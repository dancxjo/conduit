//! Ordinary Crèche Kind and generic kernel Back. The selected Host owns the
//! admitted encounter, canonical Face/MaskShow validation, and make effects.
//! This Back owns no UI, artifact inventory, worker, retry, or scheduler.
use crate::preparation_call::{DigestCallBack, DigestCompletion};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    HostCallId,
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

struct CrecheCompletion;
impl DigestCompletion for CrecheCompletion {
    const OUTCOME_BYTES: usize = CRECHE_OUTCOME_BYTES;
    fn matches(bytes: &[u8], request: [u8; 32]) -> bool {
        CrecheOutcome::decode(bytes).is_some_and(|outcome| outcome.request.0 == request)
    }
}

/// The ordinary Crèche protocol over the shared transactional Host Call Back.
pub struct CrecheBack(DigestCallBack<CrecheCompletion>);
impl CrecheBack {
    pub fn for_admitted_host(call: HostCallId) -> Self {
        Self(DigestCallBack::for_admitted_host(call))
    }
    pub fn new(call: HostCallId, request: CrecheRequest) -> Self {
        Self(DigestCallBack::new(call, request.0))
    }
}
impl<const PORTS: usize> StepBack<PORTS> for CrecheBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        self.0.step(io, bytes)
    }
    fn step_committed(&mut self) {
        <DigestCallBack<CrecheCompletion> as StepBack<PORTS>>::step_committed(&mut self.0)
    }
    fn cancel(&mut self) {
        <DigestCallBack<CrecheCompletion> as StepBack<PORTS>>::cancel(&mut self.0)
    }
}
