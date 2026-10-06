use super::*;

#[test]
fn browser_first_native_join_reseals_both_routes_on_one_current_face() {
    let (mut owner, root, _) = setup();
    let mut browser_offer = host("host/browser-first", "boot/browser-first")
        .advertisement()
        .clone();
    browser_offer
        .capabilities
        .push(conduit_browser_mask_offer::offer());
    browser_offer
        .resources
        .retain(|resource| resource.class_id.as_str() != conduit_core::PRESENTATION_RESOURCE_CLASS);
    browser_offer.resources.push(conduit_core::resource_offer(
        "browser/first/presentation",
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
        .browser_authorize_window("host/browser-first", Some(BROWSER_KEY), 10_000)
        .unwrap();
    let binding = LinkBindingId::from("line/test/browser-first");
    let Out::Challenge { challenge, .. } = owner
        .browser_begin(
            &browser.window_id,
            &binding,
            In::Advertise {
                protocol: PROTOCOL,
                advertisement: browser_offer,
                friendly_label: "Browser first".into(),
                verifying_key: BROWSER_KEY.to_vec(),
                freshness_sequence: 1,
            },
            512,
        )
        .unwrap()
    else {
        panic!("expected browser challenge")
    };
    let browser_secret = SpawnInvitationSecret::from_csprng_bytes([7; 32]).unwrap();
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
                signature: browser_secret
                    .sign(&challenge.signing_transcript())
                    .to_vec(),
            },
        )
        .unwrap();
    let browser_lines = super::route_tests::fixture_carrier_evidence(&browser_snapshot);
    let old_browser = owner
        .browser_mask_route(
            &browser.window_id,
            &browser_snapshot.credential,
            &binding,
            Some(&browser_lines),
        )
        .unwrap();
    let old_face = owner.local_face_snapshot().unwrap();
    let old_plan = owner
        .presentation_wardrobe
        .as_ref()
        .unwrap()
        .plan()
        .plan_id
        .clone();
    let placement = old_browser.planned_mask.show_placement();
    let play = conduit_core::bind_active_play(
        &old_browser.planned_mask.plan.plan_id,
        &placement.host_id,
        &placement.boot_id,
        1,
    );
    let old_show = conduit_presentation::MaskShow::prepared(
        &old_browser.planned_mask,
        &old_face,
        play,
        old_face.subjects[0].identity.clone(),
        "browser/test/old".into(),
        conduit_core::SignId::from("sign/test/browser-first-prepared"),
    )
    .unwrap()
    .transition(
        conduit_presentation::ManifestationLifecycle::Available,
        conduit_core::SignId::from("sign/test/browser-first-available"),
    )
    .unwrap();
    owner
        .acknowledge_selected_browser_show(&old_browser, &old_show)
        .unwrap();
    let WindowState::Active {
        acknowledged_show, ..
    } = &mut owner.pending_browser.as_mut().unwrap().state
    else {
        panic!("expected active browser")
    };
    *acknowledged_show = Some(Box::new(old_show));

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
    let native_offer = conduit_conduitos_mask_offer::native_host_advertisement(
        &HostId::from("host/native-after-browser"),
        &BootId::from("boot/native-after-browser"),
        1,
        &provider,
    );
    let invitation = owner.issue_invitation(&root, 60, None).unwrap();
    let secret = SpawnInvitationSecret::from_csprng_bytes(invitation.secret).unwrap();
    let receipt = owner
        .admit_invited(
            &root,
            PortableSpawnAdmissionRequest {
                schema: SPAWN_ADMISSION_REQUEST_SCHEMA.into(),
                invitation_id: invitation.claim.invitation_id.clone(),
                body_id: invitation.claim.body_id.clone(),
                host_advertisement: native_offer.clone(),
                nonce: invitation.claim.nonce,
                signature: secret
                    .sign(&invitation.claim.signing_transcript(
                        &native_offer.host_id,
                        &native_offer.boot_id,
                        native_offer.offer_generation,
                    ))
                    .to_vec(),
                membership_admitted: false,
                plan_created: false,
                play_created: false,
            },
            "host/native-after-browser",
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
            &format!("line/test/native-after-browser/{direction}"),
            &format!("binding/test/native-after-browser/{direction}"),
            conduit_core::BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
            "base-instance/test/native-after-browser",
            source,
            sink,
            limits,
        );
        line.binding.credential = conduit_core::LinkCredentialReference::Opaque(
            conduit_core::CredentialReferenceId::from("credential/test/native-after-browser"),
        );
        line.binding.authority = conduit_core::LinkAuthorityReference::Grant(
            conduit_core::AuthorityGrantId::from("grant/test/native-after-browser"),
        );
        line.contract.scope = conduit_core::LineScope::RoutedNetwork;
        line.contract.security = conduit_core::LineSecurity::AuthenticatedEncrypted;
        line
    };
    let face_line = line("face", &owner_offer, &native_offer);
    let return_line = line("return", &native_offer, &owner_offer);
    let current_face = owner.local_face_snapshot().unwrap();
    assert_ne!(old_face.revision, current_face.revision);
    let native = owner
        .seal_native_mask_route(
            &receipt,
            &face_line,
            &return_line,
            crate::durable_host::current_time_millis().unwrap() + 60_000,
        )
        .unwrap();
    let window = owner.pending_browser.as_ref().unwrap();
    let (browser_route, ..) = window.current_mask_route().unwrap();
    let browser_route = browser_route.clone();
    assert_eq!(browser_route.face_id, current_face.identity);
    assert_eq!(browser_route.face_revision, current_face.revision);
    assert_eq!(native.face_id, current_face.identity);
    assert_eq!(native.face_revision, current_face.revision);
    let WindowState::Active {
        acknowledged_show, ..
    } = &window.state
    else {
        panic!("expected active browser")
    };
    assert!(acknowledged_show.is_none());
    let plan = owner.presentation_wardrobe.as_ref().unwrap().plan();
    assert_ne!(plan.plan_id, old_plan);
    assert_eq!(plan.routes.len(), 2);
    assert_eq!(plan.face_revision, current_face.revision);

    // A checked workset replacement on this same lulled Body advances its
    // workload revision. The old browser seal is stale for that reason, not
    // merely for a changed Face revision. Its still-current carrier may be
    // resealed, while the old Show and native witness must be withdrawn.
    let body_id = owner.session.evidence().body_id.clone();
    let old_revision = owner.session.evidence().body.workload_revision;
    let old_resident = owner.resident.clone().unwrap();
    let next = crate::plot_source::parse(
        "plot revised {\n show: presentation/text\n \"Revised.\" >> show\n}.",
    )
    .unwrap()
    .expand_entry_for_authoring()
    .unwrap();
    let next_resident = ResidentPlot::new(
        next.expanded.source_document_id.clone(),
        next.expanded.checked_plot_id.clone(),
    );
    let owner_host = owner.host.advertisement();
    owner
        .session
        .remove_plot(
            old_revision,
            &old_resident,
            &owner_host.host_id,
            &owner_host.boot_id,
        )
        .unwrap();
    owner
        .session
        .admit_plot(
            old_revision + 1,
            next_resident.clone(),
            &owner_host.host_id,
            &owner_host.boot_id,
        )
        .unwrap();
    owner.resident = Some(next_resident);
    owner.resident_name = Some(next.expanded.name);
    let changed_face = owner.local_face_snapshot().unwrap();
    assert_eq!(owner.session.evidence().body_id, body_id);
    assert_ne!(
        owner.session.evidence().body.workload_revision,
        old_revision
    );
    assert!(matches!(
        browser_route.validate_current_with_interaction(
            &owner.session,
            &changed_face,
            owner.host.advertisement(),
            owner
                .pending_browser
                .as_ref()
                .unwrap()
                .current_mask_route()
                .unwrap()
                .1,
            &browser_lines.face,
            &browser_lines.returned,
            &browser_lines.interaction,
        ),
        Err(conduit_presentation::RemoteOwnerMaskRouteError::StaleBody)
    ));
    owner.refresh_browser_mask_route_for_current_face().unwrap();
    let window = owner.pending_browser.as_ref().unwrap();
    let (resealed_browser, ..) = window.current_mask_route().unwrap();
    assert_eq!(resealed_browser.body_id, body_id);
    assert_eq!(
        resealed_browser.workload_revision,
        owner.session.evidence().body.workload_revision
    );
    assert_eq!(resealed_browser.face_id, changed_face.identity);
    let WindowState::Active {
        acknowledged_show, ..
    } = &window.state
    else {
        panic!("expected active browser")
    };
    assert!(acknowledged_show.is_none());
    let resealed_native = owner
        .seal_native_mask_route(
            &receipt,
            &face_line,
            &return_line,
            crate::durable_host::current_time_millis().unwrap() + 60_000,
        )
        .unwrap();
    assert_eq!(resealed_native.body_id, body_id);
    assert_eq!(resealed_native.face_id, changed_face.identity);
    assert_eq!(
        owner
            .presentation_wardrobe
            .as_ref()
            .unwrap()
            .plan()
            .routes
            .len(),
        2
    );
    std::fs::remove_dir_all(root).unwrap();
}
