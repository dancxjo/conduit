use super::*;
use crate::durable_host::owner::{BrowserAdmittedSnapshot, BrowserCarrierLineEvidence, Owner};
use crate::durable_host_control::terminal_attach;
use conduit_body::{ResidentPlot, SpawnInvitationSecret};
use conduit_core::{BootId, HostId, OfferGeneration};
use conduit_presentation::ManifestationLifecycle;
use conduit_std_host::{
    browser_admission::{
        BrowserAdmissionEgress as Out, BrowserAdmissionIngress as In,
        BROWSER_ADMISSION_PROTOCOL as PROTOCOL,
    },
    hosted_audio::{
        AlsaPlaybackObservation, ExplicitPlaybackAuthorization, HostedPlaybackSelection,
    },
    hosted_speech_synthesis::EspeakDiscovery,
    StdHostConfig,
};
use std::{
    fs,
    os::unix::fs::{symlink, PermissionsExt},
    sync::{Arc, Barrier},
    time::{Duration, Instant},
};

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

fn fixture_carrier_evidence(snapshot: &BrowserAdmittedSnapshot) -> BrowserCarrierLineEvidence {
    let authorization = snapshot.line_authorization.as_ref().unwrap().clone();
    let owner = snapshot.owner_advertisement.as_deref().unwrap();
    let browser = snapshot.browser_advertisement.as_deref().unwrap();
    let descriptor = &conduit_host_browser_make::BROWSER_LINE_REALIZATIONS[0];
    let limits = conduit_core::LinkLimits {
        maximum_in_flight_items: descriptor.maximum_in_flight_items,
        maximum_payload_bytes: descriptor.maximum_payload_bytes,
        maximum_buffered_bytes: descriptor.maximum_buffered_bytes,
        maximum_frame_bytes: descriptor.maximum_frame_bytes,
    };
    let line = |direction: &str,
                source: &conduit_core::HostAdvertisement,
                sink: &conduit_core::HostAdvertisement,
                grant: &conduit_core::AuthorityGrantId| {
        let carrier = authorization.carrier_binding.as_str();
        let mut offer = conduit_core::process_owned_line_offer_with_limits(
            &format!("line/browser-mask/{carrier}/{direction}"),
            &format!("binding/browser-mask/{carrier}/{direction}"),
            conduit_core::BaseImplementationId::from(descriptor.base_implementation_id),
            &format!("base-instance/{carrier}"),
            source,
            sink,
            limits,
        );
        offer.binding.credential = conduit_core::LinkCredentialReference::Opaque(
            conduit_core::CredentialReferenceId::from(snapshot.credential.credential_id.as_str()),
        );
        offer.binding.authority = conduit_core::LinkAuthorityReference::Grant(grant.clone());
        offer.binding.source.endpoint_id =
            conduit_core::LinkEndpointId::from(format!("endpoint/{carrier}/{direction}/source"));
        offer.binding.sink.endpoint_id =
            conduit_core::LinkEndpointId::from(format!("endpoint/{carrier}/{direction}/sink"));
        offer.availability.sign_id =
            conduit_core::SignId::from(format!("sign/browser-mask/{carrier}/{direction}/ready"));
        offer.contract = descriptor.contract;
        offer
    };
    BrowserCarrierLineEvidence {
        face: line("face", owner, browser, &authorization.face_grant_id),
        returned: line("return", browser, owner, &authorization.return_grant_id),
        interaction: line(
            "interaction",
            browser,
            owner,
            &authorization.interaction_grant_id,
        ),
        authorization,
    }
}

