//! Storage compaction must preserve actual checked admission and key behavior.
use super::*;

#[test]
fn compacted_pending_proof_still_admits_and_retained_key_authenticates() {
    let (mut manager, mut candidates, mut membership, key, challenge) = ambient_fixture();
    let wire = serde_json::to_vec(&manager).unwrap();
    manager.try_compact_storage().unwrap();
    assert_eq!(serde_json::to_vec(&manager).unwrap(), wire);
    let credential = manager
        .complete_ambient(
            &mut candidates,
            &mut membership,
            &ambient_proof(&challenge, &key),
            NOW + 1,
            signs("sign/compacted-admission"),
        )
        .unwrap();
    manager.try_compact_storage().unwrap();
    let transcript = [42; 32];
    let signature = key.sign(&transcript).to_bytes();
    manager
        .verify_admitted_message(
            &membership,
            &credential.part_id,
            &credential.host_id,
            &credential.boot_id,
            &transcript,
            &signature,
        )
        .unwrap();
    assert_eq!(manager.receipts[0].credential, credential);
    assert_eq!(
        manager.complete_ambient(
            &mut candidates,
            &mut membership,
            &ambient_proof(&challenge, &key),
            NOW + 2,
            signs("sign/compacted-replay"),
        ),
        Err(AdmissionRefusal::Replay)
    );
}

#[test]
fn compacting_between_invalid_proofs_never_resets_the_attempt_cap() {
    let (mut manager, mut candidates, mut membership, key, challenge) = ambient_fixture();
    let mut invalid = ambient_proof(&challenge, &key);
    invalid.signature = [0; ADMISSION_SIGNATURE_BYTES];
    for attempt in 1..=MAX_ADMISSION_ATTEMPTS {
        manager.try_compact_storage().unwrap();
        let expected = if attempt == MAX_ADMISSION_ATTEMPTS {
            AdmissionRefusal::AttemptsExhausted
        } else {
            AdmissionRefusal::InvalidProof
        };
        assert_eq!(
            manager.complete_ambient(
                &mut candidates,
                &mut membership,
                &invalid,
                NOW + 1,
                signs("sign/compacted-invalid"),
            ),
            Err(expected)
        );
    }
    manager.try_compact_storage().unwrap();
    assert_eq!(
        manager.complete_ambient(
            &mut candidates,
            &mut membership,
            &ambient_proof(&challenge, &key),
            NOW + 2,
            signs("sign/compacted-after-exhaustion"),
        ),
        Err(AdmissionRefusal::AttemptsExhausted)
    );
    assert!(membership.parts.is_empty());
    assert!(manager.receipts.is_empty());
}
