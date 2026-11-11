//! Explicit owner policy recovers continuity, never stale execution authority.
use super::*;

fn presentation(
    prior: &conduit_body::MembershipCredential,
    ad: &conduit_core::HostAdvertisement,
    key: &SpawnInvitationSecret,
) -> conduit_body::MemberReturnImagePresentation {
    let mut context = ad.clone();
    context.capabilities.clear();
    let projection = conduit_body::HostAdvertisementImageProjection {
        context,
        capability_images: vec![],
        authority_grants: vec![],
    };
    conduit_body::MemberReturnImagePresentation {
        kind: "Advertise".into(),
        schema: conduit_body::MEMBER_RETURN_IMAGES_SCHEMA.into(),
        credential: prior.clone(),
        signature: key
            .sign(&conduit_body::member_return_image_transcript(prior, &projection).unwrap())
            .into(),
        projection,
    }
}

#[test]
fn retained_key_recovery_uses_latest_basis_but_preserves_old_signed_presentation() {
    let (mut owner, root) = owner_fixture();
    let (anchor, key, mut ad) = admit_with_key(&mut owner, &root, "host/recover-anchor");
    ad.capabilities.clear();
    ad.boot_id = "boot/recover/2".into();
    let challenge = owner.begin_member_return(&anchor, &ad).unwrap();
    let latest = owner
        .complete_member_return(&root, &prove(&challenge, &key))
        .unwrap();
    owner.pending_member_return = None;
    // An intermediate credential need not survive hot receipt compaction.
    owner
        .admissions
        .as_mut()
        .unwrap()
        .receipts
        .retain(|r| r.credential == latest);
    ad.boot_id = "boot/recover/3".into();
    assert!(owner.begin_member_return(&anchor, &ad).is_err());
    let claim = presentation(&anchor, &ad, &key);
    let challenge = owner
        .begin_recovered_member_image_return(&anchor, &ad, claim.clone())
        .unwrap();
    let pending = owner.pending_member_return.as_ref().unwrap();
    assert_eq!(pending.prior, latest);
    assert!(pending.presentation.as_ref().unwrap() == &claim);
    assert_eq!(
        owner
            .begin_recovered_member_image_return(&anchor, &ad, claim.clone())
            .unwrap(),
        challenge
    );
    assert!(owner
        .begin_member_image_return(&anchor, &ad, Some(claim.clone()))
        .is_err());
    let proof = prove(&challenge, &key);
    let current = owner.complete_member_return(&root, &proof).unwrap();
    let committed = owner.session.evidence().clone();
    assert_eq!(
        owner
            .begin_recovered_member_image_return(&anchor, &ad, claim.clone())
            .unwrap(),
        challenge
    );
    assert_eq!(
        owner.complete_member_return(&root, &proof).unwrap(),
        current
    );
    assert_eq!(owner.session.evidence(), &committed);
    owner.pending_member_return = None;
    assert!(owner
        .begin_recovered_member_image_return(&anchor, &ad, claim)
        .is_err());
    ad.boot_id = "boot/recover/4".into();
    let next = owner
        .begin_recovered_member_image_return(&anchor, &ad, presentation(&anchor, &ad, &key))
        .unwrap();
    assert!(owner.complete_member_return(&root, &proof).is_err());
    owner
        .complete_member_return(&root, &prove(&next, &key))
        .unwrap();
    let committed = owner.session.evidence().clone();
    assert!(owner.complete_member_return(&root, &proof).is_err());
    assert_eq!(owner.session.evidence(), &committed);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn retained_key_recovery_refuses_foreign_scope_bad_key_and_changed_latest() {
    let (mut owner, root) = owner_fixture();
    let (prior, key, mut ad) = admit_with_key(&mut owner, &root, "host/recover-refusals");
    ad.capabilities.clear();
    ad.boot_id = "boot/recover-refusals/2".into();
    let mut foreign = prior.clone();
    foreign.body_id = "body/foreign".into();
    assert!(owner
        .begin_recovered_member_image_return(&foreign, &ad, presentation(&foreign, &ad, &key))
        .is_err());
    foreign = prior.clone();
    foreign.host_id = "host/foreign".into();
    assert!(owner
        .begin_recovered_member_image_return(&foreign, &ad, presentation(&foreign, &ad, &key))
        .is_err());
    let wrong = SpawnInvitationSecret::from_csprng_bytes([91; 32]).unwrap();
    let challenge = owner
        .begin_recovered_member_image_return(&prior, &ad, presentation(&prior, &ad, &wrong))
        .unwrap();
    let before = owner.session.evidence().clone();
    let manager = owner.admissions.clone();
    let offers = owner.current_offer_summary();
    assert!(owner
        .complete_member_return(&root, &prove(&challenge, &wrong))
        .is_err());
    assert_eq!(owner.session.evidence(), &before);
    assert_eq!(owner.admissions, manager);
    assert_eq!(owner.current_offer_summary(), offers);
    // Even a correct challenge proof cannot authorize a wrong signed projection.
    assert!(owner
        .complete_member_return(&root, &prove(&challenge, &key))
        .is_err());
    assert_eq!(owner.session.evidence(), &before);
    owner.pending_member_return = None;
    let challenge = owner
        .begin_recovered_member_image_return(&prior, &ad, presentation(&prior, &ad, &key))
        .unwrap();
    owner
        .admissions
        .as_mut()
        .unwrap()
        .receipts
        .last_mut()
        .unwrap()
        .credential
        .credential_id = "credential/concurrent-change".into();
    let manager = owner.admissions.clone();
    assert!(owner
        .complete_member_return(&root, &prove(&challenge, &key))
        .is_err());
    assert_eq!(owner.admissions, manager);
    assert_eq!(owner.session.evidence(), &before);
    assert_eq!(owner.current_offer_summary(), offers);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn retained_key_recovery_refuses_revocation_and_changed_attachment() {
    let (mut owner, root) = owner_fixture();
    let (prior, key, mut ad) = admit_with_key(&mut owner, &root, "host/recover-revoked");
    ad.capabilities.clear();
    ad.boot_id = "boot/recover-revoked/2".into();
    let challenge = owner
        .begin_recovered_member_image_return(&prior, &ad, presentation(&prior, &ad, &key))
        .unwrap();
    owner.member_leave(&root, &prior).unwrap();
    let before = owner.session.evidence().clone();
    assert!(owner
        .complete_member_return(&root, &prove(&challenge, &key))
        .is_err());
    assert_eq!(owner.session.evidence(), &before);
    owner.pending_member_return = None;
    let revoked_challenge = owner
        .begin_recovered_member_image_return(&prior, &ad, presentation(&prior, &ad, &key))
        .unwrap();
    let mut revoked = before;
    let revision = revoked.membership.revision;
    revoked
        .membership
        .revoke(
            &prior.body_id,
            revision,
            &prior.part_id,
            "sign/revoke-recovery".into(),
        )
        .unwrap();
    owner.session = conduit_body::BodyLifecycleSession::open(revoked).unwrap();
    let before = owner.session.evidence().clone();
    assert!(owner
        .complete_member_return(&root, &prove(&revoked_challenge, &key))
        .is_err());
    owner.pending_member_return = None;
    assert!(owner
        .begin_recovered_member_image_return(&prior, &ad, presentation(&prior, &ad, &key))
        .is_err());
    assert_eq!(owner.session.evidence(), &before);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn retained_key_recovery_cannot_use_a_replaced_continuity_key() {
    let (mut owner, root) = owner_fixture();
    let (prior, key, mut ad) = admit_with_key(&mut owner, &root, "host/recover-key-change");
    ad.capabilities.clear();
    ad.boot_id = "boot/recover-key-change/2".into();
    let challenge = owner
        .begin_recovered_member_image_return(&prior, &ad, presentation(&prior, &ad, &key))
        .unwrap();
    // Model an independently retained key replacement; no production key-change
    // API or mutation authority is introduced by this fixture.
    let replacement = SpawnInvitationSecret::from_csprng_bytes([92; 32]).unwrap();
    let mut manager = serde_json::to_value(owner.admissions.as_ref().unwrap()).unwrap();
    let row = manager["continuity_keys"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|row| row["part_id"].as_str() == Some(prior.part_id.as_str()))
        .unwrap();
    row["verifying_key"] = serde_json::to_value(replacement.verifying_key()).unwrap();
    owner.admissions = Some(serde_json::from_value(manager).unwrap());
    let before = owner.session.evidence().clone();
    let manager = owner.admissions.clone();
    let offers = owner.current_offer_summary();
    assert!(owner
        .complete_member_return(&root, &prove(&challenge, &key))
        .is_err());
    assert_eq!(owner.session.evidence(), &before);
    assert_eq!(owner.admissions, manager);
    assert_eq!(owner.current_offer_summary(), offers);
    std::fs::remove_dir_all(root).unwrap();
}
