//! Bounded WASM ABI for browser-owned Body admission proof material.

use crate::membership::BrowserAdmissionIdentity;
use conduit_body::{
    AdmissionChallenge, PartReturnChallenge, SpawnInvitationClaim, SpawnInvitationSecret,
    ADMISSION_SIGNATURE_BYTES,
};
use conduit_core::{BootId, HostId, OfferGeneration};
use serde::Deserialize;
use std::cell::RefCell;

const INPUT_CAPACITY: usize = 4_096;
const OUTPUT_CAPACITY: usize = conduit_body::MAX_CANDIDATE_ADVERTISEMENT_BYTES as usize;
const KEY_BYTES: usize = 32;
const MAX_IDENTITY_BYTES: usize = 128;
const STATUS_READY: i32 = 0;
const ERROR_INPUT: i32 = -260;
const ERROR_NOT_INITIALIZED: i32 = -261;
const ERROR_CHALLENGE: i32 = -262;
const ERROR_ADVERTISEMENT: i32 = -263;
const ERROR_SPAWN_SECRET: i32 = -264;

thread_local! {
    static IDENTITY: RefCell<Option<BrowserAdmissionIdentity>> = const { RefCell::new(None) };
    static INPUT: RefCell<[u8; INPUT_CAPACITY]> = const { RefCell::new([0; INPUT_CAPACITY]) };
    static OUTPUT: RefCell<[u8; OUTPUT_CAPACITY]> = const { RefCell::new([0; OUTPUT_CAPACITY]) };
    static OUTPUT_LEN: RefCell<usize> = const { RefCell::new(0) };
}

#[no_mangle]
pub extern "C" fn conduit_browser_membership_input_ptr() -> *mut u8 {
    INPUT.with(|input| input.borrow_mut().as_mut_ptr())
}

#[no_mangle]
pub extern "C" fn conduit_browser_membership_input_capacity() -> u32 {
    INPUT_CAPACITY as u32
}

#[no_mangle]
pub extern "C" fn conduit_browser_membership_output_ptr() -> *const u8 {
    OUTPUT.with(|output| output.borrow().as_ptr())
}

#[no_mangle]
pub extern "C" fn conduit_browser_membership_output_len() -> u32 {
    OUTPUT_LEN.with(|length| *length.borrow() as u32)
}

/// Initializes one browser Host incarnation from adjacent host, Boot, and
/// cryptographic-seed bytes in the input buffer. Secret bytes are copied into
/// the signing key and the input buffer is zeroed before returning.
#[no_mangle]
pub extern "C" fn conduit_browser_membership_initialize(host_length: u32, boot_length: u32) -> i32 {
    clear_identity_and_output();
    let host_length = host_length as usize;
    let boot_length = boot_length as usize;
    let Some(identity_length) = host_length.checked_add(boot_length) else {
        return ERROR_INPUT;
    };
    let Some(total_length) = identity_length.checked_add(KEY_BYTES) else {
        return ERROR_INPUT;
    };
    if host_length == 0
        || boot_length == 0
        || host_length > MAX_IDENTITY_BYTES
        || boot_length > MAX_IDENTITY_BYTES
        || total_length > INPUT_CAPACITY
    {
        return ERROR_INPUT;
    }
    INPUT.with(|input| {
        let mut input = input.borrow_mut();
        let result = (|| {
            let host = core::str::from_utf8(&input[..host_length]).map_err(|_| ERROR_INPUT)?;
            let boot = core::str::from_utf8(&input[host_length..identity_length])
                .map_err(|_| ERROR_INPUT)?;
            let mut seed = [0; KEY_BYTES];
            seed.copy_from_slice(&input[identity_length..total_length]);
            BrowserAdmissionIdentity::from_csprng_seed(HostId::from(host), BootId::from(boot), seed)
                .map_err(|_| ERROR_INPUT)
        })();
        input[..total_length].fill(0);
        match result {
            Ok(identity) => {
                let key = identity.verifying_key();
                OUTPUT.with(|output| output.borrow_mut()[..KEY_BYTES].copy_from_slice(&key));
                OUTPUT_LEN.with(|length| *length.borrow_mut() = KEY_BYTES);
                IDENTITY.with(|slot| *slot.borrow_mut() = Some(identity));
                STATUS_READY
            }
            Err(error) => error,
        }
    })
}

