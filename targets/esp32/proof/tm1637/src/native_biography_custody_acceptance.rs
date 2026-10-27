//! Typed Root endorsement drafts. Decoded wire values alone cannot construct a
//! draft. Actual Store custody must rederive its pinned offer before acceptance.
use super::biography_wire::{Consumer, Hex, Offer, Receipt, Request};
use ed25519_dalek::VerifyingKey;
use sha2::{Digest, Sha256};
/// Only canonical current-admission verification can mint this draft. The
/// production Store must additionally rederive the offer from its pinned actual
/// SourceSnapshot before calling this CPU-prototype acceptance entrance.
pub struct AcceptedDraft {
    receipt: Receipt,
    root: [u8; 32],
}
impl AcceptedDraft {
    pub fn transcript(&self) -> [u8; 32] {
        self.receipt.transcript(&self.root)
    }
    pub fn root(&self) -> &[u8; 32] {
        &self.root
    }
    pub fn signed(mut self, signature: [u8; 64]) -> Receipt {
        self.receipt.signature = Hex(signature);
        self.receipt
    }
}
pub fn verify_ack(
    offer: &Offer,
    root: &VerifyingKey,
    manager: &conduit_body::AdmissionManager,
    membership: &conduit_body::BodyMembership,
    request: &Request,
) -> Result<AcceptedDraft, &'static str> {
    let id = offer.verify(&membership.body_id, root)?;
    if request.offer.0 != id {
        return Err("biography ACK offer");
    }
    let part = membership
        .parts
        .iter()
        .find(|p| p.part_id.as_str() == request.part)
        .ok_or("biography ACK part")?;
    let current = part.current.as_ref().ok_or("biography ACK presence")?;
    if current.host_id.as_str() != request.host || current.boot_id.as_str() != request.boot {
        return Err("biography ACK incarnation");
    }
    let transcript = request.transcript()?;
    let verified = manager
        .verify_admitted_message(
            membership,
            &part.part_id,
            &current.host_id,
            &current.boot_id,
            &transcript,
            &request.signature.0,
        )
        .map_err(|_| "biography admitted signature")?;
    let mut consumer = Sha256::new();
    for id in [
        verified.part_id().as_str(),
        verified.host_id().as_str(),
        verified.boot_id().as_str(),
    ] {
        consumer.update((id.len() as u64).to_le_bytes());
        consumer.update(id.as_bytes());
    }
    consumer.update(verified.membership_revision().0.to_le_bytes());
    Ok(AcceptedDraft {
        receipt: Receipt {
            body: offer.body.clone(),
            summary: offer.end.clone(),
            offer: Hex(id),
            consumer: Consumer::Admitted {
                identity: Hex(consumer.finalize().into()),
            },
            acknowledged: Hex(Sha256::digest(request.signature.0).into()),
            signature: Hex([0; 64]),
        },
        root: root.to_bytes(),
    })
}

/// Gate sender proof is affine and names this exact Root-authenticated offer.
/// The actual Store caller additionally pins and recomputes its current source.
pub(crate) fn accept_owner(
    offer: &Offer,
    root: &VerifyingKey,
    authorized: crate::native_body_join::owner::AuthorizedOwnerBiography,
) -> Result<AcceptedDraft, &'static str> {
    let id = offer.verify(&offer.body, root)?;
    if !authorized.matches(offer.body.as_str(), &root.to_bytes(), &id) {
        return Err("biography owner offer changed");
    }
    Ok(AcceptedDraft {
        receipt: Receipt {
            body: offer.body.clone(),
            summary: offer.end.clone(),
            offer: Hex(id),
            consumer: Consumer::Owner {
                key: Hex(*authorized.owner()),
            },
            acknowledged: Hex(*authorized.acknowledgement()),
            signature: Hex([0; 64]),
        },
        root: root.to_bytes(),
    })
}
