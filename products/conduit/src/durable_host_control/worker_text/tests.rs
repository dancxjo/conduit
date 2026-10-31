use super::*;
use conduit_core::{BootId, HostId, OfferGeneration, PlanFragment};
use conduit_std_host::StdHostConfig;

fn pending() -> (DurableHostRuntime, PlanFragment) {
    let host = StdHost::new_with_config(StdHostConfig {
        host_id: HostId::from("worker-control-test"),
        boot_id: BootId::from("worker-control-boot"),
        offer_generation: OfferGeneration(1),
    });
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile).unwrap();
    let syntax = conduit_plot::parse_syntax_document(
        "plot worker_control {\n \"greenhouse\" >> text/upper >> face/text(\"Result\")\n}\n",
    );
    let checked = conduit_plot::check_syntax_document(&syntax, &startup).unwrap();
    let expanded =
        conduit_plot::expand_canonical_plot(&checked, "worker_control", &profile).unwrap();
    let plan = host.plan_expanded_local(&expanded).unwrap();
    let fragment = plan.fragments[0].clone();
    let mut runtime = DurableHostRuntime::new("std/x86_64/computer".into(), "fixture".into(), host);
    let admitted = runtime.host.prepare_remote_fragment(&fragment).unwrap();
    runtime.remote_fragment = Some(admitted);
    drive_remote_fragment(
        &mut runtime.host,
        runtime.remote_fragment.as_mut().unwrap(),
        &mut runtime.pending_worker_text,
        &mut runtime.pending_native_source,
        &mut Vec::new(),
        || false,
    )
    .unwrap();
    assert!(runtime.pending_worker_text.is_some());
    (runtime, fragment)
}

fn result(runtime: &mut DurableHostRuntime) -> WorkerTextResult {
    let identity = runtime.remote_fragment.as_ref().unwrap().identity().clone();
    runtime
        .inspect_worker_text(&identity, runtime.host.advertisement().offer_generation.0)
        .unwrap()
        .unwrap()
}

#[test]
fn exact_acknowledgement_rejects_tampering_and_replay_without_duplicate_effect() {
    let (mut runtime, _) = pending();
    let expected = result(&mut runtime);
    let mut wire = serde_json::to_value(&expected).unwrap();
    wire["fragment"]["text"][0]["text"] = "forged".into();
    let forged = serde_json::from_value(wire).unwrap();
    let mut output = Vec::new();
    assert!(runtime
        .complete_worker_text_to(&forged, &mut output)
        .is_err());
    assert!(output.is_empty());
    assert_eq!(result(&mut runtime), expected);
    runtime
        .complete_worker_text_to(&expected, &mut output)
        .unwrap();
    let first = output.clone();
    assert!(String::from_utf8(first.clone())
        .unwrap()
        .contains("GREENHOUSE"));
    assert!(runtime
        .complete_worker_text_to(&expected, &mut output)
        .is_err());
    assert_eq!(output, first);
    runtime.release_remote().unwrap();
}

#[test]
fn write_failure_cancels_releases_and_forbids_retry() {
    struct Refuse;
    impl Write for Refuse {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("test refused"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let (mut runtime, fragment) = pending();
    let expected = result(&mut runtime);
    let error = runtime
        .complete_worker_text_to(&expected, &mut Refuse)
        .unwrap_err();
    assert!(error.starts_with("worker-text-effect-outcome-unknown:"));
    assert!(runtime.remote_fragment.is_none());
    assert!(runtime.pending_worker_text.is_none());
    assert!(runtime
        .complete_worker_text_to(&expected, &mut Vec::new())
        .is_err());
    let next = runtime.host.prepare_remote_fragment(&fragment).unwrap();
    runtime.host.release_remote_fragment(next).unwrap();
}

#[test]
fn inspection_requires_current_play_and_offer() {
    let (mut runtime, _) = pending();
    let mut identity = runtime.remote_fragment.as_ref().unwrap().identity().clone();
    let generation = runtime.host.advertisement().offer_generation.0;
    assert!(runtime
        .inspect_worker_text(&identity, generation + 1)
        .is_err());
    identity.boot_id = BootId::from("foreign");
    assert!(runtime.inspect_worker_text(&identity, generation).is_err());
    runtime.release_remote().unwrap();
}

fn marker(runtime: &mut DurableHostRuntime) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "worker-text-cancel-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    runtime.install_cancellation_signal(&root);
    root
}

#[test]
fn exact_cancellation_before_acknowledgement_prevents_effect_and_releases() {
    let (mut runtime, fragment) = pending();
    let expected = result(&mut runtime);
    let root = marker(&mut runtime);
    signal_remote_cancellation(
        &root,
        runtime
            .remote_fragment
            .as_ref()
            .unwrap()
            .identity()
            .active_play_id
            .as_str(),
    )
    .unwrap();
    let mut output = Vec::new();
    assert_eq!(
        runtime
            .complete_worker_text_to(&expected, &mut output)
            .unwrap_err(),
        "worker-text-cancelled"
    );
    assert!(output.is_empty());
    assert!(runtime.remote_fragment.is_none());
    assert!(runtime.pending_worker_text.is_none());
    let next = runtime.host.prepare_remote_fragment(&fragment).unwrap();
    runtime.host.release_remote_fragment(next).unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn exact_cancellation_before_inspection_removes_pending_result() {
    let (mut runtime, _) = pending();
    let identity = runtime.remote_fragment.as_ref().unwrap().identity().clone();
    let generation = runtime.host.advertisement().offer_generation.0;
    let root = marker(&mut runtime);
    signal_remote_cancellation(&root, identity.active_play_id.as_str()).unwrap();
    assert_eq!(
        runtime
            .inspect_worker_text(&identity, generation)
            .unwrap_err(),
        "worker-text-cancelled"
    );
    assert!(runtime.remote_fragment.is_none());
    assert!(runtime.pending_worker_text.is_none());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn foreign_and_prefix_extended_cancellation_do_not_cancel_current_play() {
    let (mut runtime, _) = pending();
    let expected = result(&mut runtime);
    let root = marker(&mut runtime);
    signal_remote_cancellation(&root, "foreign-play").unwrap();
    assert_eq!(result(&mut runtime), expected);
    let extended = format!(
        "{}-foreign",
        runtime
            .remote_fragment
            .as_ref()
            .unwrap()
            .identity()
            .active_play_id
            .as_str()
    );
    signal_remote_cancellation(&root, &extended).unwrap();
    assert_eq!(result(&mut runtime), expected);
    runtime.release_remote().unwrap();
    fs::remove_dir_all(root).unwrap();
}