/// Parses one exact serialized `AdmissionChallenge`, validates its Host and
/// Boot against this WASM instance, and returns only its 64-byte signature.
#[no_mangle]
pub extern "C" fn conduit_browser_membership_prove(challenge_length: u32) -> i32 {
    clear_output();
    let challenge_length = challenge_length as usize;
    if challenge_length == 0 || challenge_length > INPUT_CAPACITY {
        return ERROR_INPUT;
    }
    INPUT.with(|input| {
        let input = input.borrow();
        let challenge: AdmissionChallenge = match serde_json::from_slice(&input[..challenge_length])
        {
            Ok(challenge) => challenge,
            Err(_) => return ERROR_CHALLENGE,
        };
        IDENTITY.with(|slot| {
            let slot = slot.borrow();
            let Some(identity) = slot.as_ref() else {
                return ERROR_NOT_INITIALIZED;
            };
            let proof = match identity.prove(&challenge) {
                Ok(proof) => proof,
                Err(_) => return ERROR_CHALLENGE,
            };
            OUTPUT.with(|output| {
                output.borrow_mut()[..ADMISSION_SIGNATURE_BYTES].copy_from_slice(&proof.signature)
            });
            OUTPUT_LEN.with(|length| *length.borrow_mut() = ADMISSION_SIGNATURE_BYTES);
            STATUS_READY
        })
    })
}

/// Signs one canonical return challenge with the same in-memory browser Host
/// incarnation. It cannot change the Host or Boot named by the challenge.
#[no_mangle]
pub extern "C" fn conduit_browser_membership_prove_return(challenge_length: u32) -> i32 {
    clear_output();
    let challenge_length = challenge_length as usize;
    if challenge_length == 0 || challenge_length > INPUT_CAPACITY {
        return ERROR_INPUT;
    }
    INPUT.with(|input| {
        let input = input.borrow();
        let challenge: PartReturnChallenge =
            match serde_json::from_slice(&input[..challenge_length]) {
                Ok(challenge) => challenge,
                Err(_) => return ERROR_CHALLENGE,
            };
        IDENTITY.with(|slot| {
            let slot = slot.borrow();
            let Some(identity) = slot.as_ref() else {
                return ERROR_NOT_INITIALIZED;
            };
            let proof = match identity.prove_return(&challenge) {
                Ok(proof) => proof,
                Err(_) => return ERROR_CHALLENGE,
            };
            OUTPUT.with(|output| {
                output.borrow_mut()[..ADMISSION_SIGNATURE_BYTES].copy_from_slice(&proof.signature)
            });
            OUTPUT_LEN.with(|length| *length.borrow_mut() = ADMISSION_SIGNATURE_BYTES);
            STATUS_READY
        })
    })
}

/// Signs one Body-directed single-use invitation for this exact browser Host,
/// Boot, and the exported offer generation.
#[no_mangle]
pub extern "C" fn conduit_browser_membership_prove_spawn(claim_length: u32) -> i32 {
    clear_output();
    let claim_length = claim_length as usize;
    if claim_length == 0 || claim_length > INPUT_CAPACITY {
        return ERROR_INPUT;
    }
    INPUT.with(|input| {
        let input = input.borrow();
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct SpawnClaimEnvelope {
            claim: SpawnInvitationClaim,
            secret: Vec<u8>,
        }
        let envelope: SpawnClaimEnvelope = match serde_json::from_slice(&input[..claim_length]) {
            Ok(claim) => claim,
            Err(_) => return ERROR_CHALLENGE,
        };
        let Ok(secret_bytes) = <[u8; 32]>::try_from(envelope.secret.as_slice()) else {
            return ERROR_SPAWN_SECRET;
        };
        let secret = match SpawnInvitationSecret::from_csprng_bytes(secret_bytes) {
            Ok(secret) => secret,
            Err(_) => return ERROR_SPAWN_SECRET,
        };
        IDENTITY.with(|slot| {
            let slot = slot.borrow();
            let Some(identity) = slot.as_ref() else {
                return ERROR_NOT_INITIALIZED;
            };
            let signature = secret.sign(&envelope.claim.signing_transcript(
                identity.host_id(),
                identity.boot_id(),
                OfferGeneration(1),
            ));
            OUTPUT.with(|output| {
                output.borrow_mut()[..ADMISSION_SIGNATURE_BYTES].copy_from_slice(&signature)
            });
            OUTPUT_LEN.with(|length| *length.borrow_mut() = ADMISSION_SIGNATURE_BYTES);
            STATUS_READY
        })
    })
}

