use super::*;
use crate::durable_host as installed;
use std::sync::Arc;

#[test]
fn source_consumer_refusal_returns_original_host_after_worker_quiescence() {
    let root = std::env::temp_dir().join(installed::fresh_identity("source-refusal", "worker"));
    std::fs::create_dir_all(&root).unwrap();
    let installation = installed::Installation {
        schema: installed::INSTALL_SCHEMA.into(),
        host_id: "host/source-refusal".into(),
        release_source_identity: "source/test".into(),
        release_bundle_sha256: installed::digest(b"bundle/test"),
        product_executable: "fixture-unused".into(),
        body_state: None,
        joined_body_state: None,
        selected_speech: None,
        selected_model: None,
        selected_host_configuration: None,
        selected_todo_checkpoint: None,
    };
    installed::write_json_atomic(&root.join("installation.json"), &installation).unwrap();
    let text = include_str!("../../../../../plots/clock/main.conduit");
    let checked = crate::plot_source::parse(text)
        .unwrap()
        .expand_entry_for_authoring()
        .unwrap();
    let host = StdHost::new_with_config(conduit_std_host::StdHostConfig {
        host_id: "host/source-refusal".into(),
        boot_id: "boot/source-refusal".into(),
        offer_generation: conduit_core::OfferGeneration(1),
    });
    let mut owner = Owner::open(
        host,
        ResidentPlot::new(
            checked.expanded.source_document_id.clone(),
            checked.expanded.checked_plot_id.clone(),
        ),
        None,
        "Clock",
    )
    .unwrap();
    owner.set_resident_plot_name(&checked).unwrap();
    owner.persist(&root).unwrap();
    std::fs::write(root.join("body/source.conduit"), text).unwrap();
    let mut worker = owner.start_service_run(&root, 5000).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while owner.current_play_id().is_none() {
        assert!(Instant::now() < deadline);
        assert!(!worker.progress(&mut owner, &root).unwrap());
        thread::sleep(Duration::from_millis(5));
    }
    // Inject a foreign Source message into the actual one-slot control-side
    // ingress while a real clock Body owns the original Host on its worker.
    let selection = Arc::new(native::SourceSelection {
        attachment_id: "foreign".into(),
        host_id: "host/foreign".into(),
        boot_id: "boot/foreign".into(),
        capability_id: "cap/source".into(),
        provider_instance_id: "base/foreign".into(),
        provider_generation: 1,
        plan_id: "plan/foreign".into(),
        fragment_id: "fragment/foreign".into(),
        placement_id: "placement/foreign".into(),
        configuration_id: "config/foreign".into(),
        firmware_sha256: [7; 32],
    });
    let play = conduit_core::bind_active_play(
        &selection.plan_id,
        &selection.host_id,
        &selection.boot_id,
        1,
    );
    let (sender, receiver) = mpsc::sync_channel(1);
    worker.live_fore = Some(receiver);
    let mut bytes = [0; conduit_environment_measurement::source_observation::MAXIMUM_BYTES];
    bytes[..3].copy_from_slice(b"bad");
    sender
        .send(native::LiveFore {
            scope: Arc::new(native::CompletedScope {
                selection,
                native_play: play,
            }),
            sequence: 7,
            length: 3,
            bytes,
        })
        .unwrap_or_else(|_| panic!("original ingress is open"));
    drop(sender);
    while !worker.progress(&mut owner, &root).unwrap() {
        assert!(
            Instant::now() < deadline,
            "refused consumer failed to quiesce"
        );
        thread::sleep(Duration::from_millis(5));
    }
    assert!(!owner.host.is_playing());
    assert!(owner.current_play_id().is_none());
    let refusal = &owner.last_execution.as_ref().unwrap()["source_fore_refusal"];
    assert_eq!(refusal["first"]["sequence"], 7);
    assert_eq!(refusal["first"]["canonical_hex"], "626164");
    assert_eq!(refusal["durable_recording_acknowledged"], false);
    assert_eq!(refusal["producer_retirement_authorized"], false);
    assert_eq!(
        owner.host.advertisement().host_id.as_str(),
        "host/source-refusal"
    );
    owner.persist(&root).unwrap();
    std::fs::remove_dir_all(root).unwrap();
}