fn selected_host(root: &std::path::Path) -> (StdHost, AttachedEquipment) {
    let mut host = host("host/owner-speech-test", "boot/owner-speech-test");
    let observation = AlsaPlaybackObservation {
        card_index: 999,
        card_id: "unavailable-test-card".into(),
        card_name: "Unavailable test speaker".into(),
        device: 0,
        device_name: "Unavailable test device".into(),
        base_identity: "fixture/speaker".into(),
    };
    let offered = host.advertisement().clone();
    let playback = HostedPlaybackSelection::from_observation(
        observation,
        offered.boot_id.clone(),
        offered.offer_generation,
    )
    .with_bounded_speech_queue();
    let data = root.join("espeak-ng-data");
    fs::create_dir(&data).unwrap();
    fs::write(data.join("voice"), b"fixture voice").unwrap();
    let executable = root.join("espeak-ng");
    fs::write(&executable, b"fixture executable never launched").unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    let engine = root.join("libespeak-ng.so.1.0");
    fs::write(&engine, b"fixture engine").unwrap();
    symlink("libespeak-ng.so.1.0", root.join("libespeak-ng.so.1")).unwrap();
    let discovery = EspeakDiscovery::inspect(&executable, &data, "en-us", &[engine]).unwrap();
    let provider_sha256 = discovery.provider_sha256.clone();
    let adapter = discovery
        .initialize(
            offered.host_id.clone(),
            offered.boot_id.clone(),
            offered.offer_generation,
            "grant/test-speech-provider".into(),
            Duration::from_secs(5),
        )
        .unwrap();
    host.attach_selected_playback(playback.clone()).unwrap();
    host.attach_espeak_speech_for_selected_playback(adapter)
        .unwrap();
    let equipment = AttachedEquipment {
        playback,
        authorization: ExplicitPlaybackAuthorization::new("grant/test-speaker").unwrap(),
        provider_sha256,
        before_play: None,
    };
    assert!(equipment.matches(&host));
    (host, equipment)
}

