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
    native_member_profile(owner, root, true)
}
fn native_member_profile(
    owner: &mut Owner,
    root: &std::path::Path,
    interactive: bool,
) -> (
    conduit_body::MembershipCredential,
    conduit_body::SpawnInvitationSecret,
    conduit_core::HostAdvertisement,
) {
    let invitation = owner.issue_invitation(root, 60, None).unwrap();
    let mut provider = conduit_core::BaseProviderEntry {
        base_id: "fixture/framebuffer".into(),
        provider_instance_id: "fixture/framebuffer/1".into(),
        provider_generation: 1,
        implementation_id: "conduitos/framebuffer@1".into(),
        mechanism_family: "conduitos.base/framebuffer@1".into(),
        enforcement_class: conduit_core::BaseEnforcementClass::ConduitOsKernelEnforced,
        lifecycle: conduit_core::BaseLifecycle::Ready,
        capabilities: vec![],
        resources: vec![
            conduit_core::resource_offer("fixture/surface", conduit_face::SHOW_RESOURCE_CLASS, 1),
            conduit_core::resource_offer(
                "fixture/input",
                "conduit.resource/conduitos-human-input@1",
                1,
            ),
        ],
    };
    if !interactive {
        provider
            .resources
            .retain(|pool| pool.class_id.as_str() != "conduit.resource/conduitos-human-input@1");
    }
    provider.resources.sort_by(|a, b| a.pool_id.cmp(&b.pool_id));
    let mut ad = if interactive {
        conduit_conduitos_mask_offer::native_host_advertisement(
            &"fixture/native".into(),
            &"fixture/native/boot".into(),
            1,
            &provider,
        )
    } else {
        conduit_conduitos_mask_offer::native_read_only_host_advertisement(
            &"fixture/native".into(),
            &"fixture/native/boot".into(),
            1,
            &provider,
            conduit_conduitos_mask_offer::NativeMaskStorageProfile::Standard,
        )
    };
    // Model the exact initialized registry BEFORE signed admission: three
    // physical Backs owned by the declared provider, plus a pure unowned tee.
    provider.capabilities = ad
        .capabilities
        .iter()
        .filter(|cap| {
            [
                conduit_face::SHOW_RESOURCE_SOURCE_KIND,
                conduit_face::RESOURCE_RENDERER_KIND,
                conduit_face::FACE_INTERACTION_KIND,
            ]
            .contains(&cap.kind_id.as_str())
        })
        .cloned()
        .collect();
    ad.capabilities.retain(|cap| {
        cap.kind_id.as_str() == conduit_face::FACE_TEE_KIND || provider.capabilities.contains(cap)
    });
    ad.resources = provider.resources.clone();
    ad.bases[0].capability_ids = provider
        .capabilities
        .iter()
        .map(|cap| cap.capability_id.clone())
        .collect();
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
            &owner.current_host_offers,
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

#[test]
fn member_interaction_requires_same_live_carrier_and_acknowledged_show() {
    let (mut owner, root) = owner_fixture();
    let (credential, key, ad) = native_member(&mut owner, &root);
    let (challenge, _) = begin(&mut owner, &credential, &ad);
    owner
        .complete_current_member_carrier(&challenge, signature(&challenge, &key))
        .unwrap();
    let session = &challenge.session_id;
    owner.renew_current_member_presence(session, 1).unwrap();
    let member = crate::durable_host::owner::MemberPresentationBasis {
        credential: credential.clone(),
        host_advertisement: ad.clone(),
    };
    let (face_line, returned, interaction_line) =
        crate::durable_host::routed_join::native_lines::offer_graphical(
            owner.host.advertisement(),
            &member,
            session.as_str(),
        )
        .unwrap();
    let interaction_line = interaction_line.unwrap();
    let seal = owner
        .select_member_carrier_mask_route_with_interaction(
            session,
            &face_line,
            &returned,
            Some(&interaction_line),
        )
        .unwrap();
    let shown = show(&mut owner, &seal, 1);
    let req = request(&credential);
    let face = owner.local_face_snapshot().unwrap();
    let action = face
        .actions
        .iter()
        .find(|action| {
            action.arguments.is_empty()
                && action.availability == conduit_face::FaceActionAvailability::Available
        })
        .unwrap();
    let interaction = conduit_face::FaceInteraction::new(
        &face,
        &shown,
        &action.identity,
        &action.target,
        vec![],
        1,
    )
    .unwrap();
    assert!(owner
        .validate_member_carrier_interaction(session, &req, &shown, &interaction)
        .is_err());
    owner
        .acknowledge_member_carrier_show(session, &req, &shown)
        .unwrap();
    owner
        .validate_member_carrier_interaction(session, &req, &shown, &interaction)
        .unwrap();
    let mut substituted = req.clone();
    substituted.boot_id = "substituted/boot".into();
    assert!(owner
        .validate_member_carrier_interaction(session, &substituted, &shown, &interaction)
        .is_err());
    owner.retire_current_member_carrier(session).unwrap();
    assert!(owner
        .validate_member_carrier_interaction(session, &req, &shown, &interaction)
        .is_err());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn graphical_carrier_binds_exact_third_line_without_selecting_terminal_mask() {
    let (mut owner, root) = owner_fixture();
    let (credential, key, ad) = native_member(&mut owner, &root);
    let (challenge, _) = begin(&mut owner, &credential, &ad);
    owner
        .complete_current_member_carrier(&challenge, signature(&challenge, &key))
        .unwrap();
    owner
        .renew_current_member_presence(&challenge.session_id, 1)
        .unwrap();
    let member = crate::durable_host::owner::MemberPresentationBasis {
        credential: credential.clone(),
        host_advertisement: ad.clone(),
    };
    let (face, show_line, interaction) =
        crate::durable_host::routed_join::native_lines::offer_graphical(
            owner.host.advertisement(),
            &member,
            challenge.session_id.as_str(),
        )
        .unwrap();
    let interaction = interaction.unwrap();
    let mut wrong = interaction.clone();
    wrong.binding.binding_id = "fixture/foreign-interaction".into();
    assert!(owner
        .select_member_carrier_mask_route_with_interaction(
            &challenge.session_id,
            &face,
            &show_line,
            Some(&wrong)
        )
        .is_err());
    let seal = owner
        .select_member_carrier_mask_route_with_interaction(
            &challenge.session_id,
            &face,
            &show_line,
            Some(&interaction),
        )
        .unwrap();
    assert_eq!(seal.interaction_line, Some(interaction.admitted_line()));
    // A compact pin resolves actual admitted custody, never a sorted index or
    // a replacement inventory with matching Host/Boot/generation labels.
    let pin = owner.pin_admitted_member_offer(&ad).unwrap();
    assert!(pin
        .current(&owner.current_host_offers, owner.session.evidence())
        .is_some());
    let saved_offers = owner.current_host_offers.clone();
    let mut changed_ad = ad.clone();
    changed_ad.profile = "fixture/replaced-same-generation".into();
    owner
        .current_host_offers
        .observe(owner.session.evidence(), changed_ad)
        .unwrap();
    assert!(pin
        .current(&owner.current_host_offers, owner.session.evidence())
        .is_none());
    assert!(owner
        .inspect_current_member_carrier(&challenge.session_id)
        .is_err());
    owner.current_host_offers = saved_offers;
    assert!(owner
        .inspect_current_member_carrier(&challenge.session_id)
        .is_ok());

    #[cfg(feature = "native-display")]
    {
        // Capacity inspection is independent of wire size and semantic identity.
        let initial = native_mask_route::backing::seal_payload_bytes(&seal).unwrap();
        let mut enlarged = seal.clone();
        let original = enlarged.owner_host.offer_digest.capacity();
        enlarged.owner_host.offer_digest.reserve(193);
        let delta = enlarged.owner_host.offer_digest.capacity() - original;
        let cloned = native_mask_route::backing::seal_payload_bytes(&seal.clone()).unwrap();
        assert!(cloned <= initial);
        assert_eq!(
            native_mask_route::backing::seal_payload_bytes(&enlarged),
            Some(cloned + delta)
        );
        enlarged
            .planned_mask
            .plan
            .fragments
            .push(enlarged.planned_mask.plan.fragments[0].clone());
        assert_eq!(
            native_mask_route::backing::seal_payload_bytes(&enlarged),
            None
        );
    }
    let input = seal
        .planned_mask
        .plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .find(|placement| placement.gear_id == seal.planned_mask.mask.interaction_output.gear_id)
        .unwrap();
    assert_eq!(
        input.execution_profile_id.as_str(),
        "conduitos/bounded-interaction@1"
    );
    let current = owner.local_face_snapshot().unwrap();
    seal.validate_current_with_interaction(
        &owner.session,
        &current,
        owner.host.advertisement(),
        &ad,
        &face,
        &show_line,
        &interaction,
    )
    .unwrap();
    let shown = show(&mut owner, &seal, 1);
    #[cfg(feature = "native-display")]
    assert!(native_mask_route::backing::show_payload_bytes(&shown).unwrap() > 0);
    let mut original = seal.clone();
    original.face_basis.sign_ids.reserve(19);
    // Non-IMAGE fixture must refuse rather than manufacture a new memory owner.
    assert!(
        native_mask_route::backing::prepare_seal_clone(owner.host.current_mut(), &original)
            .is_err()
    );

    owner
        .acknowledge_member_carrier_show(&challenge.session_id, &request(&credential), &shown)
        .unwrap();
    owner
        .retire_current_member_carrier(&challenge.session_id)
        .unwrap();
    assert!(owner
        .validate_native_mask_show(&request(&credential), &shown)
        .is_err());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn authenticated_two_line_viewer_cannot_mint_human_input_from_available_actions() {
    for interactive in [true, false] {
        let (mut owner, root) = owner_fixture();
        let (credential, key, ad) = native_member_profile(&mut owner, &root, interactive);
        let (challenge, _) = begin(&mut owner, &credential, &ad);
        owner
            .complete_current_member_carrier(&challenge, signature(&challenge, &key))
            .unwrap();
        let session = &challenge.session_id;
        owner.renew_current_member_presence(session, 1).unwrap();
        let (face_line, returned) = lines(&owner, &credential, &ad, session);
        let seal = owner
            .select_member_carrier_mask_route(session, &face_line, &returned)
            .unwrap();
        assert!(seal.interaction_line.is_none());
        let shown = show(&mut owner, &seal, 1);
        let req = request(&credential);
        owner
            .acknowledge_member_carrier_show(session, &req, &shown)
            .unwrap();
        let face = owner.local_face_snapshot().unwrap();
        let action = face
            .actions
            .iter()
            .find(|action| {
                action.arguments.is_empty()
                    && action.availability == conduit_face::FaceActionAvailability::Available
            })
            .unwrap();
        let forged = conduit_face::FaceInteraction::new(
            &face,
            &shown,
            &action.identity,
            &action.target,
            vec![],
            1,
        )
        .unwrap();
        assert_eq!(
            owner
                .validate_member_carrier_interaction(session, &req, &shown, &forged)
                .unwrap_err(),
            if interactive {
                "native-mask-has-no-original-interaction-Line"
            } else {
                "native-mask-has-no-interaction-authority"
            }
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[path = "presentation/owner_navigation.rs"]
mod owner_navigation;
