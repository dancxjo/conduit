//! One receipt-authenticated Part signer, separate from persistent Host identity.
//! The retained seed is accepted only when its exact signed request is endorsed
//! by the independently pinned Root. No arbitrary credential/seed importer.
use super::*;
use conduit_body::{PartId, PartReturnChallenge};
use ed25519_dalek::{Signer, SigningKey};
use serde::{Deserialize, Serialize};

const SCHEMA: &str = "conduit.browser/part-membership@1";
const MAX_RECORD_BYTES: usize = 3072;
pub(super) struct PartSeed([u8; 32]);
impl PartSeed {
    pub(super) fn new(bytes: [u8; 32]) -> Result<Self, ()> {
        if bytes == [0; 32] {
            Err(())
        } else {
            Ok(Self(bytes))
        }
    }
    pub(super) fn sign(&self, transcript: &[u8; 32]) -> [u8; 64] {
        SigningKey::from_bytes(&self.0).sign(transcript).to_bytes()
    }
}
impl Drop for PartSeed {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}

#[derive(Serialize, Deserialize)]
#[serde(transparent)]
pub(super) struct SecretBytes(Vec<u8>);
impl SecretBytes {
    pub(super) fn as_slice(&self) -> &[u8] {
        &self.0
    }
    pub(super) fn erase(&mut self) {
        self.0.fill(0);
    }
}
impl Drop for SecretBytes {
    fn drop(&mut self) {
        self.erase();
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    schema: String,
    claim: SpawnInvitationClaim,
    secret: SecretBytes,
    receipt: PortableMembershipAdmissionReceipt,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Import {
    record: Record,
    expected_body_id: BodyId,
    expected_root: [u8; 32],
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Pins {
    expected_body_id: BodyId,
    expected_root: [u8; 32],
}

#[derive(Serialize)]
struct Export<'a> {
    schema: &'static str,
    claim: &'a SpawnInvitationClaim,
    secret: &'a [u8; 32],
    receipt: &'a PortableMembershipAdmissionReceipt,
}

/// Export a sensitive private storage record only after verified Root adoption.
/// The application must use selected private storage, never a shared Spore ZIP.
#[no_mangle]
pub extern "C" fn conduit_browser_membership_export_part(length: u32) -> i32 {
    clear_output();
    let length = length as usize;
    if length == 0 || length > INPUT_CAPACITY {
        return ERROR_INPUT;
    }
    let parsed = INPUT.with(|input| {
        let mut input = input.borrow_mut();
        let parsed = serde_json::from_slice::<Pins>(&input[..length]);
        input[..length].fill(0);
        parsed
    });
    let Ok(pins) = parsed else {
        return ERROR_MEMBERSHIP;
    };
    PENDING.with(|slot| {
        let slot = slot.borrow();
        let Some(pending) = slot.as_ref() else {
            return ERROR_NOT_INITIALIZED;
        };
        let Some(receipt) = pending.receipt.as_ref() else {
            return ERROR_MEMBERSHIP;
        };
        let actual_host = IDENTITY.with(|identity| {
            identity
                .borrow()
                .as_ref()
                .is_some_and(|identity| identity.host_id() == &pending.request.host_id)
        });
        if !actual_host
            || pending.request.body_id != pins.expected_body_id
            || pending.root != pins.expected_root
            || pending.credential.as_ref() != Some(&receipt.credential)
            || receipt.verify(&pending.root, &pending.request).is_err()
        {
            return ERROR_MEMBERSHIP;
        }
        let Ok(mut bytes) = serde_json::to_vec(&Export {
            schema: SCHEMA,
            claim: &pending.claim,
            secret: &pending.secret.0,
            receipt,
        }) else {
            return ERROR_MEMBERSHIP;
        };
        let result = if bytes.len() <= MAX_RECORD_BYTES {
            OUTPUT.with(|output| output.borrow_mut()[..bytes.len()].copy_from_slice(&bytes));
            OUTPUT_LEN.with(|length| *length.borrow_mut() = bytes.len());
            STATUS_READY
        } else {
            ERROR_INPUT
        };
        bytes.fill(0);
        result
    })
}

/// Restore only this persistent Host's exact previously admitted Part. Root and
/// Body pins are independent inputs from the validated admission configuration.
#[no_mangle]
pub extern "C" fn conduit_browser_membership_import_part(length: u32) -> i32 {
    clear_output();
    let length = length as usize;
    if length == 0 || length > INPUT_CAPACITY {
        return ERROR_INPUT;
    }
    let parsed = INPUT.with(|input| {
        let mut input = input.borrow_mut();
        let parsed = serde_json::from_slice::<Import>(&input[..length]);
        input[..length].fill(0);
        parsed
    });
    let Ok(envelope) = parsed else {
        return ERROR_MEMBERSHIP;
    };
    PENDING.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_some() {
            return ERROR_PENDING;
        }
        let record = &envelope.record;
        let Ok(mut encoded) = serde_json::to_vec(record) else {
            return ERROR_MEMBERSHIP;
        };
        let bounded = encoded.len() <= MAX_RECORD_BYTES;
        encoded.fill(0);
        if !bounded {
            return ERROR_INPUT;
        }
        if !ed25519_dalek::VerifyingKey::from_bytes(&envelope.expected_root)
            .is_ok_and(|key| !key.is_weak())
        {
            return ERROR_MEMBERSHIP;
        }
        let Ok(receipt_bytes) = serde_json::to_vec(&record.receipt) else {
            return ERROR_MEMBERSHIP;
        };
        if PortableMembershipAdmissionReceipt::decode(&receipt_bytes).is_err() {
            return ERROR_MEMBERSHIP;
        }
        if record.schema != SCHEMA
            || record.claim.body_id != envelope.expected_body_id
            || record.claim.inspect(0).is_err()
        {
            return ERROR_MEMBERSHIP;
        }
        let Ok(secret) = <[u8; 32]>::try_from(record.secret.as_slice())
            .map_err(|_| ())
            .and_then(PartSeed::new)
        else {
            return ERROR_SPAWN_SECRET;
        };
        IDENTITY.with(|identity| {
            let identity = identity.borrow();
            let Some(identity) = identity.as_ref() else {
                return ERROR_NOT_INITIALIZED;
            };
            let receipt = &record.receipt;
            if identity.host_id() != &receipt.credential.host_id
                || receipt.credential.body_id != envelope.expected_body_id
            {
                return ERROR_MEMBERSHIP;
            }
            // The Root's request digest pins the original seed and claim. An
            // arbitrary replacement seed produces a different request signature.
            let request = PortableMembershipAdmissionRequest {
                schema: MEMBERSHIP_ADMISSION_REQUEST_SCHEMA.into(),
                protocol_version: conduit_core::PROTOCOL_VERSION,
                invitation_id: record.claim.invitation_id.clone(),
                body_id: record.claim.body_id.clone(),
                host_id: receipt.credential.host_id.clone(),
                boot_id: receipt.credential.boot_id.clone(),
                offer_generation: receipt.offer_generation,
                nonce: record.claim.nonce,
                signature: secret.sign(&record.claim.signing_transcript(
                    &receipt.credential.host_id,
                    &receipt.credential.boot_id,
                    receipt.offer_generation,
                )),
            };
            if receipt.verify(&envelope.expected_root, &request).is_err() {
                return ERROR_MEMBERSHIP;
            }
            let result = publish(&receipt.credential);
            if result == STATUS_READY {
                *slot = Some(Pending {
                    request,
                    root: envelope.expected_root,
                    producer_host: receipt.producer_host.clone(),
                    producer_boot: receipt.producer_boot.clone(),
                    credential: Some(receipt.credential.clone()),
                    claim: record.claim.clone(),
                    secret,
                    receipt: Some(receipt.clone()),
                });
            }
            result
        })
    })
}