fn acknowledged_browser_show(
    owner: &mut Owner,
    root: &std::path::Path,
) -> (String, LinkBindingId, OwnerFaceSnapshotRequest, MaskShow) {
    let browser = host("host/browser-speech-test", "boot/browser-speech-test");
    let mut advertisement = browser.advertisement().clone();
    advertisement
        .capabilities
        .push(conduit_browser_mask_offer::offer());
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
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    advertisement
        .resources
        .sort_by(|a, b| a.pool_id.cmp(&b.pool_id));
    let binding = LinkBindingId::from("line/test/selected-speech-browser");
    let authorized = owner
        .browser_authorize_window("host/browser-speech-test", Some(BROWSER_KEY), 10_000)
        .unwrap();
    let Out::Challenge { challenge, .. } = owner
        .browser_begin(
            &authorized.window_id,
            &binding,
            In::Advertise {
                protocol: PROTOCOL,
                advertisement,
                friendly_label: "Browser speech test".into(),
                verifying_key: BROWSER_KEY.to_vec(),
                freshness_sequence: 1,
            },
            512,
        )
        .unwrap()
    else {
        panic!("expected challenge");
    };
    let secret = SpawnInvitationSecret::from_csprng_bytes([7; 32]).unwrap();
    let snapshot = owner
        .browser_complete(
            root,
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
    let route = owner
        .browser_mask_route(
            &authorized.window_id,
            &snapshot.credential,
            &binding,
            Some(&fixture_carrier_evidence(&snapshot)),
        )
        .unwrap();
    let face = owner.local_face_snapshot().unwrap();
    let placement = route.planned_mask.show_placement();
    let play = conduit_core::bind_active_play(
        &route.planned_mask.plan.plan_id,
        &placement.host_id,
        &placement.boot_id,
        1,
    );
    let show = MaskShow::prepared(
        &route.planned_mask,
        &face,
        play,
        face.subjects[0].identity.clone(),
        "browser/document".into(),
        conduit_core::SignId::from("sign/test/speech-source-prepared"),
    )
    .unwrap()
    .transition(
        ManifestationLifecycle::Available,
        conduit_core::SignId::from("sign/test/speech-source-available"),
    )
    .unwrap();
    let request = OwnerFaceSnapshotRequest {
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
        .acknowledge_browser_mask_show(&authorized.window_id, &binding, &request, &show)
        .unwrap();
    (authorized.window_id, binding, request, show)
}

#[test]
fn selected_direct_readout_stops_and_restores_the_one_current_host() {
    let root =
        std::env::temp_dir().join(crate::durable_host::fresh_identity("speech-test", "owner"));
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("installation.json"),
        serde_json::to_vec(&serde_json::json!({
            "schema":"conduit.install/durable-host@1",
            "host_id":"host/owner-speech-test",
            "release_source_identity":"source/test",
            "release_bundle_sha256":format!("sha256:{}", "a".repeat(64)),
            "product_executable":"fixture-unused",
            "body_state":null,
            "joined_body_state":null
        }))
        .unwrap(),
    )
    .unwrap();
    let (host, mut equipment) = selected_host(&root);
    let original = host.advertisement().clone();
    let plot = crate::plot_source::parse(
        "plot hello {\n show: presentation/text\n \"Hello.\" >> show\n}.",
    )
    .unwrap()
    .expand_entry_for_authoring()
    .unwrap();
    let resident = ResidentPlot::new(
        plot.expanded.source_document_id,
        plot.expanded.checked_plot_id,
    );
    let mut owner = Owner::open(host, resident, None, "Same Body").unwrap();
    owner.persist(&root).unwrap();
    let (window_id, binding, request, show) = acknowledged_browser_show(&mut owner, &root);
    let gate = Arc::new(Barrier::new(2));
    equipment.before_play = Some(gate.clone());
    let mut runtime = DurableHostRuntime {
        target_id: "std/x86_64/computer".into(),
        image_content_digest: "fixture".into(),
        host: HostSource::Body {
            owner: Box::new(owner),
            root: root.clone(),
            running: None,
        },
        birth: None,
        birth_root: None,
        remote_fragment: None,
        pool_member: None,
        cancellation_signal: None,
        next_observation_sequence: 0,
        terminal_route: None,
        selected_speech_equipment: None,
        speech_worker: None,
        speech_terminal: None,
        direct_spoken_worker: None,
        direct_spoken_terminal: None,
    };
    assert!(runtime
        .start_browser_speech(
            window_id.clone(),
            binding.clone(),
            request.clone(),
            show.clone()
        )
        .unwrap_err()
        .contains("no selected speech equipment"));
    assert_eq!(runtime.host.advertisement(), &original);
    runtime.selected_speech_equipment = Some(equipment);
    let operation_id = runtime
        .start_browser_speech(
            window_id.clone(),
            binding.clone(),
            request.clone(),
            show.clone(),
        )
        .unwrap();
    // Worker now owns the exact Host but has not begun any Play.
    gate.wait();
    // Exercise the service loop while the selected speaker owns the sole Host.
    // A lulled Body has no running Body worker, but that does not make its Host idle.
    terminal_attach::retire_closed_attachment(&root, &mut runtime).unwrap();
    assert!(!terminal_attach::is_attached(&mut runtime));
    runtime.progress_owned_body().unwrap();
    runtime.progress_browser_speech().unwrap();
    assert_eq!(
        runtime.browser_speech_status(&operation_id).unwrap()["state"],
        "running"
    );
    assert!(matches!(&runtime.host, HostSource::Body { owner, .. } if owner.host.is_playing()));
    runtime.stop_browser_speech(&operation_id).unwrap();
    gate.wait();
    let deadline = Instant::now() + Duration::from_secs(5);
    while runtime
        .speech_worker
        .as_ref()
        .is_some_and(|worker| !worker.thread.is_finished())
    {
        assert!(
            Instant::now() < deadline,
            "selected speech worker failed to stop"
        );
        std::thread::yield_now();
    }
    let terminal = runtime.browser_speech_status(&operation_id).unwrap();
    assert_eq!(terminal["outcome"], "cancelled");
    assert_eq!(terminal["source_show_still_current"], true);
    assert_eq!(
        runtime.browser_speech_status(&operation_id).unwrap(),
        terminal
    );
    assert_eq!(runtime.host.advertisement(), &original);
    assert!(matches!(&runtime.host, HostSource::Body { owner, .. } if !owner.host.is_playing()));

    let leaving_operation = runtime
        .start_browser_speech(window_id.clone(), binding, request, show)
        .unwrap();
    gate.wait();
    runtime.browser_cancel_window(&window_id).unwrap();
    gate.wait();
    let deadline = Instant::now() + Duration::from_secs(5);
    while runtime
        .speech_worker
        .as_ref()
        .is_some_and(|worker| !worker.thread.is_finished())
    {
        assert!(
            Instant::now() < deadline,
            "selected speech survived browser cancellation"
        );
        std::thread::yield_now();
    }
    let terminal = runtime.browser_speech_status(&leaving_operation).unwrap();
    assert_eq!(terminal["outcome"], "cancelled");
    assert_eq!(terminal["source_show_still_current"], false);
    assert_eq!(runtime.host.advertisement(), &original);
    fs::remove_dir_all(root).unwrap();
}
