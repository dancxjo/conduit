use super::*;

#[test]
fn expired_spawn_store_retires_without_enlarging_live_capacity_or_losing_part_keys() {
    use conduit_body::{PartReturnProof, MAX_SPAWN_INVITATIONS};
    let body = body();
    let mut manager = AdmissionManager::new(body.body_id.clone()).unwrap();
    let mut membership = BodyMembership::new(body.body_id).unwrap();
    let secret = SpawnInvitationSecret::from_csprng_bytes([91; 32]).unwrap();
    let first = manager
        .issue_spawn_invitation(secret.clone(), [1; 32], NOW, EXPIRES)
        .unwrap();
    let old = advertisement("host/retired-invitation", "boot/old", 1);
    let proof = SpawnAdmissionProof {
        invitation_id: first.invitation_id.clone(),
        body_id: first.body_id.clone(),
        host_id: old.host_id.clone(),
        boot_id: old.boot_id.clone(),
        nonce: first.nonce,
        signature: secret.sign(&first.signing_transcript(
            &old.host_id,
            &old.boot_id,
            old.offer_generation,
        )),
    };
    let admitted = manager
        .complete_spawn(&mut membership, &old, &proof, NOW + 1, signs("sign/first"))
        .unwrap();
    for index in 1..MAX_SPAWN_INVITATIONS {
        manager
            .issue_spawn_invitation(secret.clone(), [index as u8 + 1; 32], NOW, EXPIRES)
            .unwrap();
    }
    let full = manager.clone();
    assert!(matches!(
        manager.issue_spawn_invitation(secret.clone(), [99; 32], NOW + 2, EXPIRES),
        Err(AdmissionRefusal::InvitationCapacityExhausted)
    ));
    assert_eq!(manager, full);
    // An invalid request must not retire records as a side effect of refusal.
    assert!(manager
        .issue_spawn_invitation(secret.clone(), [99; 32], EXPIRES, EXPIRES)
        .is_err());
    assert_eq!(manager, full);
    let receipts = manager.receipts.clone();
    let keys = manager.storage_usage().continuity_keys.len;
    manager
        .issue_spawn_invitation(secret.clone(), [99; 32], EXPIRES, EXPIRES + 1_000)
        .unwrap();
    assert_eq!(manager.storage_usage().invitations.len, 1);
    assert_eq!(manager.receipts, receipts);
    assert_eq!(manager.storage_usage().continuity_keys.len, keys);
    assert_eq!(
        manager.complete_spawn(
            &mut membership,
            &old,
            &proof,
            EXPIRES,
            signs("sign/retired-replay")
        ),
        Err(AdmissionRefusal::UnknownInvitation)
    );
    membership
        .observe_offline(
            &membership.body_id.clone(),
            membership.revision,
            &admitted.part_id,
            &old.boot_id,
            "sign/old-carrier-lost".into(),
        )
        .unwrap();
    let returned = advertisement("host/retired-invitation", "boot/fresh", 2);
    let challenge = manager
        .begin_return(
            &membership,
            &admitted.part_id,
            &returned,
            [100; 32],
            EXPIRES + 1,
            EXPIRES + 1_000,
        )
        .unwrap();
    let returned_proof = PartReturnProof {
        admission_id: challenge.admission_id.clone(),
        body_id: challenge.body_id.clone(),
        part_id: challenge.part_id.clone(),
        host_id: challenge.host_id.clone(),
        boot_id: challenge.boot_id.clone(),
        nonce: challenge.nonce,
        signature: secret.sign(&challenge.signing_transcript()),
    };
    let credential = manager
        .complete_return(
            &mut membership,
            &returned,
            &returned_proof,
            EXPIRES + 2,
            "sign/returned".into(),
        )
        .unwrap();
    assert_eq!(credential.part_id, admitted.part_id);
    assert_eq!(credential.host_id, admitted.host_id);
    assert_eq!(credential.boot_id, returned.boot_id);
}

