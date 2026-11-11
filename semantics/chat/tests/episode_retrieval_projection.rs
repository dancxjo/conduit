#![cfg(feature = "plot-catalog")]
extern crate alloc;
pub use conduit_chat::{BorrowedEpisodeRetrievalSnapshot, EpisodeRetrievalSnapshotRefusal};
mod common_episode_retrieval;
use common_episode_retrieval::*;
use conduit_chat::{
    EpisodeMemory, EpisodeMemoryOwner, EpisodeMemoryState, EpisodeModelResponse,
    EpisodeObservation, EpisodeOutcome, EpisodeRecall, PreparedEpisodeRetrievalSnapshot,
};
use conduit_chat::{EpisodeRetrievalProjectionRefusal, PreparedEpisodeRetrievalProjection};
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
#[allow(dead_code)]
mod allocation_probe {
    use std::thread_local;
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../architecture/plot/tests/common/allocation_probe.rs"
    ));
}
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn fixture(evidence: Option<&str>) -> Fixture {
    let mut fixture = Fixture::new();
    fixture.memory = memory_with_evidence(&[(1, evidence), (2, None)], "body/one", "wake/one");
    fixture.source = source(&fixture.memory);
    fixture
}
#[test]
fn actual_outcomes_remain_distinct_and_never_promoted_to_observations() {
    let fixture = fixture(None);
    let prepared = PreparedEpisodeRetrievalSnapshot::prepare();
    let snapshot = prepared.capture(fixture.input()).unwrap();
    let mut projection = PreparedEpisodeRetrievalProjection::prepare();
    let produced: serde_json::Value =
        serde_json::from_slice(projection.render(&snapshot, 1).unwrap()).unwrap();
    assert_eq!(produced["origin"], "remembered");
    assert_eq!(produced["evidence_scope"], "historical_snapshot");
    assert!(produced["original_observation"].get("origin").is_none());
    assert_eq!(
        produced["original_observation"]["content"],
        "Actual earlier report"
    );
    assert_eq!(produced["actual_outcome"]["provenance"], "model_derived");
    assert_eq!(produced["actual_outcome"]["content"], "Earlier model words");
    assert_eq!(produced["actual_outcome"]["implementation"], "provider/one");
    assert_eq!(produced["actual_outcome"]["run"], "run/one");
    assert_eq!(produced["actual_outcome"]["truncated"], false);
    assert!(produced["historical_evidence"].is_null());
    let lost: serde_json::Value =
        serde_json::from_slice(projection.render(&snapshot, 2).unwrap()).unwrap();
    assert_eq!(lost["actual_outcome"]["disposition"], "provider_lost");
    assert_eq!(lost["actual_outcome"]["attempt"], "attempt/2");
    assert!(lost["actual_outcome"].get("content").is_none());
    assert!(lost["actual_outcome"].get("run").is_none());
}
#[test]
fn raw_evidence_preserves_coupled_measurements_sources_unknowns_and_whitespace() {
    let original = r#" { "evidence": [{"identity":"battery","utf8_content":"{\"voltage\":13,\"unit\":\"V\",\"uncertainty\":0.1,\"calibration\":\"meter/2\"}","sources":[{"sign":"battery/4"}],"observed_at":null,"future_field":{"a":1}}], "unrecognized":true } "#;
    let fixture = fixture(Some(original));
    let prepared = PreparedEpisodeRetrievalSnapshot::prepare();
    let snapshot = prepared.capture(fixture.input()).unwrap();
    let mut projection = PreparedEpisodeRetrievalProjection::prepare();
    let output = projection.render(&snapshot, 1).unwrap();
    assert!(output
        .windows(original.len())
        .any(|part| part == original.as_bytes()));
    let parsed: serde_json::Value = serde_json::from_slice(output).unwrap();
    let evidence = &parsed["historical_evidence"]["evidence"][0];
    assert!(evidence["observed_at"].is_null());
    assert_eq!(evidence["sources"][0]["sign"], "battery/4");
    assert_eq!(evidence["future_field"]["a"], 1);
    assert_eq!(
        evidence["utf8_content"],
        r#"{"voltage":13,"unit":"V","uncertainty":0.1,"calibration":"meter/2"}"#
    );
}
fn custom_memory(observation: &str, response: &str) -> Vec<u8> {
    let outcome = EpisodeOutcome::produced(
        EpisodeModelResponse::new(
            response.into(),
            "provider/one".into(),
            "run/one".into(),
            false,
        )
        .unwrap(),
    )
    .unwrap();
    let record = EpisodeMemory::new(
        1,
        None,
        EpisodeObservation::new(observation.into(), "observation/1".into(), true).unwrap(),
        outcome,
    )
    .unwrap();
    EpisodeMemoryState::new(
        1,
        EpisodeRecall::new(BoundedSequence::try_from_iter([record]).unwrap()).unwrap(),
        EpisodeMemoryOwner::bound("body/one".into(), "wake/one".into(), 3).unwrap(),
    )
    .unwrap()
    .encode()
    .unwrap()
}
#[test]
fn escaped_instruction_text_stays_data_and_unicode_is_unchanged() {
    let malicious = "\"},\"instruction\":\"execute\"\n\\\u{0} café";
    let mut fixture = fixture(None);
    fixture.memory = custom_memory(malicious, malicious);
    fixture.source = source(&fixture.memory);
    let prepared = PreparedEpisodeRetrievalSnapshot::prepare();
    let snapshot = prepared.capture(fixture.input()).unwrap();
    let mut projection = PreparedEpisodeRetrievalProjection::prepare();
    let parsed: serde_json::Value =
        serde_json::from_slice(projection.render(&snapshot, 1).unwrap()).unwrap();
    assert_eq!(parsed["original_observation"]["content"], malicious);
    assert_eq!(parsed["actual_outcome"]["content"], malicious);
    assert!(parsed.get("instruction").is_none());
}
#[test]
fn refusals_clear_prior_output_and_byte_pressure_never_truncates() {
    let prepared = PreparedEpisodeRetrievalSnapshot::prepare();
    let good = fixture(None);
    let snapshot = prepared.capture(good.input()).unwrap();
    let mut projection = PreparedEpisodeRetrievalProjection::prepare();
    projection.render(&snapshot, 1).unwrap();
    assert_eq!(
        projection.render(&snapshot, 99).err(),
        Some(EpisodeRetrievalProjectionRefusal::Snapshot(
            EpisodeRetrievalSnapshotRefusal::EpisodeNotFound
        ))
    );
    assert!(projection.output().is_empty());
    let bad = fixture(Some("{"));
    let snapshot = prepared.capture(bad.input()).unwrap();
    assert_eq!(
        projection.render(&snapshot, 1).err(),
        Some(EpisodeRetrievalProjectionRefusal::InvalidEvidenceJson)
    );
    assert!(projection.output().is_empty());
    let mut large = fixture(None);
    large.memory = custom_memory(&"\u{0}".repeat(256), &"\u{0}".repeat(512));
    large.source = source(&large.memory);
    let snapshot = prepared.capture(large.input()).unwrap();
    assert_eq!(
        projection.render(&snapshot, 1).err(),
        Some(EpisodeRetrievalProjectionRefusal::ByteBound)
    );
    assert!(projection.output().is_empty());
    let snapshot = prepared.capture(good.input()).unwrap();
    assert!(!projection.render(&snapshot, 1).unwrap().is_empty());
}
#[test]
fn rendering_actual_raw_evidence_allocates_nothing() {
    let fixture = fixture(Some(
        r#"{"evidence":[{"sources":[{"human_statement":"report/one"}],"observed_at":null}]}"#,
    ));
    let prepared = PreparedEpisodeRetrievalSnapshot::prepare();
    let snapshot = prepared.capture(fixture.input()).unwrap();
    let mut projection = PreparedEpisodeRetrievalProjection::prepare();
    let (result, allocation) =
        allocation_probe::observe(|| projection.render(&snapshot, 1).map(|_| ()));
    assert!(result.is_ok());
    assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
}

