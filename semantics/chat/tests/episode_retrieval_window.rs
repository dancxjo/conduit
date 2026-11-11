#![cfg(feature = "plot-catalog")]
extern crate alloc;
use conduit_chat::*;
mod common_episode_retrieval;
use common_episode_retrieval::*;
use conduit_ai::*;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
#[allow(dead_code)]
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn evidence(ticks: u64, scale: &str, clock: &str, uncertainty: u64) -> String {
    serde_json::json!({"revision":1,"evidence":[{"identity":"actual/report","origin":"human_reported","utf8_content":"A real retained statement","sources":[{"kind":"sign","identity":"sign/original"}],"observed_at":{"ticks":ticks,"scale":scale,"clock_basis":clock,"resolution_ticks":1,"uncertainty_ticks":uncertainty}}]}).to_string()
}
fn fixture(texts: &[String]) -> Fixture {
    let mut f = Fixture::new();
    let rows: Vec<_> = texts
        .iter()
        .enumerate()
        .map(|(i, s)| ((i + 1) as u64, Some(s.as_str())))
        .collect();
    f.memory = memory_with_evidence(&rows, "body/one", "wake/one");
    f.source = source(&f.memory);
    f
}
#[test]
fn three_competing_episodes_choose_actual_source_time_not_latest_episode() {
    let mut f = fixture(&[
        evidence(10, "milliseconds", "clock/one", 0),
        evidence(70, "milliseconds", "clock/one", 0),
        evidence(30, "milliseconds", "clock/one", 0),
    ]);
    f.intent = TemporalRetrievalIntent::LatestEvidence.encode().unwrap();
    let capture = PreparedEpisodeRetrievalSnapshot::prepare();
    let snapshot = capture.capture(f.input()).unwrap();
    assert_eq!(snapshot.record_count(), 3);
    let mut window = PreparedEpisodeRetrievalWindow::prepare();
    let mut selector = BoundedTemporalEvidenceFlow::prepare();
    let ((), allocations) = allocation_probe::observe(|| {
        let bytes = window.build(&snapshot).unwrap();
        let outcome = selector.execute(bytes, &f.intent).unwrap();
        assert_eq!(
            window.resolve(&snapshot, outcome).unwrap(),
            &[EpisodeRetrievalCandidateKey {
                episode: 2,
                item: 0
            }]
        );
    });
    assert_eq!((allocations.allocations, allocations.reallocations), (0, 0));
    assert_eq!(window.candidates().len(), 3);
    assert_eq!(window.omissions().count(), 0);
    assert_eq!(
        window.resolution(),
        Some(EpisodeRetrievalResolution::Selected)
    );
    let record = snapshot.original_record(2).unwrap();
    assert!(record
        .record_field("outcome")
        .unwrap()
        .unwrap()
        .variant_payload("provider_lost")
        .unwrap()
        .is_some());
}
#[test]
fn missing_and_incomparable_time_are_explicit_omissions_without_fabrication() {
    let missing = r#"{"evidence":[{"identity":"unknown","observed_at":null}]}"#.into();
    let f = fixture(&[
        missing,
        evidence(20, "seconds", "clock/one", 0),
        evidence(30, "milliseconds", "other-clock", 0),
        evidence(40, "milliseconds", "clock/one", 0),
    ]);
    let snapshot = PreparedEpisodeRetrievalSnapshot::prepare()
        .capture(f.input())
        .unwrap();
    let mut window = PreparedEpisodeRetrievalWindow::prepare();
    let bytes = window.build(&snapshot).unwrap();
    let decoded = TemporalEvidenceWindow::decode(bytes).unwrap();
    assert!(!decoded.earliest_history_complete());
    assert_eq!(decoded.candidates().get().len(), 1);
    let reasons: Vec<_> = window.omissions().map(|o| o.reason).collect();
    assert_eq!(
        reasons,
        vec![
            EpisodeTemporalOmissionReason::MissingObservedTime,
            EpisodeTemporalOmissionReason::UnsupportedTime,
            EpisodeTemporalOmissionReason::ClockMismatch
        ]
    );
}
#[test]
fn malformed_intervals_and_future_sources_are_not_relabelled_as_past() {
    let f = fixture(&[
        evidence(5, "milliseconds", "clock/one", 10),
        evidence(200, "milliseconds", "clock/one", 0),
    ]);
    let snapshot = PreparedEpisodeRetrievalSnapshot::prepare()
        .capture(f.input())
        .unwrap();
    let mut window = PreparedEpisodeRetrievalWindow::prepare();
    window.build(&snapshot).unwrap();
    assert!(window.candidates().is_empty());
    assert_eq!(
        window.omissions().map(|o| o.reason).collect::<Vec<_>>(),
        vec![
            EpisodeTemporalOmissionReason::InvalidInterval,
            EpisodeTemporalOmissionReason::FutureSource
        ]
    );
}
#[test]
fn outcome_cannot_select_a_record_from_another_snapshot_or_invent_an_identity() {
    let mut f = fixture(&[evidence(10, "milliseconds", "clock/one", 0)]);
    let capture = PreparedEpisodeRetrievalSnapshot::prepare();
    let mut window = PreparedEpisodeRetrievalWindow::prepare();
    let snapshot = capture.capture(f.input()).unwrap();
    window.build(&snapshot).unwrap();
    let native = TemporalEvidenceWindow::decode(window.output()).unwrap();
    let id = native.identity().clone();
    let result = |ids: &[&str]| {
        TemporalEvidenceOutcome::new(
            TemporalEvidenceFlowSelection::selected(
                TemporalEvidenceIdentities::new(
                    BoundedSequence::try_from_iter(
                        ids.iter()
                            .map(|id| TemporalEvidenceIdentity::new((*id).into()).unwrap()),
                    )
                    .unwrap(),
                )
                .unwrap(),
            )
            .unwrap(),
            id.clone(),
        )
        .unwrap()
        .encode()
        .unwrap()
    };
    assert_eq!(
        window.resolve(&snapshot, &result(&["episode/99/evidence/0"])),
        Err(EpisodeRetrievalWindowRefusal::UnknownSelection)
    );
    assert_eq!(
        window.resolve(
            &snapshot,
            &result(&["episode/1/evidence/0", "episode/1/evidence/0"])
        ),
        Err(EpisodeRetrievalWindowRefusal::DuplicateSelection)
    );
    let valid = result(&["episode/1/evidence/0"]);
    f.intent = TemporalRetrievalIntent::LatestEvidence.encode().unwrap();
    let changed = capture.capture(f.input()).unwrap();
    assert_eq!(
        window.resolve(&changed, &valid),
        Err(EpisodeRetrievalWindowRefusal::Snapshot(
            EpisodeRetrievalSnapshotRefusal::SnapshotMismatch
        ))
    );
    assert_eq!(window.resolution(), None);
}
#[test]
fn history_status_and_intent_changes_are_verified_against_the_bound_capture() {
    let mut f = fixture(&[
        evidence(10, "milliseconds", "clock/one", 0),
        evidence(30, "milliseconds", "clock/one", 0),
    ]);
    let capture = PreparedEpisodeRetrievalSnapshot::prepare();
    let mut window = PreparedEpisodeRetrievalWindow::prepare();
    let mut selector = BoundedTemporalEvidenceFlow::prepare();
    {
        let snapshot = capture.capture(f.input()).unwrap();
        let bytes = window.build(&snapshot).unwrap();
        let changed_intent = TemporalRetrievalIntent::LatestEvidence.encode().unwrap();
        let outcome = selector.execute(bytes, &changed_intent).unwrap();
        assert_eq!(
            window.resolve(&snapshot, outcome),
            Err(EpisodeRetrievalWindowRefusal::SelectionMismatch)
        );
        assert_eq!(window.resolution(), None);
        let decoded = TemporalEvidenceWindow::decode(window.output()).unwrap();
        let false_empty = TemporalEvidenceOutcome::new(
            TemporalEvidenceFlowSelection::NoHistory,
            decoded.identity().clone(),
        )
        .unwrap()
        .encode()
        .unwrap();
        assert_eq!(
            window.resolve(&snapshot, &false_empty),
            Err(EpisodeRetrievalWindowRefusal::SelectionMismatch)
        );
    }
    f.intent = TemporalRetrievalIntent::EarliestEvidence.encode().unwrap();
    let snapshot = capture.capture(f.input()).unwrap();
    let bytes = window.build(&snapshot).unwrap();
    let outcome = selector.execute(bytes, &f.intent).unwrap();
    assert!(window.resolve(&snapshot, outcome).unwrap().is_empty());
    assert_eq!(
        window.resolution(),
        Some(EpisodeRetrievalResolution::NeedEarlierHistory)
    );
}
#[test]
fn unavailable_record_evidence_does_not_acquire_a_timestamp() {
    let f = Fixture::new();
    let snapshot = PreparedEpisodeRetrievalSnapshot::prepare()
        .capture(f.input())
        .unwrap();
    let mut window = PreparedEpisodeRetrievalWindow::prepare();
    let mut selector = BoundedTemporalEvidenceFlow::prepare();
    let bytes = window.build(&snapshot).unwrap();
    let outcome = selector.execute(bytes, &f.intent).unwrap();
    assert!(window.resolve(&snapshot, outcome).unwrap().is_empty());
    assert_eq!(
        window.resolution(),
        Some(EpisodeRetrievalResolution::NoHistory)
    );
    assert_eq!(
        window.omissions().map(|o| o.reason).collect::<Vec<_>>(),
        vec![EpisodeTemporalOmissionReason::MissingEvidence; 2]
    );
}
