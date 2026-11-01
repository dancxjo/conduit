//! Canonical retained-Part Return protocol. No transport or execution authority.
//!
//! The connector must select/authenticate the retained rendezvous pins before
//! passing documents here. A protocol result alone is deliberately not a
//! current owner session, Face subscription, persisted credential, or Show.
use crate::durable_host::RetainedPart;
use conduit_body::MembershipCredential;
use conduit_std_host::browser_admission::{
    BrowserAdmissionEgress as Out, BrowserAdmissionIngress as In,
    BROWSER_ADMISSION_PROTOCOL as PROTOCOL, MAX_BROWSER_ADMISSION_FRAME_BYTES as LIMIT,
};

pub(super) struct NativeReturn {
    part: RetainedPart,
    phase: Phase,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Ready,
    Challenge,
    Admission,
    Finished,
    Failed,
}
impl NativeReturn {
    pub(super) fn new(part: RetainedPart) -> Result<Self, String> {
        let route = part
            .rendezvous()
            .ok_or("retained Part has no pinned rendezvous route")?;
        // Structural inspection only; an eventual connector must validate its
        // selected candidate's actual clock/attempt budget before any effect.
        route
            .validate(0)
            .map_err(|_| "retained rendezvous structure or pins invalid")?;
        if route.body_id != part.credential().body_id.as_str() {
            return Err("retained rendezvous belongs to another Body".into());
        }
        Ok(Self {
            part,
            phase: Phase::Ready,
        })
    }
    /// Exact original route, not a newly supplied invitation or discovered pin.
    pub(super) fn rendezvous(&self) -> &conduit_body::SpawnRendezvousDescriptor {
        self.part
            .rendezvous()
            .expect("constructor required retained route")
    }
    pub(super) fn advertise(&mut self) -> Result<Vec<u8>, String> {
        if self.phase != Phase::Ready {
            return Err("Return advertisement out of order".into());
        }
        let result = encode(&In::ReturnAdvertise {
            protocol: PROTOCOL,
            credential: self.part.credential().clone(),
            advertisement: self.part.advertisement().clone(),
        });
        self.phase = if result.is_ok() {
            Phase::Challenge
        } else {
            Phase::Failed
        };
        result
    }
    /// One canonical challenge, bounded before decoding; refusal is terminal.
    pub(super) fn challenge(&mut self, bytes: &[u8]) -> Result<Vec<u8>, String> {
        if self.phase != Phase::Challenge {
            return Err("Return challenge out of order".into());
        }
        self.phase = Phase::Failed;
        let Out::ReturnChallenge {
            protocol: PROTOCOL,
            challenge,
        } = decode(bytes)?
        else {
            return Err("expected canonical Return challenge".into());
        };
        let proof = self.part.prove(&challenge)?;
        let bytes = encode(&In::ReturnProof {
            protocol: PROTOCOL,
            admission_id: proof.admission_id,
            body_id: proof.body_id,
            part_id: proof.part_id,
            host_id: proof.host_id,
            boot_id: proof.boot_id,
            nonce: proof.nonce.to_vec(),
            signature: proof.signature.to_vec(),
        })?;
        self.phase = Phase::Admission;
        Ok(bytes)
    }
    /// Correlate the response only. Authentication belongs to the protected
    /// connector; this method neither persists nor grants anything.
    pub(super) fn admitted(&mut self, bytes: &[u8]) -> Result<MembershipCredential, String> {
        if self.phase != Phase::Admission {
            return Err("Return admission out of order".into());
        }
        self.phase = Phase::Failed;
        let Out::Admitted {
            protocol: PROTOCOL,
            credential,
            signed_membership_receipt,
        } = decode(bytes)?
        else {
            return Err("expected canonical Return admission".into());
        };
        // A Return does not silently convert the retained raw TLS receipt into
        // Root-signed provenance. A signed receipt needs its own verifier.
        if signed_membership_receipt.is_some() {
            return Err("unexpected Return receipt provenance".into());
        }
        let old = self.part.credential();
        let ad = self.part.advertisement();
        if credential.body_id != old.body_id
            || credential.part_id != old.part_id
            || credential.host_id != ad.host_id
            || credential.boot_id != ad.boot_id
        {
            return Err("Return admission changed exact Part or current Boot".into());
        }
        self.phase = Phase::Finished;
        Ok(credential)
    }
}
fn encode(value: &In) -> Result<Vec<u8>, String> {
    let bytes = serde_json::to_vec(value).map_err(|_| "Return request encoding")?;
    if bytes.len() > LIMIT {
        return Err("Return request exceeds canonical bound".into());
    }
    Ok(bytes)
}
fn decode(bytes: &[u8]) -> Result<Out, String> {
    if bytes.is_empty() || bytes.len() > LIMIT {
        return Err("Return response exceeds canonical bound".into());
    }
    serde_json::from_slice(bytes).map_err(|_| "Return response encoding".into())
}
#[cfg(test)]
#[path = "native_return_tests.rs"]
mod tests;
