use super::*;
use conduit_body::{
    PortableSpawnAdmissionRequest, ResidentPlot, SpawnInvitationSecret,
    SPAWN_ADMISSION_REQUEST_SCHEMA,
};
use conduit_core::{BootId, CapabilityId, HostId, OfferGeneration};
use conduit_std_host::{StdHost, StdHostConfig};
use std::path::PathBuf;

const SOURCE: &str = "plot hello {\n show: presentation/text\n \"Hello.\" >> show\n}.";
// Ed25519 public key for the deterministic test secret [7; 32].
const BROWSER_KEY: [u8; 32] = [
    0xea, 0x4a, 0x6c, 0x63, 0xe2, 0x9c, 0x52, 0x0a, 0xbe, 0xf5, 0x50, 0x7b, 0x13, 0x2e, 0xc5, 0xf9,
    0x95, 0x47, 0x76, 0xae, 0xbe, 0xbe, 0x7b, 0x92, 0x42, 0x1e, 0xea, 0x69, 0x14, 0x46, 0xd2, 0x2c,
];

fn host(id: &str, boot: &str) -> StdHost {
    StdHost::new_with_config(StdHostConfig {
        host_id: HostId::from(id),
        boot_id: BootId::from(boot),
        offer_generation: OfferGeneration(1),
    })
}

fn setup() -> (Owner, PathBuf, ResidentPlot) {
    let root = std::env::temp_dir().join(crate::durable_host::fresh_identity(
        "owner-browser-window",
        "service-test",
    ));
    std::fs::create_dir_all(&root).unwrap();
    let installation = crate::durable_host::Installation {
        schema: crate::durable_host::INSTALL_SCHEMA.into(),
        host_id: "host/owner-test".into(),
        release_source_identity: "source/test".into(),
        release_bundle_sha256: crate::durable_host::digest(b"bundle/test"),
        product_executable: "fixture-unused".into(),
        body_state: None,
        joined_body_state: None,
    };
    crate::durable_host::write_json_atomic(&root.join("installation.json"), &installation).unwrap();
    let plot = crate::plot_source::parse(SOURCE)
        .unwrap()
        .expand_entry_for_authoring()
        .unwrap();
    let resident = ResidentPlot::new(
        plot.expanded.source_document_id.clone(),
        plot.expanded.checked_plot_id.clone(),
    );
    let mut owner = Owner::open(
        host("host/owner-test", "boot/owner/first"),
        resident.clone(),
        None,
        "Same Body",
    )
    .unwrap();
    owner.persist(&root).unwrap();
    (owner, root, resident)
}

fn advertise(owner: &mut Owner, window_id: &str) -> conduit_body::AdmissionChallenge {
    let advertisement = host("host/browser-test", "boot/browser/first")
        .advertisement()
        .clone();
    let result = owner
        .browser_begin(
            window_id,
            &LinkBindingId::from("line/test/browser"),
            In::Advertise {
                protocol: PROTOCOL,
                advertisement,
                friendly_label: "Browser test".into(),
                verifying_key: BROWSER_KEY.to_vec(),
                freshness_sequence: 1,
            },
            512,
        )
        .unwrap();
    let Out::Challenge { challenge, .. } = result else {
        panic!("expected ambient challenge")
    };
    challenge
}

