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
        &mut Vec::new(),
        || false,
    )
    .unwrap();
    assert!(runtime.pending_worker_text.is_some());
    (runtime, fragment)
}

fn result(runtime: &DurableHostRuntime) -> WorkerTextResult {
    runtime
        .inspect_worker_text(
            runtime.remote_fragment.as_ref().unwrap().identity(),
            runtime.host.advertisement().offer_generation.0,
        )
        .unwrap()
        .unwrap()
}

#[test]
fn exact_acknowledgement_rejects_tampering_and_replay_without_duplicate_effect() {
    let (mut runtime, _) = pending();
    let expected = result(&runtime);
    let mut wire = serde_json::to_value(&expected).unwrap();
    wire["fragment"]["text"][0]["text"] = "forged".into();
    let forged = serde_json::from_value(wire).unwrap();
    let mut output = Vec::new();
    assert!(runtime
        .complete_worker_text_to(&forged, &mut output)
        .is_err());
    assert!(output.is_empty());
    assert_eq!(result(&runtime), expected);
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
    let expected = result(&runtime);
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
