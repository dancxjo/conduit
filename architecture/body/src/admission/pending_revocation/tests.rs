use super::*;
use crate::{
    AdmissionRefusal, AdmissionSigns, Body, BodyMembership, SpawnAdmissionProof, SpawnInvitation,
    SpawnInvitationSecret,
};
use alloc::vec;
use conduit_core::*;

fn manager() -> AdmissionManager {
    let body = Body::born(
        "source/revoke".into(),
        "checked/revoke".into(),
        1,
        "sign/revoke".into(),
    )
    .unwrap();
    AdmissionManager::new(body.body_id).unwrap()
}
fn issue(manager: &mut AdmissionManager, byte: u8) -> SpawnInvitation {
    manager
        .issue_spawn_invitation(
            SpawnInvitationSecret::from_csprng_bytes([byte; 32]).unwrap(),
            [byte; 32],
            10,
            1000,
        )
        .unwrap()
}
fn advertisement() -> HostAdvertisement {
    HostAdvertisement {
        declared_name: None,
        protocol_version: PROTOCOL_VERSION,
        host_id: "host/next".into(),
        boot_id: "boot/next".into(),
        offer_generation: OfferGeneration(1),
        profile: "profile/test".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    }
}
fn proof(invitation: &SpawnInvitation, host: &HostAdvertisement) -> SpawnAdmissionProof {
    SpawnAdmissionProof {
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
    }
}
fn signs() -> AdmissionSigns {
    AdmissionSigns {
        part_admitted: "sign/part".into(),
        host_attached: "sign/host".into(),
        candidate_admitted: "sign/candidate".into(),
    }
}

#[test]
fn owned_revocation_preserves_other_invitation_and_monotonic_sequence() {
    let mut manager = manager();
    let first = issue(&mut manager, 11);
    let other = issue(&mut manager, 12);
    let sequence = manager.next_invitation_sequence;
    manager.revoke_pending_spawn_invitation(&first).unwrap();
    assert_eq!(manager.next_invitation_sequence, sequence);
    let again = issue(&mut manager, 11);
    assert_ne!(again.invitation_id, first.invitation_id);
    assert_eq!(manager.next_invitation_sequence, sequence + 1);
    let host = advertisement();
    let mut membership = BodyMembership::new(manager.body_id.clone()).unwrap();
    manager
        .complete_spawn(&mut membership, &host, &proof(&other, &host), 20, signs())
        .unwrap();
    assert_eq!(membership.parts.len(), 1);
}

#[test]
fn foreign_claim_secret_nonce_and_times_refuse_without_mutation() {
    use PendingInvitationRevocationRefusal::*;
    let mut manager = manager();
    let invitation = issue(&mut manager, 21);
    let original = manager.clone();
    assert_eq!(
        manager.validate_owned_pending_spawn_invitation(&invitation),
        Ok(())
    );
    assert_eq!(manager, original);
    let mut wrong_secret = invitation.clone();
    wrong_secret.secret = SpawnInvitationSecret::from_csprng_bytes([22; 32]).unwrap();
    assert_eq!(
        manager.revoke_pending_spawn_invitation(&wrong_secret),
        Err(SecretMismatch)
    );
    assert_eq!(
        manager.validate_owned_pending_spawn_invitation(&wrong_secret),
        Err(SecretMismatch)
    );
    assert_eq!(manager, original);
    for field in 0..5 {
        let mut claim = invitation.clone();
        let expected = match field {
            0 => {
                claim.body_id =
                    Body::born("s/other".into(), "c/other".into(), 2, "sign/other".into())
                        .unwrap()
                        .body_id;
                WrongBody
            }
            1 => {
                claim.invitation_id = issue(&mut self::manager(), 23).invitation_id;
                UnknownInvitation
            }
            2 => {
                claim.nonce[0] ^= 1;
                ClaimMismatch
            }
            3 => {
                claim.issued_at_millis += 1;
                ClaimMismatch
            }
            _ => {
                claim.expires_at_millis += 1;
                ClaimMismatch
            }
        };
        assert_eq!(
            manager.validate_owned_pending_spawn_invitation(&claim),
            Err(expected)
        );
        assert_eq!(manager, original);
        assert_eq!(
            manager.revoke_pending_spawn_invitation(&claim),
            Err(expected)
        );
        assert_eq!(manager, original);
    }
}