#[test]
fn browser_mask_planning_requires_the_reviewed_back_and_presentation_resource() {
    let (mut owner, root, _) = setup();
    let mut advertisement = host("host/browser-test", "boot/browser/first")
        .advertisement()
        .clone();
    let mask = conduit_browser_mask_offer::offer();
    advertisement.capabilities.push(mask.clone());
    advertisement
        .resources
        .retain(|resource| resource.class_id.as_str() != conduit_core::PRESENTATION_RESOURCE_CLASS);
    advertisement.resources.push(conduit_core::resource_offer(
        "browser/presentation",
        conduit_core::PRESENTATION_RESOURCE_CLASS,
        1,
    ));
    advertisement
        .capabilities
        .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    advertisement
        .resources
        .sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
    let planned = conduit_browser_mask_offer::planned_mask(
        &advertisement,
        conduit_browser_mask_offer::MASK_SOURCE,
        "browser-graphical",
    )
    .unwrap();
    let browser_offer = advertisement.clone();
    let authorized = owner
        .browser_authorize_window("host/browser-test", Some(BROWSER_KEY), 10_000)
        .unwrap();
    let Out::Challenge { challenge, .. } = owner
        .browser_begin(
            &authorized.window_id,
            &LinkBindingId::from("line/test/browser-mask"),
            In::Advertise {
                protocol: PROTOCOL,
                advertisement,
                friendly_label: "Browser Mask test".into(),
                verifying_key: BROWSER_KEY.to_vec(),
                freshness_sequence: 1,
            },
            512,
        )
        .unwrap()
    else {
        panic!("expected browser admission challenge")
    };
    let secret = SpawnInvitationSecret::from_csprng_bytes([7; 32]).unwrap();
    let snapshot = owner
        .browser_complete(
            &root,
            &authorized.window_id,
            In::AmbientProof {
                protocol: PROTOCOL,
                admission_id: challenge.admission_id.clone(),
                body_id: challenge.body_id.clone(),
                host_id: challenge.host_id.clone(),
                boot_id: challenge.boot_id.clone(),
                nonce: challenge.nonce.to_vec(),
                signature: secret.sign(&challenge.signing_transcript()).to_vec(),
            },
        )
        .unwrap();
    let request = OfferDisclosureRequest {
        stage: OfferDisclosureStage::Planning,
        capability_ids: vec![mask.capability_id.clone()],
        resource_pool_ids: vec![conduit_core::ResourcePoolId::from("browser/presentation")],
    };
    owner
        .browser_planning_offer(&authorized.window_id, &snapshot.credential, &request)
        .unwrap();
    assert_eq!(
        owner.browser_mask_route(
            &authorized.window_id,
            &snapshot.credential,
            &LinkBindingId::from("line/test/other-carrier"),
        ),
        Err("browser-route-carrier-mismatch".into())
    );
    let selected = owner
        .browser_mask_route(
            &authorized.window_id,
            &snapshot.credential,
            &LinkBindingId::from("line/test/browser-mask"),
        )
        .unwrap();
    assert_eq!(selected.mask_host.host_id, snapshot.credential.host_id);
    let issued_face = owner.local_face_snapshot().unwrap();
    let issued_response = conduit_presentation::OwnerFaceSnapshotResponse::Snapshot {
        schema: conduit_presentation::OWNER_FACE_RESPONSE_SCHEMA.into(),
        presentation: Box::new(issued_face),
        interactions_admitted: true,
        route: Some(Box::new(selected.clone())),
    };
    assert!(
        serde_json::to_vec(&issued_response).unwrap().len()
            <= conduit_presentation::MAX_OWNER_FACE_RESPONSE_BYTES
    );
    let WindowState::Active { route, .. } = &owner.pending_browser.as_ref().unwrap().state else {
        panic!("expected active browser route")
    };
    assert_eq!(route.as_deref(), Some(&selected));
    // These explicit remote Line facts exercise the route contract only. A
    // live owner route must derive them from the retained browser carrier.
    let owner_offer = owner.host.advertisement().clone();
    let limits = conduit_core::LinkLimits {
        maximum_in_flight_items: 1,
        maximum_payload_bytes: 64 * 1024,
        maximum_buffered_bytes: 128 * 1024,
        maximum_frame_bytes: 64 * 1024,
    };
    let remote_line = |direction: &str,
                       source: &conduit_core::HostAdvertisement,
                       sink: &conduit_core::HostAdvertisement| {
        let mut line = conduit_core::process_owned_line_offer_with_limits(
            &format!("line/test/browser-mask/{direction}"),
            &format!("binding/test/browser-mask/{direction}"),
            conduit_core::BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
            "base-instance/test/browser-mask",
            source,
            sink,
            limits,
        );
        line.binding.credential = conduit_core::LinkCredentialReference::Opaque(
            conduit_core::CredentialReferenceId::from("credential/test/browser-mask"),
        );
        line.binding.authority = conduit_core::LinkAuthorityReference::Grant(
            conduit_core::AuthorityGrantId::from("grant/test/browser-mask"),
        );
        line.contract.scope = conduit_core::LineScope::RoutedNetwork;
        line.contract.security = conduit_core::LineSecurity::PlaintextNetwork;
        line
    };
    let face_line = remote_line("face", &owner_offer, &browser_offer);
    let return_line = remote_line("return", &browser_offer, &owner_offer);
    let face = owner.local_face_snapshot().unwrap();
    let placement = selected.planned_mask.show_placement();
    let play = conduit_core::bind_active_play(
        &selected.planned_mask.plan.plan_id,
        &placement.host_id,
        &placement.boot_id,
        1,
    );
    let available_show = conduit_presentation::MaskShow::prepared(
        &selected.planned_mask,
        &face,
        play,
        face.subjects[0].identity.clone(),
        "browser/document".into(),
        conduit_core::SignId::from("sign/test/browser-mask-prepared"),
    )
    .unwrap()
    .transition(
        conduit_presentation::ManifestationLifecycle::Available,
        conduit_core::SignId::from("sign/test/browser-mask-available"),
    )
    .unwrap();
    let face_request = conduit_presentation::OwnerFaceSnapshotRequest {
        schema: conduit_presentation::OWNER_FACE_REQUEST_SCHEMA.into(),
        credential_id: snapshot.credential.credential_id.as_str().into(),
        body_id: snapshot.credential.body_id.clone(),
        part_id: snapshot.credential.part_id.clone(),
        host_id: snapshot.credential.host_id.clone(),
        boot_id: snapshot.credential.boot_id.clone(),
        last_seen_revision: None,
        last_seen_identity: None,
    };
    owner
        .validate_browser_mask_show(
            &authorized.window_id,
            &LinkBindingId::from("line/test/browser-mask"),
            &face_request,
            &available_show,
        )
        .unwrap();
    assert!(owner
        .validate_browser_mask_show(
            &authorized.window_id,
            &LinkBindingId::from("line/test/other-carrier"),
            &face_request,
            &available_show,
        )
        .is_err());
    let seal = conduit_presentation::RemoteOwnerMaskRouteSeal::seal_lulled(
        &owner.session,
        &face,
        &owner_offer,
        &browser_offer,
        &planned,
        &face_line,
        &return_line,
    )
    .unwrap();
    let route_bytes = serde_json::to_vec(&seal).unwrap().len();
    let face_bytes = serde_json::to_vec(&face).unwrap().len();
    assert!(route_bytes + face_bytes + 1024 <= conduit_presentation::MAX_OWNER_FACE_RESPONSE_BYTES);
    let mut process_owned = face_line.clone();
    process_owned.binding.authority = conduit_core::LinkAuthorityReference::ProcessOwned;
    assert_eq!(
        conduit_presentation::RemoteOwnerMaskRouteSeal::seal_lulled(
            &owner.session,
            &face,
            &owner_offer,
            &browser_offer,
            &planned,
            &process_owned,
            &return_line,
        ),
        Err(conduit_presentation::RemoteOwnerMaskRouteError::MissingOrInvalidLine)
    );
    let mut missing_credential = return_line.clone();
    missing_credential.binding.credential = conduit_core::LinkCredentialReference::None;
    assert_eq!(
        conduit_presentation::RemoteOwnerMaskRouteSeal::seal_lulled(
            &owner.session,
            &face,
            &owner_offer,
            &browser_offer,
            &planned,
            &face_line,
            &missing_credential,
        ),
        Err(conduit_presentation::RemoteOwnerMaskRouteError::MissingOrInvalidLine)
    );
    let mut wrong_line = face_line.clone();
    wrong_line.binding.sink.boot_id = conduit_core::BootId::from("boot/browser/wrong");
    assert_eq!(
        conduit_presentation::RemoteOwnerMaskRouteSeal::seal_lulled(
            &owner.session,
            &face,
            &owner_offer,
            &browser_offer,
            &planned,
            &wrong_line,
            &return_line,
        ),
        Err(conduit_presentation::RemoteOwnerMaskRouteError::MissingOrInvalidLine)
    );
    let mut missing_back = browser_offer.clone();
    missing_back
        .capabilities
        .retain(|offer| offer.capability_id != mask.capability_id);
    assert!(matches!(
        conduit_presentation::RemoteOwnerMaskRouteSeal::seal_lulled(
            &owner.session,
            &face,
            &owner_offer,
            &missing_back,
            &planned,
            &face_line,
            &return_line,
        ),
        Err(conduit_presentation::RemoteOwnerMaskRouteError::InvalidMaskPlan(_))
    ));
    assert_eq!(
        seal.validate_return_payload(64 * 1024 + 1),
        Err(conduit_presentation::RemoteOwnerMaskRouteError::ReturnExceedsLine)
    );
    seal.validate_current(
        &owner.session,
        &face,
        &owner_offer,
        &browser_offer,
        &face_line,
        &return_line,
    )
    .unwrap();
    let mut unavailable = return_line.clone();
    unavailable.availability.availability = conduit_core::LineAvailability::Unavailable;
    assert_eq!(
        seal.validate_current(
            &owner.session,
            &face,
            &owner_offer,
            &browser_offer,
            &face_line,
            &unavailable,
        ),
        Err(conduit_presentation::RemoteOwnerMaskRouteError::LineUnavailable)
    );
    let mut altered_browser = browser_offer.clone();
    altered_browser.offer_generation.0 += 1;
    assert_eq!(
        seal.validate_current(
            &owner.session,
            &face,
            &owner_offer,
            &altered_browser,
            &face_line,
            &return_line,
        ),
        Err(conduit_presentation::RemoteOwnerMaskRouteError::StaleHost)
    );
    let WindowState::Active { observation, .. } =
        &mut owner.pending_browser.as_mut().unwrap().state
    else {
        panic!("expected active browser offer")
    };
    let offered = observation
        .advertisement
        .capabilities
        .iter_mut()
        .find(|offer| offer.capability_id == mask.capability_id)
        .unwrap();
    offered.implementation.artifact_id = conduit_core::ArtifactId::from("artifact/forged");
    assert_eq!(
        owner.browser_planning_offer(&authorized.window_id, &snapshot.credential, &request),
        Err("browser-mask-offer-mismatch".into())
    );
    assert!(owner
        .validate_browser_mask_show(
            &authorized.window_id,
            &LinkBindingId::from("line/test/browser-mask"),
            &face_request,
            &available_show,
        )
        .is_err());
    let WindowState::Active { observation, .. } =
        &mut owner.pending_browser.as_mut().unwrap().state
    else {
        unreachable!()
    };
    *observation
        .advertisement
        .capabilities
        .iter_mut()
        .find(|offer| offer.capability_id == mask.capability_id)
        .unwrap() = mask.clone();
    observation
        .advertisement
        .resources
        .retain(|resource| resource.pool_id.as_str() != "browser/presentation");
    assert_eq!(
        owner.browser_planning_offer(&authorized.window_id, &snapshot.credential, &request),
        Err("browser-mask-offer-mismatch".into())
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn current_browser_window_discloses_only_requested_planning_offer_detail() {
    let (mut owner, root, _) = setup();
    let browser = host("host/browser-test", "boot/browser/first")
        .advertisement()
        .clone();
    let capability = browser.capabilities[0].capability_id.clone();
    let authorized = owner
        .browser_authorize_window("host/browser-test", Some(BROWSER_KEY), 10_000)
        .unwrap();
    let challenge = advertise(&mut owner, &authorized.window_id);
    let secret = SpawnInvitationSecret::from_csprng_bytes([7; 32]).unwrap();
    let snapshot = owner
        .browser_complete(
            &root,
            &authorized.window_id,
            In::AmbientProof {
                protocol: PROTOCOL,
                admission_id: challenge.admission_id.clone(),
                body_id: challenge.body_id.clone(),
                host_id: challenge.host_id.clone(),
                boot_id: challenge.boot_id.clone(),
                nonce: challenge.nonce.to_vec(),
                signature: secret.sign(&challenge.signing_transcript()).to_vec(),
            },
        )
        .unwrap();
    assert!(snapshot.offer.capabilities.is_empty());
    let request = OfferDisclosureRequest {
        stage: OfferDisclosureStage::Planning,
        capability_ids: vec![capability.clone()],
        resource_pool_ids: vec![],
    };
    let detailed = owner
        .browser_planning_offer(&authorized.window_id, &snapshot.credential, &request)
        .unwrap();
    assert_eq!(detailed.stage, OfferDisclosureStage::Planning);
    assert_eq!(detailed.proof_class, RemoteProofClass::SelfReported);
    assert_eq!(detailed.host_id, snapshot.credential.host_id);
    assert_eq!(detailed.boot_id, snapshot.credential.boot_id);
    assert_eq!(detailed.offer_generation, browser.offer_generation);
    assert_eq!(detailed.capabilities.len(), 1);
    assert_eq!(detailed.capabilities[0].capability_id, capability);
    assert!(detailed.capability_summary.is_empty());
    let mut unknown = request.clone();
    unknown.capability_ids = vec![CapabilityId::from("capability/unknown")];
    assert_eq!(
        owner.browser_planning_offer(&authorized.window_id, &snapshot.credential, &unknown),
        Err("unknown-capability".into())
    );
    let mut premature = request.clone();
    premature.stage = OfferDisclosureStage::AdmittedMembership;
    assert_eq!(
        owner.browser_planning_offer(&authorized.window_id, &snapshot.credential, &premature),
        Err("invalid-offer-request".into())
    );
    let mut altered = snapshot.credential.clone();
    altered.issued_at_millis += 1;
    assert_eq!(
        owner.browser_planning_offer(&authorized.window_id, &altered, &request),
        Err("credential-mismatch".into())
    );
    owner
        .browser_cancel_window(&root, &authorized.window_id)
        .unwrap();
    assert_eq!(
        owner.browser_planning_offer(&authorized.window_id, &snapshot.credential, &request),
        Err("window-not-active".into())
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn signed_browser_proof_uses_latest_owner_after_interleaved_native_admission() {
    let (mut owner, root, _) = setup();
    let authorized = owner
        .browser_authorize_window("host/browser-test", Some(BROWSER_KEY), 10_000)
        .unwrap();
    let challenge = advertise(&mut owner, &authorized.window_id);
    assert!(
        owner.admissions.is_none(),
        "unfinished browser challenge must stay ephemeral"
    );

    let invitation = owner.issue_invitation(&root, 60, None).unwrap();
    let native = host("host/native-test", "boot/native/first")
        .advertisement()
        .clone();
    let secret = SpawnInvitationSecret::from_csprng_bytes(invitation.secret).unwrap();
    let request = PortableSpawnAdmissionRequest {
        schema: SPAWN_ADMISSION_REQUEST_SCHEMA.into(),
        invitation_id: invitation.claim.invitation_id.clone(),
        body_id: invitation.claim.body_id.clone(),
        host_advertisement: native.clone(),
        nonce: invitation.claim.nonce,
        signature: secret
            .sign(&invitation.claim.signing_transcript(
                &native.host_id,
                &native.boot_id,
                native.offer_generation,
            ))
            .to_vec(),
        membership_admitted: false,
        plan_created: false,
        play_created: false,
    };
    let native_receipt = owner
        .admit_invited(&root, request, "host/native-test")
        .unwrap();
    let browser_secret = SpawnInvitationSecret::from_csprng_bytes([7; 32]).unwrap();
    let snapshot = owner
        .browser_complete(
            &root,
            &authorized.window_id,
            In::AmbientProof {
                protocol: PROTOCOL,
                admission_id: challenge.admission_id.clone(),
                body_id: challenge.body_id.clone(),
                host_id: challenge.host_id.clone(),
                boot_id: challenge.boot_id.clone(),
                nonce: challenge.nonce.to_vec(),
                signature: browser_secret
                    .sign(&challenge.signing_transcript())
                    .to_vec(),
            },
        )
        .unwrap();
    assert_eq!(snapshot.biography.membership.parts.len(), 3);
    assert_eq!(
        snapshot.credential.host_id,
        HostId::from("host/browser-test")
    );
    assert!(snapshot
        .biography
        .membership
        .parts
        .iter()
        .any(|part| part.part_id == native_receipt.credential.part_id
            && part
                .current
                .as_ref()
                .is_some_and(|current| current.host_id == native.host_id
                    && current.boot_id == native.boot_id)));
    // Simulate a worker terminating after proof but before ordinary leave.
    owner
        .browser_cancel_window(&root, &authorized.window_id)
        .unwrap();
    assert!(owner
        .session
        .evidence()
        .membership
        .parts
        .iter()
        .any(|part| part.part_id == native_receipt.credential.part_id && part.current.is_some()));
    assert!(owner
        .session
        .evidence()
        .membership
        .parts
        .iter()
        .any(|part| part.part_id == snapshot.credential.part_id && part.current.is_none()));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn aborted_challenge_is_not_retained_across_service_restart() {
    let (mut owner, root, resident) = setup();
    let authorized = owner
        .browser_authorize_window("host/browser-test", Some(BROWSER_KEY), 10_000)
        .unwrap();
    let first = advertise(&mut owner, &authorized.window_id);
    owner.browser_abort(&authorized.window_id).unwrap();
    owner
        .browser_cancel_window(&root, &authorized.window_id)
        .unwrap();
    let mut resumed = Owner::open(
        host("host/owner-test", "boot/owner/second"),
        resident,
        Some(owner.session.evidence().clone()),
        "ignored",
    )
    .unwrap();
    resumed.restore_execution(&root).unwrap();
    let second = resumed
        .browser_authorize_window("host/browser-test", Some(BROWSER_KEY), 10_000)
        .unwrap();
    let next = advertise(&mut resumed, &second.window_id);
    assert_ne!(first.nonce, next.nonce);
    assert!(resumed
        .admissions
        .as_ref()
        .is_none_or(|manager| manager.receipts.is_empty()));
    std::fs::remove_dir_all(root).unwrap();
}
