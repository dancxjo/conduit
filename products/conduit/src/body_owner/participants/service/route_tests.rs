use super::*;

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
    assert_eq!(
        owner.validate_browser_mask_show(
            &authorized.window_id,
            &LinkBindingId::from("line/test/browser-mask"),
            &face_request,
            &available_show,
        ),
        Err("browser-mask-show-not-acknowledged".into())
    );
    owner
        .acknowledge_browser_mask_show(
            &authorized.window_id,
            &LinkBindingId::from("line/test/browser-mask"),
            &face_request,
            &available_show,
        )
        .unwrap();
    owner
        .validate_browser_mask_show(
            &authorized.window_id,
            &LinkBindingId::from("line/test/browser-mask"),
            &face_request,
            &available_show,
        )
        .unwrap();
    assert_eq!(
        owner
            .browser_mask_route(
                &authorized.window_id,
                &snapshot.credential,
                &LinkBindingId::from("line/test/browser-mask"),
            )
            .unwrap(),
        selected
    );
    assert_eq!(
        owner.validate_browser_mask_show(
            &authorized.window_id,
            &LinkBindingId::from("line/test/browser-mask"),
            &face_request,
            &available_show,
        ),
        Err("browser-mask-show-not-acknowledged".into())
    );
    owner
        .acknowledge_browser_mask_show(
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
