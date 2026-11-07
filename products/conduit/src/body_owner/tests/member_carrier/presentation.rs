//! CPU custody proof: fixture Lines/Shows are explicit, never physical evidence.
use super::*;
use conduit_body::{PortableSpawnAdmissionRequest, SPAWN_ADMISSION_REQUEST_SCHEMA};
use conduit_face::{MaskShow, OwnerFaceSnapshotRequest};
fn native_member(
    owner: &mut Owner,
    root: &std::path::Path,
) -> (
    conduit_body::MembershipCredential,
    conduit_body::SpawnInvitationSecret,
    conduit_core::HostAdvertisement,
) {
    let invitation = owner.issue_invitation(root, 60, None).unwrap();
    let provider = conduit_core::BaseProviderEntry {
        base_id: "fixture/framebuffer".into(),
        provider_instance_id: "fixture/framebuffer/1".into(),
        provider_generation: 1,
        implementation_id: "conduitos/framebuffer@1".into(),
        mechanism_family: "conduitos.base/framebuffer@1".into(),
        enforcement_class: conduit_core::BaseEnforcementClass::ConduitOsKernelEnforced,
        lifecycle: conduit_core::BaseLifecycle::Ready,
        capabilities: vec![],
        resources: vec![conduit_core::resource_offer(
            "fixture/surface",
            conduit_face::SHOW_RESOURCE_CLASS,
            1,
        )],
    };
    let ad = conduit_conduitos_mask_offer::native_host_advertisement(
        &"fixture/native".into(),
        &"fixture/native/boot".into(),
        1,
        &provider,
    );
    let key = conduit_body::SpawnInvitationSecret::from_csprng_bytes(invitation.secret).unwrap();
    let request = PortableSpawnAdmissionRequest {
        schema: SPAWN_ADMISSION_REQUEST_SCHEMA.into(),
        invitation_id: invitation.claim.invitation_id.clone(),
        body_id: invitation.claim.body_id.clone(),
        host_advertisement: ad.clone(),
        nonce: invitation.claim.nonce,
        signature: key
            .sign(&invitation.claim.signing_transcript(
                &ad.host_id,
                &ad.boot_id,
                ad.offer_generation,
            ))
            .to_vec(),
        membership_admitted: false,
        plan_created: false,
        play_created: false,
    };
    let credential = owner
        .admit_invited(root, request, ad.host_id.as_str())
        .unwrap()
        .credential;
    (credential, key, ad)
}
fn lines(
    owner: &Owner,
    credential: &conduit_body::MembershipCredential,
    ad: &conduit_core::HostAdvertisement,
    session: &conduit_core::LinkBindingId,
) -> (conduit_core::LineOffer, conduit_core::LineOffer) {
    crate::durable_host::routed_join::native_lines::offer_pair(
        owner.host.advertisement(),
        &crate::durable_host::owner::MemberPresentationBasis::new(credential.clone(), ad.clone())
            .unwrap(),
        session.as_str(),
    )
}
fn show(
    owner: &mut Owner,
    seal: &conduit_face::RemoteOwnerMaskRouteSeal,
    sequence: u64,
) -> MaskShow {
    let face = owner.local_face_snapshot().unwrap();
    let placement = seal.planned_mask.show_placement();
    MaskShow::prepared(
        &seal.planned_mask,
        &face,
        conduit_core::bind_active_play(
            &seal.planned_mask.plan.plan_id,
            &placement.host_id,
            &placement.boot_id,
            sequence,
        ),
        face.subjects[0].identity.clone(),
        "fixture/native".into(),
        format!("sign/fixture/prepared/{sequence}").into(),
    )
    .unwrap()
    .transition(
        conduit_face::ManifestationLifecycle::Available,
        format!("sign/fixture/available/{sequence}").into(),
    )
    .unwrap()
}
fn request(c: &conduit_body::MembershipCredential) -> OwnerFaceSnapshotRequest {
    OwnerFaceSnapshotRequest {
        schema: conduit_face::OWNER_FACE_REQUEST_SCHEMA.into(),
        credential_id: c.credential_id.as_str().into(),
        body_id: c.body_id.clone(),
        part_id: c.part_id.clone(),
        host_id: c.host_id.clone(),
        boot_id: c.boot_id.clone(),
        last_seen_revision: None,
        last_seen_identity: None,
    }
}
#[test]
fn current_carrier_reseals_and_exact_cleanup_preserve_membership() {
    let (mut owner, root) = owner_fixture();
    let (credential, key, ad) = native_member(&mut owner, &root);
    let before = owner.session.evidence().clone();
    let admissions = owner.admissions.clone();
    let (challenge, _) = begin(&mut owner, &credential, &ad);
    owner
        .complete_current_member_carrier(&challenge, signature(&challenge, &key))
        .unwrap();
    let session = &challenge.session_id;
    let (face, returned) = lines(&owner, &credential, &ad, session);
    assert!(owner
        .select_member_carrier_mask_route(session, &face, &returned)
        .is_err());
    owner.renew_current_member_presence(session, 1).unwrap();
    let mut wrong = face.clone();
    wrong.binding.limits.maximum_payload_bytes += 1;
    assert!(owner
        .select_member_carrier_mask_route(session, &wrong, &returned)
        .is_err());
    let req = request(&credential);
    let mut old_show = None;
    for sequence in 1..=8 {
        owner
            .renew_current_member_presence(session, sequence + 1)
            .unwrap();
        let seal = owner
            .select_member_carrier_mask_route(session, &face, &returned)
            .unwrap();
        let shown = show(&mut owner, &seal, sequence);
        owner
            .acknowledge_member_carrier_show(session, &req, &shown)
            .unwrap();
        old_show = Some(shown);
    }
    owner.retire_current_member_carrier(session).unwrap();
    let (fresh, _) = begin(&mut owner, &credential, &ad);
    owner
        .complete_current_member_carrier(&fresh, signature(&fresh, &key))
        .unwrap();
    owner
        .renew_current_member_presence(&fresh.session_id, 1)
        .unwrap();
    let (face, returned) = lines(&owner, &credential, &ad, &fresh.session_id);
    let seal = owner
        .select_member_carrier_mask_route(&fresh.session_id, &face, &returned)
        .unwrap();
    assert!(owner.retire_current_member_carrier(session).is_err());
    assert!(owner
        .acknowledge_member_carrier_show(session, &req, old_show.as_ref().unwrap())
        .is_err());
    assert!(owner
        .acknowledge_member_carrier_show(&fresh.session_id, &req, old_show.as_ref().unwrap())
        .is_err());
    let shown = show(&mut owner, &seal, 9);
    owner
        .acknowledge_member_carrier_show(&fresh.session_id, &req, &shown)
        .unwrap();
    assert_eq!(owner.session.evidence(), &before);
    assert_eq!(owner.admissions, admissions);
    assert_eq!(state::load(&root).unwrap().unwrap(), before);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn short_presence_expiry_withdraws_native_witness_without_member_departure() {
    let (mut owner, root) = owner_fixture();
    let (credential, key, ad) = native_member(&mut owner, &root);
    let before = owner.session.evidence().clone();
    let (challenge, _) = begin(&mut owner, &credential, &ad);
    owner
        .complete_current_member_carrier(&challenge, signature(&challenge, &key))
        .unwrap();
    owner
        .renew_current_member_presence(&challenge.session_id, 1)
        .unwrap();
    let (face_line, returned) = lines(&owner, &credential, &ad, &challenge.session_id);
    let seal = owner
        .select_member_carrier_mask_route(&challenge.session_id, &face_line, &returned)
        .unwrap();
    let shown = show(&mut owner, &seal, 1);
    let face = owner.local_face_snapshot().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(2050));
    let route = owner.pending_native_mask.as_ref().unwrap();
    assert!(route
        .current_witness(
            &owner.session,
            &face,
            crate::durable_host::current_time_millis().unwrap()
        )
        .is_none());
    assert!(owner
        .acknowledge_member_carrier_show(&challenge.session_id, &request(&credential), &shown)
        .is_err());
    assert!(owner
        .renew_current_member_presence(&challenge.session_id, 2)
        .is_err());
    assert_eq!(owner.session.evidence(), &before);
    std::fs::remove_dir_all(root).unwrap();
}