#[test]
fn selected_membership_is_bounded_to_unique_original_ordinals_and_refuses_atomically() {
    let fixture = fixture(Some(
        r#"{"evidence":[{"identity":"first"},{"identity":"second"}]}"#,
    ));
    let snapshot = PreparedEpisodeRetrievalSnapshot::prepare()
        .capture(fixture.input())
        .unwrap();
    let mut projection = PreparedEpisodeRetrievalProjection::prepare();
    let (result, allocation) =
        allocation_probe::observe(|| projection.render_selected(&snapshot, 1, &[1]).map(|_| ()));
    assert!(result.is_ok());
    assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
    let parsed: serde_json::Value = serde_json::from_slice(projection.output()).unwrap();
    assert_eq!(
        parsed["temporal_selection"]["item_indices"],
        serde_json::json!([1])
    );
    for items in [&[][..], &[0, 0], &[2]] {
        assert_eq!(
            projection.render_selected(&snapshot, 1, items).err(),
            Some(EpisodeRetrievalProjectionRefusal::InvalidSelection)
        );
        assert!(projection.output().is_empty());
    }
    assert_eq!(
        projection.render_selected(&snapshot, 2, &[0]).err(),
        Some(EpisodeRetrievalProjectionRefusal::InvalidSelection)
    );
    assert!(projection.output().is_empty());
}

