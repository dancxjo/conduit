//! Unlinked typed custody vertical. The native archive must retain this module
//! as a child so only its actual frozen journal can construct an offer draft.
//! Neither a network offer nor a decoded ACK constructs admitted/erase custody.
#[path = "native_observed_custody_wire.rs"]
pub(crate) mod wire;
use super::{custody_journal::WholeEpoch, state::Active};
use conduit_body::{AdmissionManager, BodyId, BodyMembership};
use conduit_environment_runtime::{
    display_evidence_journal::Storage,
    observed_record::{self, ObservedRecord},
};
use ed25519_dalek::{Signature, VerifyingKey};
use sha2::{Digest, Sha256};
use wire::{Hex, Offer, Request};
pub(crate) struct ClosedOfferDraft {
    offer: Offer<'static>,
    root: [u8; 32],
}
impl ClosedOfferDraft {
    pub(crate) fn transcript(&self) -> [u8; 32] {
        wire::offer_transcript(&self.offer, &self.root)
    }
    pub(crate) fn root(&self) -> &[u8; 32] {
        &self.root
    }
    pub(crate) fn signed(self, signature: [u8; 64]) -> Offer<'static> {
        Offer {
            signature: Hex(signature),
            ..self.offer
        }
    }
}
/// `failed` may hide a physically committed record behind stale live counters.
/// Such media must be reopened read-only before any offer can acknowledge it.
pub(crate) fn inspect<S: Storage + WholeEpoch>(
    active: &mut Active<S>,
    body: &BodyId,
    key: &VerifyingKey,
    work: &mut [u8],
) -> Result<ClosedOfferDraft, &'static str> {
    if !active.recovered || !active.signed || active.failed {
        return Err("custody journal not frozen");
    }
    let count = active.journal.committed_count();
    if count == 0
        || count > 960
        || active.last_sequence != u64::from(count)
        || active.scope[..32] != Sha256::digest(body.as_str().as_bytes())[..]
    {
        return Err("custody journal scope");
    }
    let region = active.journal.inspect_storage(WholeEpoch::identity);
    let mut cursor = active.journal.replay_cursor();
    let mut signed = [0; observed_record::BYTES + 64];
    let mut hash = Sha256::new();
    let mut sequence = 0;
    let mut clock = 0;
    while let Some(length) = active
        .journal
        .replay(&mut cursor, &mut signed, work)
        .map_err(|_| "custody journal replay")?
    {
        if length != signed.len() {
            return Err("custody signed record extent");
        }
        let record = ObservedRecord::decode(&signed[..observed_record::BYTES])
            .map_err(|_| "custody record")?;
        if record.sequence != sequence + 1 || record.observed_at_boot_ms < clock {
            return Err("custody record continuity");
        }
        super::history::verify(
            key,
            &active.scope,
            &active.forms,
            &signed[..observed_record::BYTES],
            signed[observed_record::BYTES..].try_into().unwrap(),
        )
        .map_err(|_| "custody record signature")?;
        hash.update(&signed[..observed_record::BYTES]);
        sequence = record.sequence;
        clock = record.observed_at_boot_ms;
    }
    if sequence != u64::from(count) {
        return Err("custody journal count");
    }
    Ok(ClosedOfferDraft {
        offer: Offer {
            schema: wire::OFFER,
            region: Hex(region),
            scope: Hex(active.scope),
            forms: Hex(active.forms),
            committed: count,
            digest: Hex(hash.finalize().into()),
            signature: Hex([0; 64]),
        },
        root: key.to_bytes(),
    })
}
/// Created only from the actual immutable journal and canonical opaque current
/// admitted-message verification. Root's signer must require its exact key.
pub(crate) struct AcceptedDraft {
    bytes: [u8; 416],
    root: [u8; 32],
}
impl AcceptedDraft {
    pub(crate) fn transcript(&self) -> [u8; 32] {
        wire::accepted_transcript(&self.bytes)
    }
    pub(crate) fn root(&self) -> &[u8; 32] {
        &self.root
    }
    pub(crate) fn signed(mut self, signature: [u8; 64]) -> [u8; 416] {
        self.bytes[352..].copy_from_slice(&signature);
        self.bytes
    }
}
pub(crate) fn verify_ack(
    draft: &ClosedOfferDraft,
    manager: &AdmissionManager,
    membership: &BodyMembership,
    request: &Request<'_>,
) -> Result<AcceptedDraft, &'static str> {
    request.validate()?;
    if request.offer.0 != draft.transcript()
        || draft.offer.scope.0[..32] != Sha256::digest(membership.body_id.as_str().as_bytes())[..]
    {
        return Err("custody acknowledgement scope");
    }
    let part = membership
        .parts
        .iter()
        .find(|part| part.part_id.as_str() == request.part)
        .ok_or("custody member")?;
    let current = part.current.as_ref().ok_or("custody presence")?;
    if current.host_id.as_str() != request.host || current.boot_id.as_str() != request.boot {
        return Err("custody current incarnation");
    }
    let transcript = request.transcript();
    let verified = manager
        .verify_admitted_message(
            membership,
            &part.part_id,
            &current.host_id,
            &current.boot_id,
            &transcript,
            &request.signature.0,
        )
        .map_err(|_| "custody admitted signature")?;
    let mut bytes = [0; 416];
    bytes[..32].copy_from_slice(&draft.offer.region.0);
    bytes[32..128].copy_from_slice(&draft.offer.scope.0);
    bytes[128..160].copy_from_slice(&draft.offer.forms.0);
    bytes[160..164].copy_from_slice(&draft.offer.committed.to_le_bytes());
    bytes[164..196].copy_from_slice(&draft.offer.digest.0);
    bytes[196..228].copy_from_slice(verified.transcript());
    bytes[228..292].copy_from_slice(&request.signature.0);
    let mut member = Sha256::new();
    for id in [
        verified.part_id().as_str(),
        verified.host_id().as_str(),
        verified.boot_id().as_str(),
    ] {
        member.update((id.len() as u64).to_le_bytes());
        member.update(id.as_bytes());
    }
    member.update(verified.membership_revision().0.to_le_bytes());
    bytes[292..324].copy_from_slice(&member.finalize());
    Ok(AcceptedDraft {
        bytes,
        root: draft.root,
    })
}
pub(crate) fn verify_accepted(bytes: &[u8; 416], body: &BodyId, key: &VerifyingKey) -> bool {
    let count = u32::from_le_bytes(bytes[160..164].try_into().unwrap());
    bytes[32..64] == Sha256::digest(body.as_str().as_bytes())[..]
        && count > 0
        && count <= 960
        && bytes[324..352] == [0; 28]
        && key
            .verify_strict(
                &wire::accepted_transcript(bytes),
                &Signature::from_bytes(bytes[352..].try_into().unwrap()),
            )
            .is_ok()
}
