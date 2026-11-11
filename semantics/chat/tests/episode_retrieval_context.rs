#![cfg(feature = "plot-catalog")]
extern crate alloc;
use conduit_ai::*;
use conduit_chat::*;
use conduit_plot::rust_binding::NativeRustBinding;
mod common_episode_retrieval;
use common_episode_retrieval::*;
#[allow(dead_code)]
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn evidence(times: &[u64]) -> String {
    serde_json::json!({"revision":1,"evidence":times.iter().enumerate().map(|(i,time)|serde_json::json!({"identity":format!("evidence/{i}"),"origin":"human_reported","utf8_content":"{\"voltage\":13,\"unit\":\"V\",\"uncertainty\":0.1}","sources":[{"kind":"sign","identity":"actual/source"}],"observed_at":{"ticks":time,"scale":"milliseconds","clock_basis":"clock/one","resolution_ticks":1,"uncertainty_ticks":0}})).collect::<Vec<_>>()}).to_string()
}
fn policy(items: u16, bytes: u32) -> ContextSelectionPolicy {
    ContextSelectionPolicy::new(
        "context/episode-history@1".into(),
        EPISODE_CONTEXT_BYTE_CHARGE_PROFILE.into(),
        ContextRedundancyPolicy::OnePerReviewedGroup,
        ContextOrderingPolicy::ChronologicalOldestFirst,
        items,
        bytes,
        4096,
        4,
    )
    .unwrap()
}
fn fixture(texts: &[String], policy: &ContextSelectionPolicy) -> Fixture {
    let mut f = Fixture::new();
    let rows: Vec<_> = texts
        .iter()
        .enumerate()
        .map(|(i, text)| ((i + 1) as u64, Some(text.as_str())))
        .collect();
    f.memory = memory_with_evidence(&rows, "body/one", "wake/one");
    f.source = source(&f.memory);
    f.policy = policy.clone().encode().unwrap();
    f
}
#[test]
fn actual_context_selection_preserves_chronology_costs_and_outcome_roles_without_allocating() {
    let policy = policy(2, 4096);
    let f = fixture(
        &[evidence(&[10]), evidence(&[30]), evidence(&[20])],
        &policy,
    );
    let capture = PreparedEpisodeRetrievalSnapshot::prepare();
    let snapshot = capture.capture(f.input()).unwrap();
    let mut window = PreparedEpisodeRetrievalWindow::prepare();
    let mut temporal = BoundedTemporalEvidenceFlow::prepare();
    let output = temporal
        .execute(window.build(&snapshot).unwrap(), &f.intent)
        .unwrap();
    let mut context = PreparedEpisodeRetrievalContext::prepare(policy).unwrap();
    let (result, allocation) = allocation_probe::observe(|| context.select(&snapshot, output));
    let selected = result.unwrap().unwrap();
    assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
    assert_eq!(context.selected_episodes().collect::<Vec<_>>(), vec![1, 3]);
    assert_eq!(selected.used.work_units, 2);
    assert_eq!(
        selected.used.bytes,
        context
            .selected_fragments()
            .map(|bytes| bytes.len() as u32)
            .sum::<u32>()
    );
    assert_eq!(selected.used.tokens, selected.used.bytes);
    assert_eq!(
        context.omissions().collect::<Vec<_>>(),
        vec![(2, ContextOmissionReason::ItemBudget)]
    );
    let values: Vec<serde_json::Value> = context
        .selected_fragments()
        .map(|bytes| serde_json::from_slice(bytes).unwrap())
        .collect();
    assert_eq!(values[0]["origin"], "remembered");
    assert_eq!(values[0]["actual_outcome"]["provenance"], "model_derived");
    assert_eq!(values[1]["actual_outcome"]["disposition"], "provider_lost");
    assert!(values[1]["actual_outcome"].get("content").is_none());
    assert_eq!(
        values[0]["historical_evidence"]["evidence"][0]["sources"][0]["identity"],
        "actual/source"
    );
}
#[test]
fn multiple_selected_items_group_only_the_same_original_episode() {
    let policy = policy(4, 4096);
    let f = fixture(&[evidence(&[10, 15]), evidence(&[20])], &policy);
    let snapshot = PreparedEpisodeRetrievalSnapshot::prepare()
        .capture(f.input())
        .unwrap();
    let mut window = PreparedEpisodeRetrievalWindow::prepare();
    let mut temporal = BoundedTemporalEvidenceFlow::prepare();
    let output = temporal
        .execute(window.build(&snapshot).unwrap(), &f.intent)
        .unwrap();
    let mut context = PreparedEpisodeRetrievalContext::prepare(policy).unwrap();
    assert_eq!(
        context
            .select(&snapshot, output)
            .unwrap()
            .unwrap()
            .selected_len,
        2
    );
    assert_eq!(context.selected_episodes().collect::<Vec<_>>(), vec![1, 2]);
    assert_eq!(context.selected_fragments().count(), 2);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(context.selected_fragments().next().unwrap())
            .unwrap()["historical_evidence"]["evidence"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}
#[test]
fn policy_mismatch_bad_results_and_insufficient_budget_clear_prior_selection() {
    let policy = policy(2, 4096);
    let f = fixture(&[evidence(&[10]), evidence(&[20])], &policy);
    let snapshot = PreparedEpisodeRetrievalSnapshot::prepare()
        .capture(f.input())
        .unwrap();
    let mut window = PreparedEpisodeRetrievalWindow::prepare();
    let mut temporal = BoundedTemporalEvidenceFlow::prepare();
    let output = temporal
        .execute(window.build(&snapshot).unwrap(), &f.intent)
        .unwrap();
    let mut context = PreparedEpisodeRetrievalContext::prepare(policy).unwrap();
    context.select(&snapshot, output).unwrap();
    assert!(context.select(&snapshot, b"invalid").is_err());
    assert_eq!(context.selected_fragments().count(), 0);
    assert!(context.selection().is_none());
    let wrong = ContextSelectionPolicy::new(
        "different".into(),
        EPISODE_CONTEXT_BYTE_CHARGE_PROFILE.into(),
        ContextRedundancyPolicy::KeepAll,
        ContextOrderingPolicy::Reranked,
        1,
        4096,
        4096,
        4,
    )
    .unwrap();
    let mut mismatch = PreparedEpisodeRetrievalContext::prepare(wrong).unwrap();
    assert_eq!(
        mismatch.select(&snapshot, output),
        Err(EpisodeRetrievalContextRefusal::PolicyMismatch)
    );
    let tiny = crate::policy(2, 1);
    let small = fixture(&[evidence(&[10])], &tiny);
    let snapshot = PreparedEpisodeRetrievalSnapshot::prepare()
        .capture(small.input())
        .unwrap();
    let output = temporal
        .execute(window.build(&snapshot).unwrap(), &small.intent)
        .unwrap();
    let mut bounded = PreparedEpisodeRetrievalContext::prepare(tiny).unwrap();
    let selected = bounded.select(&snapshot, output).unwrap().unwrap();
    assert_eq!(selected.selected_len, 0);
    assert_eq!(selected.omitted_len, 1);
    assert_eq!(
        bounded.selected_snapshot_identity(),
        Some(snapshot.identity())
    );
    assert_eq!(
        bounded.omissions().collect::<Vec<_>>(),
        vec![(1, ContextOmissionReason::ByteBudget)]
    );
    assert_eq!(bounded.selected_fragments().count(), 0);
}
#[test]
fn no_temporal_selection_keeps_the_scoped_reason_and_omissions() {
    let policy = policy(2, 4096);
    let mut f = Fixture::new();
    f.policy = policy.clone().encode().unwrap();
    let snapshot = PreparedEpisodeRetrievalSnapshot::prepare()
        .capture(f.input())
        .unwrap();
    let mut window = PreparedEpisodeRetrievalWindow::prepare();
    let mut temporal = BoundedTemporalEvidenceFlow::prepare();
    let output = temporal
        .execute(window.build(&snapshot).unwrap(), &f.intent)
        .unwrap();
    let mut context = PreparedEpisodeRetrievalContext::prepare(policy).unwrap();
    assert_eq!(context.select(&snapshot, output).unwrap(), None);
    assert_eq!(
        context.temporal_resolution(),
        Some(EpisodeRetrievalResolution::NoHistory)
    );
    assert_eq!(context.temporal_omissions().count(), 2);
    assert_eq!(snapshot.record_count(), 2);
}

#[test]
fn only_selected_complete_items_reach_the_model_and_the_archive_stays_local() {
    let policy = policy(4, 4096);
    let mut f = fixture(&[evidence(&[10, 120, 20])], &policy);
    f.intent = TemporalRetrievalIntent::LatestEvidence.encode().unwrap();
    let capture = PreparedEpisodeRetrievalSnapshot::prepare();
    let snapshot = capture.capture(f.input()).unwrap();
    let mut window = PreparedEpisodeRetrievalWindow::prepare();
    let mut temporal = BoundedTemporalEvidenceFlow::prepare();
    let output = temporal
        .execute(window.build(&snapshot).unwrap(), &f.intent)
        .unwrap();
    let mut context = PreparedEpisodeRetrievalContext::prepare(policy).unwrap();
    context.select(&snapshot, output).unwrap();
    let fragment = context.selected_fragments().next().unwrap().to_vec();
    let value: serde_json::Value = serde_json::from_slice(&fragment).unwrap();
    assert_eq!(
        value["temporal_selection"]["item_indices"],
        serde_json::json!([2])
    );
    assert_eq!(value["archive_scope"], "selected_complete_items_only");
    assert_eq!(
        value["historical_evidence"]["evidence"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        value["historical_evidence"]["evidence"][0]["observed_at"]["ticks"],
        20
    );
    // Original bytes retain all three items in the local immutable capture.
    assert_eq!(snapshot.record_count(), 1);
    assert_eq!(snapshot.input().memory, f.memory);
    assert_eq!(
        context.temporal_omissions().collect::<Vec<_>>(),
        vec![EpisodeTemporalOmission {
            episode: 1,
            item: Some(1),
            reason: EpisodeTemporalOmissionReason::FutureSource,
        }]
    );
    let identity = context.selected_chunk_identities().next().unwrap();
    // Same exact source record, different selection parameters: both rendered
    // membership and its transform-bound chunk identity must change.
    f.intent = TemporalRetrievalIntent::EventOrdering.encode().unwrap();
    let snapshot = capture.capture(f.input()).unwrap();
    let output = temporal
        .execute(window.build(&snapshot).unwrap(), &f.intent)
        .unwrap();
    context.select(&snapshot, output).unwrap();
    let value: serde_json::Value =
        serde_json::from_slice(context.selected_fragments().next().unwrap()).unwrap();
    assert_eq!(
        value["temporal_selection"]["item_indices"],
        serde_json::json!([0, 2])
    );
    assert_ne!(
        identity,
        context.selected_chunk_identities().next().unwrap()
    );
    assert_ne!(fragment, context.selected_fragments().next().unwrap());
}