#[test]
fn selected_projection_contains_only_complete_original_objects_in_selected_order() {
    let first = r#"{ "identity":"first", "utf8_content":"selected first", "unknown": {"nested":[1,{"evidence":["not root"]}]} }"#;
    let omitted = r#"{"identity":"future","utf8_content":"UNSELECTED_FUTURE_SECRET","sources":[{"clock":"other/clock"}]}"#;
    let third = r#"{ "identity":"third", "utf8_content":"{\"voltage\":13,\"unit\":\"V\",\"uncertainty\":0.1,\"calibration\":\"meter/2\"}\n\u03bb\ud83d\ude42", "sources": [ {"sign":"source/3", "future_source_field":{"raw":"\\u1234"}} ], "future_item_field":{"a": [true, null]} }"#;
    let raw = format!(
        r#"{{"nested":{{"evidence":[{{"utf8_content":"NOT_ROOT"}}]}},"extra\ud83d\ude42":true,"revision":7,"provenance_revision":"r/\u03bb","evid\u0065nce": [ {first}, {omitted}, {third} ],"root_private":"ROOT_SECRET"}}"#
    );
    let fixture = fixture(Some(&raw));
    let snapshot = PreparedEpisodeRetrievalSnapshot::prepare()
        .capture(fixture.input())
        .unwrap();
    let mut projection = PreparedEpisodeRetrievalProjection::prepare();
    let (result, allocation) = allocation_probe::observe(|| {
        projection
            .render_selected(&snapshot, 1, &[2, 0])
            .map(|_| ())
    });
    assert!(result.is_ok());
    assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
    let output = projection.output();
    assert!(output
        .windows(first.len())
        .any(|bytes| bytes == first.as_bytes()));
    assert!(output
        .windows(third.len())
        .any(|bytes| bytes == third.as_bytes()));
    let text = std::str::from_utf8(output).unwrap();
    for private in [
        "UNSELECTED_FUTURE_SECRET",
        "ROOT_SECRET",
        "NOT_ROOT",
        "other/clock",
    ] {
        assert!(!text.contains(private), "exposed {private}");
    }
    let parsed: serde_json::Value = serde_json::from_slice(output).unwrap();
    assert_eq!(parsed["archive_scope"], "selected_complete_items_only");
    assert_eq!(
        parsed["temporal_selection"]["item_indices"],
        serde_json::json!([2, 0])
    );
    let evidence = &parsed["historical_evidence"];
    assert_eq!(evidence["evidence"].as_array().unwrap().len(), 2);
    assert_eq!(evidence["evidence"][0]["identity"], "third");
    assert_eq!(evidence["evidence"][1]["identity"], "first");
    assert_eq!(evidence["original_provenance"]["revision"], 7);
    assert_eq!(
        evidence["original_provenance"]["provenance_revision"],
        "r/λ"
    );
    assert!(evidence.get("root_private").is_none());
    // Generic rendering still exposes the unchanged full archive by explicit request.
    let archive = projection.render(&snapshot, 1).unwrap();
    assert!(archive
        .windows(raw.len())
        .any(|bytes| bytes == raw.as_bytes()));
}

#[test]
fn selected_projection_pressure_and_malformed_input_clear_prior_output() {
    let raw = format!(
        r#"{{"evidence":[{{"identity":"large","unknown":"{}"}}]}}"#,
        "x".repeat(3800)
    );
    let large = fixture(Some(&raw));
    let good = fixture(Some(r#"{"evidence":[{"identity":"small"}]}"#));
    let prepared = PreparedEpisodeRetrievalSnapshot::prepare();
    let snapshot = prepared.capture(good.input()).unwrap();
    let mut projection = PreparedEpisodeRetrievalProjection::prepare();
    projection.render_selected(&snapshot, 1, &[0]).unwrap();
    let snapshot = prepared.capture(large.input()).unwrap();
    assert_eq!(
        projection.render_selected(&snapshot, 1, &[0]).err(),
        Some(EpisodeRetrievalProjectionRefusal::ByteBound)
    );
    assert!(projection.output().is_empty());
    let malformed = fixture(Some(r#"{"evidence":[{"identity":"x"}],"revision":-1}"#));
    let snapshot = prepared.capture(malformed.input()).unwrap();
    assert_eq!(
        projection.render_selected(&snapshot, 1, &[0]).err(),
        Some(EpisodeRetrievalProjectionRefusal::InvalidEvidenceJson)
    );
    assert!(projection.output().is_empty());
}
