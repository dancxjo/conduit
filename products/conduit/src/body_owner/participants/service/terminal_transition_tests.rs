use super::*;

#[test]
fn attached_terminal_replaces_prior_generation_browser_witness_without_removing_its_part() {
    let (mut owner, root, _) = setup();
    let mut browser = host("host/browser-test", "boot/browser/first")
        .advertisement()
        .clone();
    let mask = conduit_browser_mask_offer::offer();
    browser.capabilities.push(mask.clone());
    browser
        .resources
        .retain(|resource| resource.class_id.as_str() != conduit_core::PRESENTATION_RESOURCE_CLASS);
    browser.resources.push(conduit_core::resource_offer(
        "browser/presentation",
        conduit_core::PRESENTATION_RESOURCE_CLASS,
        1,
    ));
    browser
        .capabilities
        .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    browser
        .resources
        .sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
    let authorized = owner
        .browser_authorize_window("host/browser-test", Some(BROWSER_KEY), 10_000)
        .unwrap();
    let binding = LinkBindingId::from("line/test/browser-mask");
    let Out::Challenge { challenge, .. } = owner
        .browser_begin(
            &authorized.window_id,
            &binding,
            In::Advertise {
                protocol: PROTOCOL,
                advertisement: browser,
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
    owner
        .browser_planning_offer(
            &authorized.window_id,
            &snapshot.credential,
            &OfferDisclosureRequest {
                stage: OfferDisclosureStage::Planning,
                capability_ids: vec![mask.capability_id],
                resource_pool_ids: vec![conduit_core::ResourcePoolId::from("browser/presentation")],
            },
        )
        .unwrap();
    owner
        .browser_mask_route(
            &authorized.window_id,
            &snapshot.credential,
            &binding,
            Some(&super::route_tests::fixture_carrier_evidence(&snapshot)),
        )
        .unwrap();
    let joined_part_ids = owner
        .session
        .evidence()
        .membership
        .parts
        .iter()
        .map(|part| part.part_id.clone())
        .collect::<Vec<_>>();
    let prior_browser_seal = owner
        .pending_browser
        .as_ref()
        .unwrap()
        .current_mask_route()
        .unwrap()
        .0
        .clone();
    assert_eq!(
        Owner::current_presentation_routes(
            owner.host.advertisement(),
            owner.pending_browser.as_ref(),
            None,
        )
        .len(),
        1,
        "the current browser witness must not be withdrawn before the owner offer changes"
    );

    let (terminal, _provider) = std::os::unix::net::UnixStream::pair().unwrap();
    owner.host.attach_terminal_mask(terminal).unwrap();
    assert_ne!(
        prior_browser_seal.owner_host.offer_generation,
        owner.host.advertisement().offer_generation
    );
    // Browser membership and its historical seal remain. They do not become
    // a current route under the changed owner offer merely by staying joined.
    assert!(owner
        .pending_browser
        .as_ref()
        .unwrap()
        .current_mask_route()
        .is_some());
    let (face, seal) = owner.seal_attached_terminal_route().unwrap();
    let routes = Owner::current_presentation_routes(
        owner.host.advertisement(),
        owner.pending_browser.as_ref(),
        Some(&seal),
    );
    assert_eq!(routes.len(), 1);
    assert!(matches!(
        routes[0],
        conduit_presentation::CurrentOwnerPresentationRoute::Local { .. }
    ));
    let placement = seal.planned_mask.show_placement();
    let play = conduit_core::bind_active_play(
        &seal.planned_mask.plan.plan_id,
        &placement.host_id,
        &placement.boot_id,
        1,
    );
    let show = conduit_presentation::MaskShow::prepared(
        &seal.planned_mask,
        &face,
        play,
        face.subjects[0].identity.clone(),
        "terminal/test".into(),
        conduit_core::SignId::from("sign/test/terminal-prepared"),
    )
    .unwrap()
    .transition(
        conduit_presentation::ManifestationLifecycle::Available,
        conduit_core::SignId::from("sign/test/terminal-available"),
    )
    .unwrap();
    owner
        .acknowledge_attached_terminal_show(&seal, &show)
        .unwrap();
    assert_eq!(
        owner
            .presentation_wardrobe
            .as_ref()
            .unwrap()
            .plan()
            .routes
            .len(),
        1
    );
    assert_eq!(
        owner
            .session
            .evidence()
            .membership
            .parts
            .iter()
            .map(|part| part.part_id.clone())
            .collect::<Vec<_>>(),
        joined_part_ids
    );
    std::fs::remove_dir_all(root).unwrap();
}
