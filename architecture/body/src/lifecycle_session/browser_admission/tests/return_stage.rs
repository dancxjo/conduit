//! Real retained-key return parity and proposal-only custody.
use super::*;

fn returning() -> (
    BodyLifecycleSession,
    AdmissionManager,
    HostAdvertisement,
    PartReturnProof,
) {
    let mut session = session();
    let (mut manager, mut inventory, proof, key) = pending(&session);
    let credential = session
        .admit_ambient_host(&mut manager, &mut inventory, &proof, 110, &host(), &boot())
        .unwrap();
    session
        .observe_host_lost(&credential.host_id, &credential.boot_id, &host(), &boot())
        .unwrap();
    let fresh = advertisement("boot/browser/2");
    let challenge = manager
        .begin_return(
            &session.evidence.membership,
            &credential.part_id,
            &fresh,
            [3; 32],
            120,
            200,
        )
        .unwrap();
    let signature = key.sign(&challenge.signing_transcript()).to_bytes();
    let proof = PartReturnProof {
        admission_id: challenge.admission_id,
        body_id: challenge.body_id,
        part_id: challenge.part_id,
        host_id: challenge.host_id,
        boot_id: challenge.boot_id,
        nonce: challenge.nonce,
        signature,
    };
    (session, manager, fresh, proof)
}
fn unchanged(actual: &BodyLifecycleSession, before: &BodyLifecycleSession) {
    assert_eq!(actual.evidence, before.evidence);
    assert_eq!(actual.realization, before.realization);
    assert_eq!(actual.foreground, before.foreground);
    assert_eq!(actual.pending_archives, before.pending_archives);
}
#[test]
fn owned_stage_matches_mutating_return_without_adopting_original_session() {
    let (session, manager, fresh, proof) = returning();
    let before = session.clone();
    let manager_before = manager.clone();
    let mut owned_manager = manager.clone();
    // Spare metadata capacity lets this assert ownership, not resource grants.
    owned_manager.receipts.reserve(1);
    let receipts = owned_manager.receipts.as_ptr();
    let body_id = owned_manager.body_id.as_str().as_ptr();
    let (proposed, staged_manager, staged_credential) = session
        .stage_returning_host(owned_manager, &fresh, &proof, 130, (&host(), &boot()))
        .unwrap()
        .into_parts();
    unchanged(&session, &before);
    assert_eq!(manager, manager_before);
    let mut applied = session.clone();
    let mut applied_manager = manager;
    let credential = applied
        .admit_returning_host(&mut applied_manager, &fresh, &proof, 130, &host(), &boot())
        .unwrap();
    unchanged(&proposed, &applied);
    assert_eq!(staged_manager, applied_manager);
    assert_eq!(staged_manager.receipts.as_ptr(), receipts);
    assert_eq!(staged_manager.body_id.as_str().as_ptr(), body_id);
    assert_eq!(staged_credential, credential);
    assert_ne!(proposed.evidence, before.evidence);
    proposed.evidence.validate().unwrap();
}
#[test]
fn failed_proof_and_wrong_authority_preserve_original_and_match_existing_refusals() {
    for wrong_authority in [false, true] {
        let (session, manager, fresh, mut proof) = returning();
        if !wrong_authority {
            proof.signature[0] ^= 1;
        }
        let authority_boot = if wrong_authority {
            "boot/wrong".into()
        } else {
            boot()
        };
        let before = session.clone();
        let manager_before = manager.clone();
        let stage_error = session
            .stage_returning_host(
                manager.clone(),
                &fresh,
                &proof,
                130,
                (&host(), &authority_boot),
            )
            .err()
            .unwrap();
        let mut applied = session.clone();
        let mut applied_manager = manager;
        let existing_error = applied
            .admit_returning_host(
                &mut applied_manager,
                &fresh,
                &proof,
                130,
                &host(),
                &authority_boot,
            )
            .unwrap_err();
        assert_eq!(stage_error, existing_error);
        unchanged(&session, &before);
        unchanged(&applied, &before);
        assert_eq!(applied_manager, manager_before);
    }
}
