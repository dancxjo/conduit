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
    owner
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
    std::fs::remove_dir_all(root).unwrap();
}