#[test]
fn queued_proof_cannot_admit_after_revocation() {
    let mut manager = manager();
    let invitation = issue(&mut manager, 31);
    let host = advertisement();
    // An external request may already hold a complete, valid signed proof.
    let queued = proof(&invitation, &host);
    let mut membership = BodyMembership::new(manager.body_id.clone()).unwrap();
    manager
        .revoke_pending_spawn_invitation(&invitation)
        .unwrap();
    let before = manager.clone();
    assert_eq!(
        manager.complete_spawn(&mut membership, &host, &queued, 20, signs()),
        Err(AdmissionRefusal::UnknownInvitation)
    );
    assert_eq!(manager, before);
    assert!(membership.parts.is_empty());
    assert!(manager.receipts.is_empty());
}

#[test]
fn completed_admission_refuses_revocation_without_erasing_part_or_receipt() {
    let mut manager = manager();
    let invitation = issue(&mut manager, 41);
    let host = advertisement();
    let mut membership = BodyMembership::new(manager.body_id.clone()).unwrap();
    manager
        .complete_spawn(
            &mut membership,
            &host,
            &proof(&invitation, &host),
            20,
            signs(),
        )
        .unwrap();
    let before = manager.clone();
    let parts = membership.clone();
    assert_eq!(
        manager.validate_owned_pending_spawn_invitation(&invitation),
        Err(PendingInvitationRevocationRefusal::AlreadyUsed)
    );
    assert_eq!(manager, before);
    assert_eq!(
        manager.revoke_pending_spawn_invitation(&invitation),
        Err(PendingInvitationRevocationRefusal::AlreadyUsed)
    );
    assert_eq!(manager, before);
    assert_eq!(membership, parts);
    assert_eq!(manager.receipts.len(), 1);
}

#[test]
fn expired_owned_invitation_can_retire_but_a_repeat_does_not_claim_success() {
    let mut manager = manager();
    let invitation = issue(&mut manager, 51);
    assert_eq!(
        invitation.claim().inspect(1000),
        Err(AdmissionRefusal::Expired)
    );
    manager
        .revoke_pending_spawn_invitation(&invitation)
        .unwrap();
    let before = manager.clone();
    assert_eq!(
        manager.revoke_pending_spawn_invitation(&invitation),
        Err(PendingInvitationRevocationRefusal::UnknownInvitation)
    );
    assert_eq!(manager, before);
}

#[test]
fn retiring_another_invitation_preserves_existing_admission_custody() {
    let mut manager = manager();
    let admitted = issue(&mut manager, 61);
    let host = advertisement();
    let mut membership = BodyMembership::new(manager.body_id.clone()).unwrap();
    manager
        .complete_spawn(
            &mut membership,
            &host,
            &proof(&admitted, &host),
            20,
            signs(),
        )
        .unwrap();
    let pending = issue(&mut manager, 62);
    let receipts = manager.receipts.clone();
    let keys = manager.continuity_keys.clone();
    let parts = membership.clone();
    manager.revoke_pending_spawn_invitation(&pending).unwrap();
    assert_eq!(manager.receipts, receipts);
    assert_eq!(manager.continuity_keys, keys);
    assert_eq!(membership, parts);
    assert_eq!(
        manager.complete_spawn(
            &mut membership,
            &host,
            &proof(&admitted, &host),
            21,
            signs()
        ),
        Err(AdmissionRefusal::Replay)
    );
}

#[test]
fn readonly_check_does_not_assert_time_validity_and_refuses_pruned_record() {
    let mut manager = manager();
    let invitation = issue(&mut manager, 71);
    assert_eq!(
        invitation.claim().inspect(1000),
        Err(AdmissionRefusal::Expired)
    );
    let before = manager.clone();
    assert_eq!(
        manager.validate_owned_pending_spawn_invitation(&invitation),
        Ok(())
    );
    assert_eq!(manager, before);
    manager.invitations.clear();
    let pruned = manager.clone();
    assert_eq!(
        manager.validate_owned_pending_spawn_invitation(&invitation),
        Err(PendingInvitationRevocationRefusal::UnknownInvitation)
    );
    assert_eq!(manager, pruned);
}