pub(in super::super) fn return_signature(
    identity: &BrowserAdmissionIdentity,
    challenge: &PartReturnChallenge,
) -> Result<Option<[u8; 64]>, i32> {
    PENDING.with(|slot| {
        let slot = slot.borrow();
        let Some(pending) = slot.as_ref().filter(|p| p.receipt.is_some()) else {
            return Ok(None);
        };
        let credential = pending.credential.as_ref().ok_or(ERROR_MEMBERSHIP)?;
        if challenge.body_id != credential.body_id
            || challenge.part_id != credential.part_id
            || challenge.host_id != credential.host_id
            || challenge.host_id != *identity.host_id()
            || challenge.boot_id != *identity.boot_id()
            || challenge.offer_generation != OfferGeneration(1)
            || challenge.nonce == [0; 32]
            || challenge.expires_at_millis <= challenge.issued_at_millis
        {
            return Err(ERROR_CHALLENGE);
        }
        Ok(Some(pending.secret.sign(&challenge.signing_transcript())))
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Retire {
    body_id: BodyId,
    part_id: Option<PartId>,
}
/// Explicitly retire the exact selected scope. Transport loss does not call it:
/// a successfully retained Part must remain available for future return.
#[no_mangle]
pub extern "C" fn conduit_browser_membership_retire_scope(length: u32) -> i32 {
    clear_output();
    let length = length as usize;
    if length == 0 || length > INPUT_CAPACITY {
        return ERROR_INPUT;
    }
    let parsed = INPUT.with(|input| {
        let mut input = input.borrow_mut();
        let parsed = serde_json::from_slice::<Retire>(&input[..length]);
        input[..length].fill(0);
        parsed
    });
    let Ok(scope) = parsed else {
        return ERROR_MEMBERSHIP;
    };
    PENDING.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(pending) = slot.as_ref() else {
            return ERROR_NOT_INITIALIZED;
        };
        let actual_host = IDENTITY.with(|identity| {
            identity
                .borrow()
                .as_ref()
                .is_some_and(|identity| identity.host_id() == &pending.request.host_id)
        });
        if !actual_host {
            return ERROR_MEMBERSHIP;
        }
        if scope.body_id != pending.claim.body_id
            || scope.part_id.as_ref() != pending.credential.as_ref().map(|c| &c.part_id)
        {
            return ERROR_MEMBERSHIP;
        }
        slot.take();
        STATUS_READY
    })
}
