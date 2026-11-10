#[path = "common_episode_loop/mod.rs"]
mod common;
#[path = "../../../semantics/catalog/tests/common/experience_evidence.rs"]
mod evidence;

use common::{evidence_harness, run, terminal, Collector};
use conduit_core::{ConnectionTrack, TerminalDisposition};
use conduit_human::*;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduit_std_host::{ExternalForeInput, RunControl};
use evidence::{items, observation, selection_with_relationships};

const PROGRAM: &str = include_str!("../../../plots/experience-response/main.conduit");

#[test]
fn experience_relationships_reach_the_model_across_two_revisions() {
    let mut harness = evidence_harness(PROGRAM);
    let selection = selection_with_relationships(&["a", "b"], &[("a", "b")]);
    let original = observation("a");
    let observed = conduit_human::ExperienceEvidenceItem::new(
        *original.availability(),
        *original.certainty(),
        original.content_kind().clone(),
        *original.domain(),
        original.encoded_content().clone(),
        original.identity().clone(),
        ExperienceEvidenceInstant::known(
            conduit_time::NativeTemporalInstant::new(
                "clock/test".into(),
                1,
                conduit_time::NativeTemporalScale::Milliseconds,
                100,
                2,
            )
            .unwrap(),
        )
        .unwrap(),
        *original.origin(),
        original.provenance_revision().clone(),
        original.recorded_at().clone(),
        *original.revision(),
        original.sources().clone(),
        *original.temporal_role(),
    )
    .unwrap();
    let first = ExperienceEvidenceRequest::new(
        items(vec![
            evidence::with_sources(observation("b"), vec![evidence::resource_source()]),
            observed,
        ]),
        selection,
    )
    .unwrap();
    let next = evidence::item(
        "next",
        ExperienceOrigin::Remembered,
        ExperienceTemporalRole::Historical,
        ExperienceAvailability::Present,
        8,
        "provenance/8",
        b"recalled fact",
    );
    let selection = ExperienceEvidenceSelection::new(
        evidence::selection(&["next"]).item_identities().clone(),
        evidence::identity("provenance/8"),
        ExperienceEvidenceRelationships::new(BoundedSequence::new()).unwrap(),
        8,
    )
    .unwrap();
    let second = ExperienceEvidenceRequest::new(items(vec![next]), selection).unwrap();
    let inputs = [first, second]
        .into_iter()
        .map(|request| ExternalForeInput {
            front_port_id: "evidence".into(),
            track: ConnectionTrack::Payload,
            bytes: request.encode().unwrap(),
        })
        .collect::<Vec<_>>();
    let mut collector = Collector::default();
    let report = run(
        &mut harness,
        &inputs,
        &mut collector,
        &RunControl::default(),
    )
    .unwrap();
    assert_eq!(terminal(&report), &TerminalDisposition::Completed);
    assert_eq!(collector.0.len(), 2);
    let requests = harness.requests.lock().unwrap();
    let first: serde_json::Value = serde_json::from_str(&requests[0]).unwrap();
    let second: serde_json::Value = serde_json::from_str(&requests[1]).unwrap();
    assert_eq!(first["evidence"][0]["identity"], "a");
    assert_eq!(
        first["evidence"][0]["observed_at"]["clock_basis"],
        "clock/test"
    );
    assert_eq!(first["evidence"][0]["observed_at"]["ticks"], 100);
    assert_eq!(
        first["evidence"][0]["classifications"]["origin"],
        "observation"
    );
    assert!(first["evidence"][1]["sources"][0]["resource"].is_array());
    assert_eq!(first["relationships"][0]["kind"], "contradicts");
    assert_eq!(first["relationships"][0]["subject"], "a");
    assert_eq!(first["relationships"][0]["object"], "b");
    assert_eq!(first["revision"], 7);
    assert_eq!(second["revision"], 8);
    assert_eq!(second["evidence"][0]["identity"], "next");
    assert_eq!(
        second["evidence"][0]["classifications"]["origin"],
        "remembered"
    );
    assert_eq!(
        second["evidence"][0]["classifications"]["temporal_role"],
        "historical"
    );
    assert_eq!(second["relationships"], serde_json::json!([]));
    let kernel = report.kernel.unwrap();
    println!(
        "CONDUIT_PLOT_EVIDENCE={{\"plan_id\":\"{}\",\"play_id\":\"{}\",\"workload_revision\":null}}",
        harness.plan_id, kernel.active_play_id.as_str()
    );
}

#[test]
fn oversized_rendering_refuses_before_model_admission() {
    let mut harness = evidence_harness(PROGRAM);
    let item = evidence::item(
        "oversize",
        ExperienceOrigin::Observation,
        ExperienceTemporalRole::Current,
        ExperienceAvailability::Present,
        7,
        "provenance/7",
        &[0; 2048],
    );
    let request = evidence::request(&["oversize"], vec![item]);
    let input = ExternalForeInput {
        front_port_id: "evidence".into(),
        track: ConnectionTrack::Payload,
        bytes: request.encode().unwrap(),
    };
    let mut collector = Collector::default();
    let error = run(
        &mut harness,
        &[input],
        &mut collector,
        &RunControl::default(),
    )
    .expect_err("JSON escaping exceeds the admitted model input size");
    assert!(
        error.contains("InvalidInput") && error.contains("detail: 2"),
        "{error}"
    );
    assert!(collector.0.is_empty());
    assert!(harness.requests.lock().unwrap().is_empty());
}

#[test]
#[ignore = "requires a running loopback Ollama with gemma3:1b already installed"]
fn live_model_uses_selected_experience() {
    use conduit_ai::{LocalModelKindProfile, ModelDerivedResult};
    use conduit_std_host::hosted_local_model::OllamaDiscovery;
    let adapter = OllamaDiscovery::discover_at("http://127.0.0.1:11434", "gemma3:1b")
        .unwrap()
        .initialize(2048, vec![LocalModelKindProfile::GenerateFlow])
        .unwrap();
    let mut harness = common::evidence_harness_with_model(PROGRAM, Box::new(adapter));
    let memory = evidence::item(
        "past/1",
        ExperienceOrigin::Remembered,
        ExperienceTemporalRole::Historical,
        ExperienceAvailability::Present,
        7,
        "provenance/7",
        b"The hallway switch label was COPPER KITE.",
    );
    let input = ExternalForeInput {
        front_port_id: "evidence".into(),
        track: ConnectionTrack::Payload,
        bytes: evidence::request(&["past/1"], vec![memory])
            .encode()
            .unwrap(),
    };
    let mut collector = Collector::default();
    let report = run(
        &mut harness,
        &[input],
        &mut collector,
        &RunControl::default(),
    )
    .unwrap();
    assert_eq!(terminal(&report), &TerminalDisposition::Completed);
    let [response] = collector.0.as_slice() else {
        panic!("expected one model response");
    };
    let result: ModelDerivedResult = serde_json::from_slice(&response.bytes).unwrap();
    let content = std::str::from_utf8(&result.payload).unwrap();
    println!("{}: {content}", result.run_identity);
    assert!(content.to_ascii_uppercase().contains("COPPER KITE"));
}
