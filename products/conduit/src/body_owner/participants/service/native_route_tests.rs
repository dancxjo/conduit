use super::*;

#[test]
fn remote_native_mask_accepts_only_the_offered_scoped_present_binding() {
    let (mut owner, root, _) = setup();
    let host_id = HostId::from("host/native-test");
    let boot_id = BootId::from("boot/native/first");
    let provider = conduit_core::BaseProviderEntry {
        base_id: "native/test/framebuffer".into(),
        provider_instance_id: "native/test/framebuffer/1".into(),
        provider_generation: 1,
        implementation_id: "conduitos/framebuffer@1".into(),
        mechanism_family: "conduitos.base/framebuffer@1".into(),
        enforcement_class: conduit_core::BaseEnforcementClass::ConduitOsKernelEnforced,
        lifecycle: conduit_core::BaseLifecycle::Ready,
        capabilities: Vec::new(),
        resources: vec![conduit_core::resource_offer(
            "native/test/framebuffer/surface",
            conduit_presentation::SHOW_RESOURCE_CLASS,
            1,
        )],
    };
    let native_offer =
        conduit_conduitos_mask_offer::native_host_advertisement(&host_id, &boot_id, 1, &provider);
    let planned = conduit_conduitos_mask_offer::prepare_stage_from_offer(
        conduit_conduitos_mask_offer::Adapter::Native,
        &native_offer,
    )
    .unwrap()
    .planned_mask;
    assert!(planned.plan.fragments[0]
        .placements
        .iter()
        .any(|placement| !placement.authority.is_empty()));

    let authorized = owner
        .browser_authorize_window("host/native-test", Some(BROWSER_KEY), 10_000)
        .unwrap();
    let Out::Challenge { challenge, .. } = owner
        .browser_begin(
            &authorized.window_id,
            &LinkBindingId::from("line/test/native-mask"),
            In::Advertise {
                protocol: PROTOCOL,
                advertisement: native_offer.clone(),
                friendly_label: "Native Mask test".into(),
                verifying_key: BROWSER_KEY.to_vec(),
                freshness_sequence: 1,
            },
            512,
        )
        .unwrap()
    else {
        panic!("expected native admission challenge")
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
    let owner_offer = owner.host.advertisement().clone();
    let limits = conduit_core::LinkLimits {
        maximum_in_flight_items: 1,
        maximum_payload_bytes: 64 * 1024,
        maximum_buffered_bytes: 128 * 1024,
        maximum_frame_bytes: 8 * 1024,
    };
    let line = |direction: &str,
                source: &conduit_core::HostAdvertisement,
                sink: &conduit_core::HostAdvertisement| {
        let mut line = conduit_core::process_owned_line_offer_with_limits(
            &format!("line/test/native-mask/{direction}"),
            &format!("binding/test/native-mask/{direction}"),
            conduit_core::BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
            "base-instance/test/native-mask",
            source,
            sink,
            limits,
        );
        line.binding.credential = conduit_core::LinkCredentialReference::Opaque(
            conduit_core::CredentialReferenceId::from("credential/test/native-mask"),
        );
        line.binding.authority = conduit_core::LinkAuthorityReference::Grant(
            conduit_core::AuthorityGrantId::from("grant/test/native-mask"),
        );
        line.contract.scope = conduit_core::LineScope::RoutedNetwork;
        line.contract.security = conduit_core::LineSecurity::AuthenticatedEncrypted;
        line
    };
    let face_line = line("face", &owner_offer, &native_offer);
    let return_line = line("return", &native_offer, &owner_offer);
    let face = owner.local_face_snapshot().unwrap();
    let seal = conduit_presentation::RemoteOwnerMaskRouteSeal::seal_current(
        &owner.session,
        &face,
        &owner_offer,
        &native_offer,
        &planned,
        &face_line,
        &return_line,
    )
    .unwrap();
    seal.validate_mask_host_offer(&native_offer).unwrap();
    seal.validate_current(
        &owner.session,
        &face,
        &owner_offer,
        &native_offer,
        &face_line,
        &return_line,
    )
    .unwrap();

    let mut wrong_offer = native_offer.clone();
    wrong_offer
        .capabilities
        .iter_mut()
        .find(|offer| offer.capability_id.as_str() == "renderer-conduitos")
        .unwrap()
        .authority_requirements
        .clear();
    assert!(seal.validate_mask_host_offer(&wrong_offer).is_err());

    let receipt = conduit_body::PortableAdmissionReceipt {
        schema: conduit_body::SPAWN_ADMISSION_RECEIPT_SCHEMA.into(),
        credential: snapshot.credential.clone(),
        host_advertisement: native_offer.clone(),
        membership_admitted: true,
        current_offers_available: false,
        plan_created: false,
        play_created: false,
    };
    let expires = crate::durable_host::current_time_millis().unwrap() + 60_000;
    let selected = owner
        .seal_native_mask_route(&receipt, &face_line, &return_line, expires)
        .unwrap();
    assert_eq!(selected, seal);
    let plan_id = owner
        .presentation_wardrobe
        .as_ref()
        .unwrap()
        .plan()
        .plan_id
        .clone();
    let placement = selected.planned_mask.show_placement();
    let play = conduit_core::bind_active_play(
        &selected.planned_mask.plan.plan_id,
        &placement.host_id,
        &placement.boot_id,
        1,
    );
    let show = conduit_presentation::MaskShow::prepared(
        &selected.planned_mask,
        &face,
        play,
        face.subjects[0].identity.clone(),
        "native/test".into(),
        conduit_core::SignId::from("sign/test/native-prepared"),
    )
    .unwrap()
    .transition(
        conduit_presentation::ManifestationLifecycle::Available,
        conduit_core::SignId::from("sign/test/native-available"),
    )
    .unwrap();
    let request = conduit_presentation::OwnerFaceSnapshotRequest {
        schema: conduit_presentation::OWNER_FACE_REQUEST_SCHEMA.into(),
        credential_id: snapshot.credential.credential_id.as_str().into(),
        body_id: snapshot.credential.body_id.clone(),
        part_id: snapshot.credential.part_id.clone(),
        host_id: snapshot.credential.host_id.clone(),
        boot_id: snapshot.credential.boot_id.clone(),
        last_seen_revision: None,
        last_seen_identity: None,
    };
    let current = Owner::current_presentation_routes_with_native(
        owner.host.advertisement(),
        owner.pending_browser.as_ref(),
        None,
        owner.pending_native_mask.as_ref(),
        &owner.session,
        &face,
        expires - 1,
    );
    assert_eq!(current.len(), 1);
    let native_mask = selected.planned_mask.mask.plot_identity.clone();
    // The native participant's admission credential alone does not grant
    // wardrobe control. This direct owner call models a separately
    // authenticated owner instruction; the browser carrier is tested below.
    assert!(owner
        .browser_wardrobe_report(
            &authorized.window_id,
            &LinkBindingId::from("line/test/native-mask"),
            &request,
            None,
            0,
            None,
        )
        .is_err());
    let inspected = owner.owner_wardrobe_report(None, 0, None).unwrap();
    assert_eq!(inspected["owner_plan_id"], serde_json::json!(plan_id));
    assert_eq!(inspected["wardrobe_revision_decimal"], "0");
    assert_eq!(
        inspected["route_descriptions"][0]["mask_name"],
        "native-graphical"
    );
    assert!(owner
        .owner_wardrobe_report(
            Some(&conduit_core::PlanId::from("plan/stale")),
            0,
            Some(conduit_presentation::MaskWardrobeAction::Doff(
                native_mask.clone()
            )),
        )
        .is_err());
    owner
        .owner_wardrobe_report(
            Some(&plan_id),
            0,
            Some(conduit_presentation::MaskWardrobeAction::Doff(
                native_mask.clone(),
            )),
        )
        .unwrap();
    assert!(owner.acknowledge_native_mask_show(&request, &show).is_err());
    assert!(owner
        .owner_wardrobe_report(
            Some(&plan_id),
            0,
            Some(conduit_presentation::MaskWardrobeAction::Wear(
                native_mask.clone()
            )),
        )
        .is_err());
    owner
        .owner_wardrobe_report(
            Some(&plan_id),
            1,
            Some(conduit_presentation::MaskWardrobeAction::Wear(native_mask)),
        )
        .unwrap();
    owner.acknowledge_native_mask_show(&request, &show).unwrap();
    owner.validate_native_mask_show(&request, &show).unwrap();
    assert_eq!(
        owner.presentation_wardrobe.as_ref().unwrap().plan().plan_id,
        plan_id
    );
    let expired = Owner::current_presentation_routes_with_native(
        owner.host.advertisement(),
        owner.pending_browser.as_ref(),
        None,
        owner.pending_native_mask.as_ref(),
        &owner.session,
        &face,
        expires,
    );
    assert!(expired.is_empty());
    let loss = owner
        .presentation_wardrobe
        .as_mut()
        .unwrap()
        .admit_or_replace(&owner.session, &face, &expired)
        .unwrap();
    assert!(matches!(
        loss.show,
        conduit_presentation::MaskShowDisposition::NoCurrentShow { .. }
    ));
    assert_eq!(
        owner.presentation_wardrobe.as_ref().unwrap().plan().plan_id,
        plan_id
    );

    // The first admission above is only a deterministic stand-in for the
    // native guest Part. A separate browser carrier now joins with its own
    // Face/Show/interaction grants; native remains a current Body member.
    owner.pending_browser = None;
    let mut browser_offer = host("host/browser-control", "boot/browser/control")
        .advertisement()
        .clone();
    browser_offer
        .capabilities
        .push(conduit_browser_mask_offer::offer());
    browser_offer
        .resources
        .retain(|resource| resource.class_id.as_str() != conduit_core::PRESENTATION_RESOURCE_CLASS);
    browser_offer.resources.push(conduit_core::resource_offer(
        "browser/control/presentation",
        conduit_core::PRESENTATION_RESOURCE_CLASS,
        1,
    ));
    browser_offer
        .capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    browser_offer
        .resources
        .sort_by(|a, b| a.pool_id.cmp(&b.pool_id));
    let browser = owner
        .browser_authorize_window("host/browser-control", Some(BROWSER_KEY), 10_000)
        .unwrap();
    let browser_binding = LinkBindingId::from("line/test/browser-control");
    let Out::Challenge { challenge, .. } = owner
        .browser_begin(
            &browser.window_id,
            &browser_binding,
            In::Advertise {
                protocol: PROTOCOL,
                advertisement: browser_offer,
                friendly_label: "Browser control".into(),
                verifying_key: BROWSER_KEY.to_vec(),
                freshness_sequence: 2,
            },
            512,
        )
        .unwrap()
    else {
        panic!("expected browser control challenge")
    };
    let browser_snapshot = owner
        .browser_complete(
            &root,
            &browser.window_id,
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
    let browser_lines = super::route_tests::fixture_carrier_evidence(&browser_snapshot);
    let browser_route = owner
        .browser_mask_route(
            &browser.window_id,
            &browser_snapshot.credential,
            &browser_binding,
            Some(&browser_lines),
        )
        .unwrap();
    let native_route = owner
        .seal_native_mask_route(
            &receipt,
            &face_line,
            &return_line,
            crate::durable_host::current_time_millis().unwrap() + 60_000,
        )
        .unwrap();
    let browser_request = conduit_presentation::OwnerFaceSnapshotRequest {
        schema: conduit_presentation::OWNER_FACE_REQUEST_SCHEMA.into(),
        credential_id: browser_snapshot.credential.credential_id.as_str().into(),
        body_id: browser_snapshot.credential.body_id.clone(),
        part_id: browser_snapshot.credential.part_id.clone(),
        host_id: browser_snapshot.credential.host_id.clone(),
        boot_id: browser_snapshot.credential.boot_id.clone(),
        last_seen_revision: None,
        last_seen_identity: None,
    };
    let report = owner
        .browser_wardrobe_report(
            &browser.window_id,
            &browser_binding,
            &browser_request,
            None,
            0,
            None,
        )
        .unwrap();
    assert_eq!(report["admitted_routes"].as_array().unwrap().len(), 2);
    let joint_plan = owner
        .presentation_wardrobe
        .as_ref()
        .unwrap()
        .plan()
        .plan_id
        .clone();
    let browser_mask = browser_route.planned_mask.mask.plot_identity.clone();
    let native_mask = native_route.planned_mask.mask.plot_identity.clone();
    let revision = owner
        .presentation_wardrobe
        .as_ref()
        .unwrap()
        .control()
        .scoped_wardrobe
        .wardrobe
        .revision;
    let apply = |owner: &mut Owner, revision, action| {
        owner.browser_wardrobe_report(
            &browser.window_id,
            &browser_binding,
            &browser_request,
            Some(&joint_plan),
            revision,
            Some(action),
        )
    };
    apply(
        &mut owner,
        revision,
        conduit_presentation::MaskWardrobeAction::Wear(browser_mask.clone()),
    )
    .unwrap();
    apply(
        &mut owner,
        revision + 1,
        conduit_presentation::MaskWardrobeAction::Prefer(vec![browser_mask.clone()]),
    )
    .unwrap();
    apply(
        &mut owner,
        revision + 2,
        conduit_presentation::MaskWardrobeAction::Doff(native_mask.clone()),
    )
    .unwrap();
    let current_face = owner.local_face_snapshot().unwrap();
    let placement = native_route.planned_mask.show_placement();
    let play = conduit_core::bind_active_play(
        &native_route.planned_mask.plan.plan_id,
        &placement.host_id,
        &placement.boot_id,
        2,
    );
    let native_show = conduit_presentation::MaskShow::prepared(
        &native_route.planned_mask,
        &current_face,
        play,
        current_face.subjects[0].identity.clone(),
        "native/test/joint".into(),
        conduit_core::SignId::from("sign/test/native-joint-prepared"),
    )
    .unwrap()
    .transition(
        conduit_presentation::ManifestationLifecycle::Available,
        conduit_core::SignId::from("sign/test/native-joint-available"),
    )
    .unwrap();
    assert!(owner
        .acknowledge_native_mask_show(&request, &native_show)
        .is_err());
    apply(
        &mut owner,
        revision + 3,
        conduit_presentation::MaskWardrobeAction::Wear(native_mask),
    )
    .unwrap();
    apply(
        &mut owner,
        revision + 4,
        conduit_presentation::MaskWardrobeAction::Doff(browser_mask),
    )
    .unwrap();
    owner
        .acknowledge_native_mask_show(&request, &native_show)
        .unwrap();
    owner
        .validate_native_mask_show(&request, &native_show)
        .unwrap();
    assert_eq!(
        owner.presentation_wardrobe.as_ref().unwrap().plan().plan_id,
        joint_plan
    );
    std::fs::remove_dir_all(root).unwrap();
}
