//! Canonical observer return: durable challenge and presence before endorsement.
use super::{recovery, return_wire};
use alloc::{string::String, vec::Vec};
use conduit_body::{PartReturnChallenge, SealedBodyLifecycleSession};
use conduit_core::{BootId, HostId};
use sha2::{Digest, Sha256};
pub(crate) const CHALLENGE: &str = "conduit.environment/observer-return-challenge@1";
#[derive(serde::Serialize)]
pub(crate) struct Challenge {
    pub schema: String,
    pub challenge: PartReturnChallenge,
    pub producer_host: HostId,
    pub producer_boot: BootId,
    pub producer_key: [u8; 32],
    pub signature: Vec<u8>,
}
pub(crate) struct DurableChallenge(Challenge);
fn text(h: &mut Sha256, s: &str) {
    h.update((s.len() as u64).to_le_bytes());
    h.update(s.as_bytes());
}
impl DurableChallenge {
    pub(crate) fn transcript(&self) -> [u8; 32] {
        let r = &self.0;
        let c = &r.challenge;
        let mut h = Sha256::new();
        h.update(b"conduit.environment/observer-return-challenge@1\0");
        for s in [
            c.admission_id.as_str(),
            c.body_id.as_str(),
            c.part_id.as_str(),
            c.host_id.as_str(),
            c.boot_id.as_str(),
        ] {
            text(&mut h, s)
        }
        h.update(c.offer_generation.0.to_le_bytes());
        h.update(c.nonce);
        h.update(c.issued_at_millis.to_le_bytes());
        h.update(c.expires_at_millis.to_le_bytes());
        text(&mut h, r.producer_host.as_str());
        text(&mut h, r.producer_boot.as_str());
        h.update(r.producer_key);
        h.finalize().into()
    }
    pub(crate) fn signed(mut self, signature: [u8; 64]) -> Challenge {
        self.0.signature = signature.to_vec();
        self.0
    }
}
pub(crate) struct DurableReturn(recovery::Receipt);
impl DurableReturn {
    pub(crate) fn transcript(&self) -> [u8; 32] {
        self.0.transcript()
    }
    pub(crate) fn signed(mut self, signature: [u8; 64]) -> recovery::Receipt {
        self.0.signature = signature.to_vec();
        self.0
    }
}
fn profile(
    owner: &SealedBodyLifecycleSession,
    ad: &conduit_core::HostAdvertisement,
    host: &HostId,
) -> Result<(), &'static str> {
    if &ad.host_id == host
        || !ad.bases.is_empty()
        || !ad.resources.is_empty()
        || !ad.capabilities.is_empty()
        || !ad.planner_capabilities.is_empty()
        || owner
            .realization()
            .is_some_and(|r| r.plan.body_time_requirement().is_some())
    {
        return Err("body-return-observer-profile");
    };
    Ok(())
}
#[allow(clippy::too_many_arguments)]
pub(crate) fn begin(
    owner: &mut SealedBodyLifecycleSession,
    key: &[u8; 32],
    host: &HostId,
    boot: &BootId,
    request: &return_wire::Begin,
    nonce: [u8; 32],
    now: u64,
) -> Result<DurableChallenge, &'static str> {
    profile(owner, &request.advertisement, host)?;
    if request.body_id != owner.evidence().body_id {
        return Err("body-return-body");
    }
    let expires = now.checked_add(30_000).ok_or("body-return-clock")?;
    let retained = crate::native_restart::with_admissions(|manager| {
        Ok(manager.pending_return_challenge(
            &owner.evidence().membership,
            &request.part_id,
            &request.advertisement,
            now,
        ))
    })?;
    let expired = matches!(&retained, Err(conduit_body::AdmissionRefusal::Expired));
    let challenge = match retained {
        Ok(Some(challenge)) => challenge,
        Ok(None) | Err(conduit_body::AdmissionRefusal::Expired) => {
            crate::native_restart::admissions_transaction(owner, key, |owner, manager| {
                let mut next = manager.clone();
                if expired {
                    next.retire_expired_return_for(
                        &owner.evidence().membership,
                        &request.part_id,
                        &request.advertisement,
                        now,
                    )
                    .map_err(|_| "body-return-expiry")?;
                }
                let challenge = owner
                    .issue_observer_return(
                        &mut next,
                        &request.part_id,
                        &request.advertisement,
                        nonce,
                        now,
                        expires,
                        host,
                        boot,
                    )
                    .map_err(|_| "body-return-challenge")?;
                *manager = next;
                Ok(challenge)
            })?
        }
        Err(_) => return Err("body-return-pending"),
    };
    Ok(DurableChallenge(Challenge {
        schema: String::from(CHALLENGE),
        challenge,
        producer_host: host.clone(),
        producer_boot: boot.clone(),
        producer_key: *key,
        signature: Vec::new(),
    }))
}
pub(crate) fn complete(
    owner: &mut SealedBodyLifecycleSession,
    key: &[u8; 32],
    host: &HostId,
    boot: &BootId,
    request: &return_wire::Complete,
    now: u64,
) -> Result<DurableReturn, &'static str> {
    profile(owner, &request.advertisement, host)?;
    let credential =
        crate::native_restart::admissions_transaction(owner, key, |owner, manager| {
            owner
                .admit_returning_observer(
                    manager,
                    &request.advertisement,
                    &request.proof,
                    now,
                    host,
                    boot,
                )
                .map_err(|_| "body-return-proof")
        })?;
    Ok(DurableReturn(recovery::Receipt {
        schema: String::from(recovery::RECEIPT),
        credential,
        membership_revision: owner.evidence().membership.revision.0,
        producer_host: host.clone(),
        producer_boot: boot.clone(),
        producer_key: *key,
        request_nonce: request.proof.nonce,
        signature: Vec::new(),
    }))
}
