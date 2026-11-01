//! Membership-only browser counterpart to the native Root checkpoint receipt.
//! No equipment advertisement, selected Plan, presence lease or Play is created.
use super::*;
use conduit_body::{
    BodyId, MembershipCredential, PortableMembershipAdmissionReceipt,
    PortableMembershipAdmissionRequest, MEMBERSHIP_ADMISSION_REQUEST_SCHEMA,
};
const ERROR_MEMBERSHIP: i32 = -265;
const ERROR_PENDING: i32 = -266;
mod part;
use part::{PartSeed, SecretBytes};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Preparation {
    claim: SpawnInvitationClaim,
    secret: SecretBytes,
    expected_body_id: BodyId,
    expected_root: [u8; 32],
    producer_host: HostId,
    producer_boot: BootId,
}
struct Pending {
    request: PortableMembershipAdmissionRequest,
    root: [u8; 32],
    producer_host: HostId,
    producer_boot: BootId,
    credential: Option<MembershipCredential>,
    claim: SpawnInvitationClaim,
    secret: PartSeed,
    receipt: Option<PortableMembershipAdmissionReceipt>,
}
thread_local! {
    static PENDING: RefCell<Option<Pending>> = const { RefCell::new(None) };
}
pub(super) fn reset() {
    PENDING.with(|slot| slot.borrow_mut().take());
}
fn publish(value: &impl serde::Serialize) -> i32 {
    let Ok(encoded) = serde_json::to_vec(value) else {
        return ERROR_MEMBERSHIP;
    };
    if encoded.len() > OUTPUT_CAPACITY {
        return ERROR_INPUT;
    }
    OUTPUT.with(|output| output.borrow_mut()[..encoded.len()].copy_from_slice(&encoded));
    OUTPUT_LEN.with(|length| *length.borrow_mut() = encoded.len());
    STATUS_READY
}
/// Takes a validated Spore's invitation and independently pinned Body/Root/
/// producer scope. IMAGE validation and consent remain ordinary Spore gates.
/// Invitation expiry is decided by the Root's clock when its Start executes.
#[no_mangle]
pub extern "C" fn conduit_browser_membership_prepare_scope(length: u32) -> i32 {
    clear_output();
    let length = length as usize;
    if length == 0 || length > INPUT_CAPACITY {
        return ERROR_INPUT;
    }
    let envelope = INPUT.with(|input| {
        let mut input = input.borrow_mut();
        let result = serde_json::from_slice::<Preparation>(&input[..length]);
        input[..length].fill(0);
        result
    });
    let Ok(mut envelope) = envelope else {
        return ERROR_MEMBERSHIP;
    };
    let secret = <[u8; 32]>::try_from(envelope.secret.as_slice());
    envelope.secret.erase();
    let Ok(secret) = secret else {
        return ERROR_SPAWN_SECRET;
    };
    let Ok(secret) = PartSeed::new(secret) else {
        return ERROR_SPAWN_SECRET;
    };
    if envelope.claim.body_id != envelope.expected_body_id
        || envelope.claim.inspect(0).is_err()
        || envelope.producer_host.as_str().is_empty()
        || envelope.producer_boot.as_str().is_empty()
        || envelope.producer_host.as_str().len() > MAX_IDENTITY_BYTES
        || envelope.producer_boot.as_str().len() > MAX_IDENTITY_BYTES
        || !ed25519_dalek::VerifyingKey::from_bytes(&envelope.expected_root)
            .is_ok_and(|key| !key.is_weak())
    {
        return ERROR_MEMBERSHIP;
    }
    PENDING.with(|slot| {
        let mut pending = slot.borrow_mut();
        if pending.is_some() {
            return ERROR_PENDING;
        }
        IDENTITY.with(|identity| {
            let identity = identity.borrow();
            let Some(identity) = identity.as_ref() else {
                return ERROR_NOT_INITIALIZED;
            };
            let signature = secret.sign(&envelope.claim.signing_transcript(
                identity.host_id(),
                identity.boot_id(),
                OfferGeneration(1),
            ));
            let claim = envelope.claim.clone();
            let request = PortableMembershipAdmissionRequest {
                schema: MEMBERSHIP_ADMISSION_REQUEST_SCHEMA.into(),
                protocol_version: conduit_core::PROTOCOL_VERSION,
                invitation_id: envelope.claim.invitation_id,
                body_id: envelope.claim.body_id,
                host_id: identity.host_id().clone(),
                boot_id: identity.boot_id().clone(),
                offer_generation: OfferGeneration(1),
                nonce: envelope.claim.nonce,
                signature,
            };
            if request.validate().is_err() {
                return ERROR_MEMBERSHIP;
            }
            let result = publish(&request);
            if result == STATUS_READY {
                *pending = Some(Pending {
                    request,
                    root: envelope.expected_root,
                    producer_host: envelope.producer_host,
                    producer_boot: envelope.producer_boot,
                    credential: None,
                    claim,
                    secret,
                    receipt: None,
                });
            }
            result
        })
    })
}
/// Authenticates the exact original request and independently pinned producer.
/// Only a verified Root receipt releases the canonical membership credential.
#[no_mangle]
pub extern "C" fn conduit_browser_membership_adopt_scope(length: u32) -> i32 {
    clear_output();
    let length = length as usize;
    if length == 0 || length > conduit_body::MAX_MEMBERSHIP_ADMISSION_RECEIPT_BYTES {
        return ERROR_INPUT;
    }
    let receipt =
        INPUT.with(|input| PortableMembershipAdmissionReceipt::decode(&input.borrow()[..length]));
    let Ok(receipt) = receipt else {
        return ERROR_MEMBERSHIP;
    };
    PENDING.with(|slot| {
        let mut pending = slot.borrow_mut();
        let Some(pending) = pending.as_mut() else {
            return ERROR_NOT_INITIALIZED;
        };
        if receipt.producer_host != pending.producer_host
            || receipt.producer_boot != pending.producer_boot
            || receipt.verify(&pending.root, &pending.request).is_err()
        {
            return ERROR_MEMBERSHIP;
        }
        let result = publish(&receipt.credential);
        if result == STATUS_READY {
            pending.credential = Some(receipt.credential.clone());
            pending.receipt = Some(receipt);
        }
        result
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    fn input(bytes: &[u8]) {
        INPUT.with(|slot| slot.borrow_mut()[..bytes.len()].copy_from_slice(bytes));
    }
    fn output() -> Vec<u8> {
        let length = OUTPUT_LEN.with(|slot| *slot.borrow());
        OUTPUT.with(|slot| slot.borrow()[..length].to_vec())
    }
    fn prepare(root: &[u8; 32]) -> PortableMembershipAdmissionRequest {
        clear_identity_and_output();
        IDENTITY.with(|slot| {
            *slot.borrow_mut() = Some(
                BrowserAdmissionIdentity::from_csprng_seed(
                    "browser/own-host".into(),
                    "boot/browser-own".into(),
                    [7; 32],
                )
                .unwrap(),
            )
        });
        let envelope = serde_json::json!({
            "claim": {"invitation_id":"invitation/test", "body_id":"body/test", "nonce":vec![5u8;32], "expires_at_millis":90000},
            "secret":vec![9u8;32], "expected_body_id":"body/test", "expected_root":root,
            "producer_host":"esp32/actual-root", "producer_boot":"boot/actual-root"
        });
        let bytes = serde_json::to_vec(&envelope).unwrap();
        input(&bytes);
        assert_eq!(
            conduit_browser_membership_prepare_scope(bytes.len() as u32),
            STATUS_READY
        );
        assert!(INPUT.with(|slot| slot.borrow()[..bytes.len()].iter().all(|byte| *byte == 0)));
        PortableMembershipAdmissionRequest::decode(&output()).unwrap()
    }
    fn receipt(
        request: &PortableMembershipAdmissionRequest,
        root: &SigningKey,
    ) -> PortableMembershipAdmissionReceipt {
        let mut receipt: PortableMembershipAdmissionReceipt = serde_json::from_value(serde_json::json!({
            "schema":conduit_body::MEMBERSHIP_ADMISSION_RECEIPT_SCHEMA, "protocol_version":conduit_core::PROTOCOL_VERSION,
            "credential":{"credential_id":"credential/test", "body_id":request.body_id,
                "part_id":"part/test", "host_id":request.host_id, "boot_id":request.boot_id, "issued_at_millis":100},
            "membership_revision":2, "request_digest":request.digest(), "producer_host":"esp32/actual-root",
            "producer_boot":"boot/actual-root", "producer_key":root.verifying_key().to_bytes(),
            "invitation_id":request.invitation_id, "offer_generation":1, "membership_admitted":true,
            "current_offers_available":false,"plan_created":false,"play_created":false,"signature":vec![0u8;64]
        })).unwrap();
        receipt.signature = root.sign(&receipt.transcript()).to_bytes();
        receipt
    }
    fn adopt(receipt: &PortableMembershipAdmissionReceipt) -> i32 {
        let bytes = serde_json::to_vec(receipt).unwrap();
        input(&bytes);
        conduit_browser_membership_adopt_scope(bytes.len() as u32)
    }
    #[test]
    fn exact_browser_membership_is_verified_without_replacing_identity_or_offers() {
        let root = SigningKey::from_bytes(&[11; 32]);
        let request = prepare(&root.verifying_key().to_bytes());
        assert_eq!(request.host_id.as_str(), "browser/own-host");
        assert_eq!(request.boot_id.as_str(), "boot/browser-own");
        let identity_key = IDENTITY.with(|slot| slot.borrow().as_ref().unwrap().verifying_key());
        let receipt = receipt(&request, &root);
        assert_eq!(adopt(&receipt), STATUS_READY);
        let credential: MembershipCredential = serde_json::from_slice(&output()).unwrap();
        assert_eq!(credential, receipt.credential);
        assert_eq!(adopt(&receipt), STATUS_READY); // exact replay is idempotent
        assert_eq!(
            IDENTITY.with(|slot| slot.borrow().as_ref().unwrap().verifying_key()),
            identity_key
        );
        assert!(
            !receipt.current_offers_available && !receipt.plan_created && !receipt.play_created
        );
        assert_eq!(conduit_browser_membership_advertisement(), STATUS_READY);
        let advertisement: conduit_core::HostAdvertisement =
            serde_json::from_slice(&output()).unwrap();
        assert_eq!(advertisement.host_id, request.host_id);
        assert!(
            !advertisement.capabilities.is_empty(),
            "membership must not replace installed offers with empty metadata"
        );
    }
    #[test]
    fn forged_or_misbound_receipts_release_no_credential_and_keep_original_request() {
        let root = SigningKey::from_bytes(&[11; 32]);
        let request = prepare(&root.verifying_key().to_bytes());
        let valid = receipt(&request, &root);
        let mut variants = Vec::new();
        let mut bad = valid.clone();
        bad.signature[0] ^= 1;
        variants.push(bad);
        let mut bad = valid.clone();
        bad.producer_boot = "boot/other".into();
        bad.signature = root.sign(&bad.transcript()).to_bytes();
        variants.push(bad);
        let mut bad = valid.clone();
        bad.credential.boot_id = "boot/other".into();
        bad.signature = root.sign(&bad.transcript()).to_bytes();
        variants.push(bad);
        let mut bad = valid.clone();
        bad.request_digest[0] ^= 1;
        bad.signature = root.sign(&bad.transcript()).to_bytes();
        variants.push(bad);
        let mut bad = valid.clone();
        bad.current_offers_available = true;
        bad.signature = root.sign(&bad.transcript()).to_bytes();
        variants.push(bad);
        for bad in variants {
            assert_eq!(adopt(&bad), ERROR_MEMBERSHIP);
            assert!(output().is_empty());
            PENDING.with(|slot| {
                let slot = slot.borrow();
                let pending = slot.as_ref().unwrap();
                assert_eq!(pending.request, request);
                assert!(pending.credential.is_none());
            });
        }
        assert_eq!(adopt(&valid), STATUS_READY);
        clear_identity_and_output();
        assert_eq!(adopt(&valid), ERROR_NOT_INITIALIZED);
    }
}

// Existing Host-key return remains unchanged unless a verified Part scope is
// selected in this instance. A selected scope never falls back on mismatch.
pub(super) use part::return_signature;

#[cfg(test)]
mod part_tests;
