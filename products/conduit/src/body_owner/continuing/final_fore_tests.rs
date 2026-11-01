//! Deterministic final-slot custody through the actual joined-worker path.
use super::*;
use crate::durable_host as installed;
use std::sync::Arc;
fn fixture() -> (std::path::PathBuf, Owner, RunWorker) {
    let root = std::env::temp_dir().join(installed::fresh_identity("source-final-fore", "worker"));
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
    // Consume the real worker start handshake without polling or sleeping.
    let (play, wake) = worker.started.recv_timeout(Duration::from_secs(3)).unwrap();
    owner.retain_started(&root, play, wake).unwrap();
    worker.acknowledge.take().unwrap().send(Ok(())).unwrap();
    (root, owner, worker)
}
fn foreign_fore(sequence: u64) -> native::LiveFore {
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
    let mut bytes = [0; conduit_environment_measurement::source_observation::MAXIMUM_BYTES];
    bytes[..3].copy_from_slice(b"bad");
    native::LiveFore {
        scope: Arc::new(native::CompletedScope {
            selection,
            native_play: play,
        }),
        sequence,
        length: 3,
        bytes,
    }
}
fn final_slot(prior_refusal: bool) {
    let (root, mut owner, mut worker) = fixture();
    assert!(owner.current_play_id().is_some());
    assert!(owner.last_execution.is_none());
    let (sender, receiver) = mpsc::sync_channel(1);
    worker.live_fore = Some(receiver);
    if prior_refusal {
        worker.fore_refusal = Some(fore_refusal::ForeRefusal::new(
            foreign_fore(7),
            "first refused".into(),
        ));
    }
    worker.request_lull().unwrap();
    let original = worker.thread.take().unwrap();
    let (release, publish) = mpsc::channel();
    // The real Host returns through its actual worker. The gated wrapper
    // deterministically publishes AFTER the consumer observed pre-join Empty,
    // then drops its only sender and exits before the actual join returns.
    worker.thread = Some(thread::spawn(move || {
        let outcome = original.join().unwrap();
        publish.recv().unwrap();
        sender
            .send(foreign_fore(8))
            .unwrap_or_else(|_| panic!("final slot closed before custody transfer"));
        outcome
    }));
    assert!(matches!(
        worker.live_fore.as_ref().unwrap().try_recv(),
        Err(TryRecvError::Empty)
    ));
    release.send(()).unwrap();
    let outcome = worker.thread.take().unwrap().join().unwrap();
    // Isolate the post-join behavior: no new stop request may be manufactured
    // for a producer whose actual thread and sender have already retired.
    worker.control = RunControl::default();
    assert!(!worker.control.stop_requested());
    assert!(owner.last_execution.is_none());
    assert!(worker.finish_joined(&mut owner, &root, outcome).unwrap());
    assert!(!worker.control.stop_requested());
    assert!(!owner.host.is_playing());
    assert!(owner.current_play_id().is_none());
    assert_eq!(
        owner.host.advertisement().host_id.as_str(),
        "host/source-refusal"
    );
    let refusal = &owner.last_execution.as_ref().unwrap()["source_fore_refusal"];
    assert_eq!(refusal["durable_recording_acknowledged"], false);
    assert_eq!(refusal["producer_retirement_authorized"], false);
    assert_eq!(
        refusal["first"]["sequence"],
        if prior_refusal { 7 } else { 8 }
    );
    if prior_refusal {
        assert_eq!(refusal["trailing"]["sequence"], 8);
    } else {
        assert!(refusal["trailing"].is_null());
    }
    assert_eq!(
        refusal[if prior_refusal { "trailing" } else { "first" }]["canonical_hex"],
        "626164"
    );
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn final_fore_after_prejoin_empty_is_refused_before_lifecycle_receipt() {
    final_slot(false);
}
#[test]
fn final_fore_after_existing_refusal_retains_trailing_slot_before_receipt() {
    final_slot(true);
}