#[test]
fn spawn_deadline_matches_portable_claim_and_never_accepts_at_expiry() {
    let body = body();
    let mut manager = AdmissionManager::new(body.body_id.clone()).unwrap();
    let mut membership = BodyMembership::new(body.body_id).unwrap();
    let invitation = manager
        .issue_spawn_invitation(
            SpawnInvitationSecret::from_csprng_bytes([101; 32]).unwrap(),
            [102; 32],
            NOW,
            EXPIRES,
        )
        .unwrap();
    let host = advertisement("host/deadline", "boot/deadline", 1);
    let proof = SpawnAdmissionProof {
        invitation_id: invitation.invitation_id.clone(),
        body_id: invitation.body_id.clone(),
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        nonce: invitation.nonce,
        signature: invitation.secret.sign(&invitation.signing_transcript(
            &host.host_id,
            &host.boot_id,
            host.offer_generation,
        )),
    };
    let before = manager.clone();
    assert_eq!(
        invitation.claim().inspect(EXPIRES),
        Err(AdmissionRefusal::Expired)
    );
    assert_eq!(
        manager.complete_spawn(
            &mut membership,
            &host,
            &proof,
            EXPIRES,
            signs("sign/at-expiry")
        ),
        Err(AdmissionRefusal::Expired)
    );
    assert_eq!(manager, before);
    assert!(membership.parts.is_empty());
}

#[test]
fn retired_invitation_identity_does_not_return_after_reopen_and_clock_rollback() {
    let body = body();
    let mut manager = AdmissionManager::new(body.body_id).unwrap();
    let secret = SpawnInvitationSecret::from_csprng_bytes([103; 32]).unwrap();
    let first = manager
        .issue_spawn_invitation(secret.clone(), [104; 32], NOW, EXPIRES)
        .unwrap();
    let second = manager
        .issue_spawn_invitation(secret.clone(), [104; 32], EXPIRES, EXPIRES + 1_000)
        .unwrap();
    assert_ne!(first.invitation_id, second.invitation_id);
    let mut reopened: AdmissionManager =
        serde_json::from_slice(&serde_json::to_vec(&manager).unwrap()).unwrap();
    let third = reopened
        .issue_spawn_invitation(secret, [104; 32], NOW, EXPIRES)
        .unwrap();
    assert_ne!(first.invitation_id, third.invitation_id);
    assert_ne!(second.invitation_id, third.invitation_id);
    assert_eq!(reopened.storage_usage().invitations.len, 2);
    let mut record = serde_json::to_value(&reopened).unwrap();
    record["next_invitation_sequence"] = serde_json::json!(u64::MAX);
    let mut exhausted: AdmissionManager = serde_json::from_value(record).unwrap();
    let before = exhausted.clone();
    assert!(matches!(
        exhausted.issue_spawn_invitation(
            SpawnInvitationSecret::from_csprng_bytes([105; 32]).unwrap(),
            [106; 32],
            EXPIRES + 1_000,
            EXPIRES + 2_000
        ),
        Err(AdmissionRefusal::InvitationSequenceExhausted)
    ));
    assert_eq!(exhausted, before);
}

#[test]
fn retained_manager_without_sequence_field_preserves_invitations_on_open() {
    let mut manager = AdmissionManager::new(body().body_id).unwrap();
    let first = manager
        .issue_spawn_invitation(
            SpawnInvitationSecret::from_csprng_bytes([107; 32]).unwrap(),
            [108; 32],
            NOW,
            EXPIRES,
        )
        .unwrap();
    let mut legacy = serde_json::to_value(&manager).unwrap();
    legacy
        .as_object_mut()
        .unwrap()
        .remove("next_invitation_sequence");
    let mut reopened: AdmissionManager = serde_json::from_value(legacy).unwrap();
    assert_eq!(reopened.storage_usage().invitations.len, 1);
    let second = reopened
        .issue_spawn_invitation(
            SpawnInvitationSecret::from_csprng_bytes([107; 32]).unwrap(),
            [109; 32],
            NOW,
            EXPIRES,
        )
        .unwrap();
    assert_ne!(first.invitation_id, second.invitation_id);
    assert_eq!(reopened.storage_usage().invitations.len, 2);
}