/// Exports this Host's exact finite browser profile as serialized canonical
/// `HostAdvertisement` data. The renderer does not synthesize capabilities.
#[no_mangle]
pub extern "C" fn conduit_browser_membership_advertisement() -> i32 {
    clear_output();
    IDENTITY.with(|slot| {
        let slot = slot.borrow();
        let Some(identity) = slot.as_ref() else {
            return ERROR_NOT_INITIALIZED;
        };
        let advertisement = crate::installed_browser::membership_advertisement(
            identity.host_id().clone(),
            identity.boot_id().clone(),
        );
        let encoded = match serde_json::to_vec(&advertisement) {
            Ok(encoded) if encoded.len() <= OUTPUT_CAPACITY => encoded,
            _ => return ERROR_ADVERTISEMENT,
        };
        OUTPUT.with(|output| output.borrow_mut()[..encoded.len()].copy_from_slice(&encoded));
        OUTPUT_LEN.with(|length| *length.borrow_mut() = encoded.len());
        STATUS_READY
    })
}

fn clear_identity_and_output() {
    IDENTITY.with(|slot| slot.borrow_mut().take());
    clear_output();
}

fn clear_output() {
    OUTPUT.with(|output| output.borrow_mut().fill(0));
    OUTPUT_LEN.with(|length| *length.borrow_mut() = 0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signature, Verifier, VerifyingKey};
    fn write(bytes: &[u8]) {
        INPUT.with(|input| input.borrow_mut()[..bytes.len()].copy_from_slice(bytes));
    }

    fn output<const N: usize>() -> [u8; N] {
        OUTPUT.with(|output| output.borrow()[..N].try_into().unwrap())
    }

    fn initialize() -> AdmissionChallenge {
        let host = b"browser/tab-live";
        let boot = b"browser-boot/live";
        let mut frame = Vec::from(host.as_slice());
        frame.extend_from_slice(boot);
        frame.extend_from_slice(&[7; KEY_BYTES]);
        write(&frame);
        assert_eq!(
            conduit_browser_membership_initialize(host.len() as u32, boot.len() as u32),
            STATUS_READY
        );
        assert_eq!(conduit_browser_membership_output_len(), KEY_BYTES as u32);
        INPUT.with(|input| assert!(input.borrow()[..frame.len()].iter().all(|byte| *byte == 0)));
        serde_json::from_value(serde_json::json!({
            "admission_id": "admission/live",
            "body_id": "body/live",
            "candidate_id": "candidate/live",
            "host_id": "browser/tab-live",
            "boot_id": "browser-boot/live",
            "offer_generation": 3,
            "nonce": vec![9; 32],
            "issued_at_millis": 1_000,
            "expires_at_millis": 2_000
        }))
        .unwrap()
    }

    #[test]
    fn abi_returns_public_key_then_exact_challenge_signature() {
        let challenge = initialize();
        let verifying_key = VerifyingKey::from_bytes(&output()).unwrap();
        let encoded = serde_json::to_vec(&challenge).unwrap();
        write(&encoded);
        assert_eq!(
            conduit_browser_membership_prove(encoded.len() as u32),
            STATUS_READY
        );
        assert_eq!(
            conduit_browser_membership_output_len(),
            ADMISSION_SIGNATURE_BYTES as u32
        );
        let signature = Signature::from_bytes(&output());
        verifying_key
            .verify(&challenge.signing_transcript(), &signature)
            .unwrap();
    }

    #[test]
    fn abi_signs_exact_same_boot_return_and_refuses_replacement_boot() {
        initialize();
        let verifying_key = VerifyingKey::from_bytes(&output()).unwrap();
        let challenge: PartReturnChallenge = serde_json::from_value(serde_json::json!({
            "admission_id": "return/live",
            "body_id": "body/live",
            "part_id": "part/live",
            "host_id": "browser/tab-live",
            "boot_id": "browser-boot/live",
            "offer_generation": 1,
            "nonce": vec![4; 32],
            "issued_at_millis": 2_000,
            "expires_at_millis": 3_000
        }))
        .unwrap();
        let encoded = serde_json::to_vec(&challenge).unwrap();
        write(&encoded);
        assert_eq!(
            conduit_browser_membership_prove_return(encoded.len() as u32),
            STATUS_READY
        );
        let signature = Signature::from_bytes(&output());
        verifying_key
            .verify(&challenge.signing_transcript(), &signature)
            .unwrap();

        let mut replaced = challenge;
        replaced.boot_id = BootId::from("browser-boot/replaced");
        let encoded = serde_json::to_vec(&replaced).unwrap();
        write(&encoded);
        assert_eq!(
            conduit_browser_membership_prove_return(encoded.len() as u32),
            ERROR_CHALLENGE
        );
        assert_eq!(conduit_browser_membership_output_len(), 0);
    }

    #[test]
    fn abi_exports_the_exact_finite_browser_advertisement() {
        let challenge = initialize();
        assert_eq!(conduit_browser_membership_advertisement(), STATUS_READY);
        let length = conduit_browser_membership_output_len() as usize;
        let encoded = OUTPUT.with(|output| output.borrow()[..length].to_vec());
        let advertisement: conduit_core::HostAdvertisement =
            serde_json::from_slice(&encoded).unwrap();
        assert_eq!(advertisement.host_id, challenge.host_id);
        assert_eq!(advertisement.boot_id, challenge.boot_id);
        assert!(!advertisement.capabilities.is_empty());
        assert!(!advertisement.resources.is_empty());
    }

    #[test]
    fn abi_signs_one_exact_spawn_invitation_claim() {
        let challenge = initialize();
        let claim: SpawnInvitationClaim = serde_json::from_value(serde_json::json!({
            "invitation_id": "spawn/live",
            "body_id": "body/live",
            "nonce": vec![5; 32],
            "expires_at_millis": 2_000
        }))
        .unwrap();
        let encoded = serde_json::to_vec(&serde_json::json!({
            "claim": &claim,
            "secret": vec![13; 32]
        }))
        .unwrap();
        write(&encoded);
        assert_eq!(
            conduit_browser_membership_prove_spawn(encoded.len() as u32),
            STATUS_READY
        );
        let signature = Signature::from_bytes(&output());
        let invitation_key = ed25519_dalek::SigningKey::from_bytes(&[13; 32]).verifying_key();
        invitation_key
            .verify(
                &claim.signing_transcript(
                    &challenge.host_id,
                    &challenge.boot_id,
                    OfferGeneration(1),
                ),
                &signature,
            )
            .unwrap();
    }

    #[test]
    fn abi_refuses_uninitialized_malformed_and_wrong_boot_challenges() {
        clear_identity_and_output();
        write(b"{}");
        assert_eq!(conduit_browser_membership_prove(2), ERROR_CHALLENGE);
        let mut challenge = initialize();
        challenge.boot_id = BootId::from("browser-boot/stale");
        let encoded = serde_json::to_vec(&challenge).unwrap();
        write(&encoded);
        assert_eq!(
            conduit_browser_membership_prove(encoded.len() as u32),
            ERROR_CHALLENGE
        );
        assert_eq!(conduit_browser_membership_output_len(), 0);
    }

    #[test]
    fn initialization_refuses_weak_seed_and_clears_prior_identity() {
        initialize();
        let host = b"browser/tab-live";
        let boot = b"browser-boot/live";
        let mut frame = Vec::from(host.as_slice());
        frame.extend_from_slice(boot);
        frame.extend_from_slice(&[0; KEY_BYTES]);
        write(&frame);
        assert_eq!(
            conduit_browser_membership_initialize(host.len() as u32, boot.len() as u32),
            ERROR_INPUT
        );
        assert_eq!(conduit_browser_membership_output_len(), 0);
        write(b"{}");
        assert_eq!(conduit_browser_membership_prove(2), ERROR_CHALLENGE);
    }
}
