#![cfg(feature = "plot-catalog")]
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
fn policy() -> ContextSelectionPolicy {
    ContextSelectionPolicy::new(
        "context/shared-terms@1".into(),
        EPISODE_CONTEXT_BYTE_CHARGE_PROFILE.into(),
        ContextRedundancyPolicy::KeepAll,
        ContextOrderingPolicy::Reranked,
        1,
        2048,
        2048,
        64,
    )
    .unwrap()
}
fn set_query(f: &mut Fixture, query: &str) {
    let old = EpisodeExperience::decode(&f.experience).unwrap();
    f.experience = EpisodeExperience::new(
        old.evidence().clone(),
        EpisodeObservation::new(query.into(), "query/one".into(), false).unwrap(),
        old.reference().clone(),
    )
    .unwrap()
    .encode()
    .unwrap();
}
fn document(texts: &[&str]) -> String {
    serde_json::json!({"revision":1,"evidence":texts.iter().enumerate().map(|(i,text)|serde_json::json!({"identity":format!("item/{i}"),"origin":"human_reported","utf8_content":text,"sources":[{"sign":"actual/report"}],"observed_at":{"clock_basis":"clock/one","scale":"milliseconds","ticks":10,"resolution_ticks":1,"uncertainty_ticks":0}})).collect::<Vec<_>>()} ).to_string()
}
fn fixture() -> Fixture {
    let mut f = Fixture::new();
    let first = document(&[
        "UNSELECTED VOLTAGE SECRET",
        "The hallway label is COPPER KITE 🙂.",
    ])
    .replace("COPPER", "C\\u004fPPER")
    .replace('🙂', "\\ud83d\\ude42");
    let question = document(&["What was the hallway label?"]);
    let unrelated = document(&["The garden gate is closed."]);
    f.memory = memory_with_evidence(
        &[
            (1, Some(&first)),
            (2, Some(&question)),
            (3, Some(&unrelated)),
        ],
        "body/one",
        "wake/one",
    );
    f.source = source(&f.memory);
    f.policy = policy().encode().unwrap();
    set_query(&mut f, "What was the hallway label?");
    f
}
fn current(_: &[u8], output: &mut [u8]) -> Result<usize, EpisodeCurrentProjectionRefusal> {
    output[..2].copy_from_slice(b"{}");
    Ok(2)
}
#[test]
fn shared_terms_select_the_original_fact_not_a_followup_question_without_allocation() {
    let f = fixture();
    let snapshot = PreparedEpisodeRetrievalSnapshot::prepare()
        .capture(f.input())
        .unwrap();
    let mut window = PreparedEpisodeRetrievalWindow::prepare();
    let mut temporal = BoundedTemporalEvidenceFlow::prepare();
    let outcome = temporal
        .execute(window.build(&snapshot).unwrap(), &f.intent)
        .unwrap();
    let mut context = PreparedEpisodeRetrievalContext::prepare_with_shared_terms(policy()).unwrap();
    let mut request =
        PreparedEpisodeRetrievalRequest::prepare("Read only selected evidence", current).unwrap();
    let (_, allocation) = allocation_probe::observe(|| {
        context.select(&snapshot, outcome).unwrap();
        request.compose(&snapshot, &context).unwrap();
    });
    assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
    assert_eq!(context.selected_episodes().collect::<Vec<_>>(), vec![1]);
    assert_eq!(context.irrelevant_episodes(), &[2, 3]);
    assert_eq!(context.relevance_omitted_items(), 3);
    let fragment = core::str::from_utf8(context.selected_fragments().next().unwrap()).unwrap();
    assert!(fragment.contains("C\\u004fPPER"));
    assert!(fragment.contains("\\ud83d\\ude42"));
    assert!(!fragment.contains("UNSELECTED VOLTAGE SECRET"));
    assert!(!fragment.contains("Earlier model words"));
    let value: serde_json::Value = serde_json::from_str(fragment).unwrap();
    assert_eq!(value["original_items"], serde_json::json!([1]));
    assert_eq!(
        value["historical_evidence"]["evidence"][0]["utf8_content"],
        "The hallway label is COPPER KITE 🙂."
    );
}
#[test]
fn a_changed_query_clears_prior_history_and_reports_actual_exclusions() {
    let mut f = fixture();
    let mut context = PreparedEpisodeRetrievalContext::prepare_with_shared_terms(policy()).unwrap();
    {
        let snapshot = PreparedEpisodeRetrievalSnapshot::prepare()
            .capture(f.input())
            .unwrap();
        let mut window = PreparedEpisodeRetrievalWindow::prepare();
        let mut temporal = BoundedTemporalEvidenceFlow::prepare();
        let outcome = temporal
            .execute(window.build(&snapshot).unwrap(), &f.intent)
            .unwrap();
        context.select(&snapshot, outcome).unwrap();
        assert_eq!(context.selected_episodes().collect::<Vec<_>>(), vec![1]);
    }
    set_query(&mut f, "Tell me about the library roof.");
    let snapshot = PreparedEpisodeRetrievalSnapshot::prepare()
        .capture(f.input())
        .unwrap();
    let mut window = PreparedEpisodeRetrievalWindow::prepare();
    let mut temporal = BoundedTemporalEvidenceFlow::prepare();
    let outcome = temporal
        .execute(window.build(&snapshot).unwrap(), &f.intent)
        .unwrap();

    assert!(context.select(&snapshot, outcome).unwrap().is_none());
    assert_eq!(context.selected_fragments().count(), 0);
    assert_eq!(context.irrelevant_episodes(), &[1, 2, 3]);
    assert_eq!(context.relevance_omitted_items(), 4);
    let mut request =
        PreparedEpisodeRetrievalRequest::prepare("Read selected evidence", current).unwrap();
    let data: serde_json::Value =
        serde_json::from_slice(request.compose(&snapshot, &context).unwrap()).unwrap();
    assert_eq!(data["data"]["recalled"], serde_json::json!([]));
    assert_eq!(
        data["data"]["retrieval"]["context"],
        "no_relevant_history_current_only"
    );
    assert_eq!(
        data["data"]["retrieval"]["irrelevant_episodes"],
        serde_json::json!([1, 2, 3])
    );
}
#[test]
fn outcome_queries_rank_typed_provider_loss_and_preserve_native_attempt_provenance() {
    let mut f = fixture();
    set_query(
        &mut f,
        "What was the earlier model provider outcome that was lost?",
    );
    let snapshot = PreparedEpisodeRetrievalSnapshot::prepare()
        .capture(f.input())
        .unwrap();
    let mut window = PreparedEpisodeRetrievalWindow::prepare();
    let mut temporal = BoundedTemporalEvidenceFlow::prepare();
    let outcome = temporal
        .execute(window.build(&snapshot).unwrap(), &f.intent)
        .unwrap();
    let mut context = PreparedEpisodeRetrievalContext::prepare_with_shared_terms(policy()).unwrap();
    context.select(&snapshot, outcome).unwrap();
    assert_eq!(context.selected_episodes().collect::<Vec<_>>(), vec![2]);
    let fragment: serde_json::Value =
        serde_json::from_slice(context.selected_fragments().next().unwrap()).unwrap();
    assert_eq!(fragment["actual_outcome"]["disposition"], "provider_lost");
    assert_eq!(fragment["actual_outcome"]["source"], "host");
    let mut request =
        PreparedEpisodeRetrievalRequest::prepare("Read selected evidence", current).unwrap();
    request.compose(&snapshot, &context).unwrap();
    let pending = EpisodePending::decode(request.pending()).unwrap();
    let EpisodeOutcome::ProviderLost(actual) = pending.recalled().get()[0].outcome() else {
        panic!("typed loss required")
    };
    assert!(!actual.attempt().is_empty());
    assert!(!actual.implementation().is_empty());
}
