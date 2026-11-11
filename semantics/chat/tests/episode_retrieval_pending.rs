#![cfg(feature = "plot-catalog")]
use conduit_ai::*;
use conduit_chat::*;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
mod common_episode_retrieval;
use common_episode_retrieval::*;
use std::cell::Cell;
#[allow(dead_code)]
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
const CURRENT: &[u8] = br#"{"evidence":[]}"#;
// Fenced empty-current fixture: verify the actual packet contains no current
// evidence before rendering an empty array; history is never supplied here.
fn current(packet: &[u8], scratch: &mut [u8]) -> Result<usize, EpisodeCurrentProjectionRefusal> {
    let packet = conduit_core::validate_canonical_structured_value(packet)
        .map_err(|_| EpisodeCurrentProjectionRefusal::Malformed)?;
    let evidence = packet
        .record_field("evidence")
        .unwrap()
        .unwrap()
        .record_field("evidence")
        .unwrap()
        .unwrap();
    if evidence.collection_index(0).unwrap().is_some() {
        return Err(EpisodeCurrentProjectionRefusal::Unsupported);
    }
    scratch[..CURRENT.len()].copy_from_slice(CURRENT);
    Ok(CURRENT.len())
}
fn policy(bytes: u32) -> ContextSelectionPolicy {
    ContextSelectionPolicy::new(
        "policy/pending@1".into(),
        EPISODE_CONTEXT_BYTE_CHARGE_PROFILE.into(),
        ContextRedundancyPolicy::OnePerReviewedGroup,
        ContextOrderingPolicy::ChronologicalOldestFirst,
        4,
        bytes,
        2400,
        4,
    )
    .unwrap()
}
fn fixture(bytes: u32) -> Fixture {
    let mut f = Fixture::new();
    let evidence = |ticks| {
        serde_json::json!({"evidence":[{"identity":"report/one","origin":"human_reported","sources":[{"human_statement":"source/original"}],"observed_at":{"ticks":ticks,"scale":"milliseconds","clock_basis":"clock/one","resolution_ticks":1,"uncertainty_ticks":0}}]}).to_string()
    };
    let first = evidence(20);
    let second = evidence(10);
    f.memory = memory_with_evidence(
        &[(1, Some(&first)), (2, Some(&second))],
        "body/one",
        "wake/one",
    );
    f.source = source(&f.memory);
    f.policy = policy(bytes).encode().unwrap();
    f
}
fn select(
    snapshot: &BorrowedEpisodeRetrievalSnapshot<'_>,
    f: &Fixture,
    bytes: u32,
) -> PreparedEpisodeRetrievalContext {
    let mut window = PreparedEpisodeRetrievalWindow::prepare();
    let mut flow = BoundedTemporalEvidenceFlow::prepare();
    let outcome = flow
        .execute(window.build(snapshot).unwrap(), &f.intent)
        .unwrap();
    let mut context = PreparedEpisodeRetrievalContext::prepare(policy(bytes)).unwrap();
    context.select(snapshot, outcome).unwrap();
    context
}
#[test]
fn successful_compose_pairs_native_pending_with_exact_transmitted_fragments_without_allocation() {
    let f = fixture(2400);
    let snapshot = PreparedEpisodeRetrievalSnapshot::prepare()
        .capture(f.input())
        .unwrap();
    let context = select(&snapshot, &f, 2400);
    assert_eq!(context.selected_episodes().collect::<Vec<_>>(), vec![2, 1]);
    let mut request =
        PreparedEpisodeRetrievalRequest::prepare("Answer from evidence", current).unwrap();
    let (result, allocations) =
        allocation_probe::observe(|| request.compose(&snapshot, &context).map(|_| ()));
    result.unwrap();
    assert_eq!((allocations.allocations, allocations.reallocations), (0, 0));
    let pending = EpisodePending::decode(request.pending()).unwrap();
    assert_eq!(pending.body_id(), "body/one");
    assert_eq!(pending.wake_id(), "wake/one");
    assert_eq!(*pending.wake_sequence(), 3);
    assert_eq!(*pending.episode(), 3);
    assert_eq!(
        pending.evidence().as_deref(),
        Some(core::str::from_utf8(CURRENT).unwrap())
    );
    let experience = EpisodeExperience::decode(&f.experience).unwrap();
    assert_eq!(pending.observation(), experience.observation());
    let original = EpisodeMemoryState::decode(&f.memory).unwrap();
    for ((retained, fragment), episode) in pending
        .recalled()
        .get()
        .iter()
        .zip(context.selected_fragments())
        .zip(context.selected_episodes())
    {
        assert_eq!(*retained.episode(), episode);
        assert_eq!(
            retained.evidence().as_deref(),
            Some(core::str::from_utf8(fragment).unwrap())
        );
        let original = original
            .memories()
            .get()
            .iter()
            .find(|memory| *memory.episode() == episode)
            .unwrap();
        assert_eq!(retained.observation(), original.observation());
        assert_eq!(retained.outcome(), original.outcome());
    }
    assert!(matches!(
        pending.recalled().get().as_slice()[0].outcome(),
        EpisodeOutcome::ProviderLost(_)
    ));
    assert!(matches!(
        pending.recalled().get().as_slice()[1].outcome(),
        EpisodeOutcome::Produced(_)
    ));
}
#[test]
fn all_omitted_context_yields_native_current_only_pending() {
    let f = fixture(1);
    let snapshot = PreparedEpisodeRetrievalSnapshot::prepare()
        .capture(f.input())
        .unwrap();
    let context = select(&snapshot, &f, 1);
    let mut request = PreparedEpisodeRetrievalRequest::prepare("Answer", current).unwrap();
    request.compose(&snapshot, &context).unwrap();
    let pending = EpisodePending::decode(request.pending()).unwrap();
    assert!(pending.recalled().get().is_empty());
    assert!(pending.evidence().is_some());
}
#[test]
fn callback_stale_snapshot_and_episode_overflow_refusals_clear_both_outputs() {
    let f = fixture(2400);
    let capture = PreparedEpisodeRetrievalSnapshot::prepare();
    let snapshot = capture.capture(f.input()).unwrap();
    let context = select(&snapshot, &f, 2400);
    let fail = Cell::new(false);
    let mut request =
        PreparedEpisodeRetrievalRequest::prepare("Answer", |packet: &[u8], scratch: &mut [u8]| {
            if fail.get() {
                Err(EpisodeCurrentProjectionRefusal::Unsupported)
            } else {
                current(packet, scratch)
            }
        })
        .unwrap();
    request.compose(&snapshot, &context).unwrap();
    assert!(!request.pending().is_empty());
    fail.set(true);
    assert!(request.compose(&snapshot, &context).is_err());
    assert!(request.output().is_empty());
    assert!(request.pending().is_empty());
    fail.set(false);
    request.compose(&snapshot, &context).unwrap();
    let mut changed = f.input();
    changed.reference.ticks += 1;
    let changed = capture.capture(changed).unwrap();
    assert!(request.compose(&changed, &context).is_err());
    assert!(request.output().is_empty());
    assert!(request.pending().is_empty());
    let mut overflow = fixture(2400);
    overflow.memory = EpisodeMemoryState::new(
        u64::MAX,
        EpisodeRecall::new(BoundedSequence::new()).unwrap(),
        EpisodeMemoryOwner::bound("body/one".into(), "wake/one".into(), 3).unwrap(),
    )
    .unwrap()
    .encode()
    .unwrap();
    overflow.source = source(&overflow.memory);
    let snapshot = capture.capture(overflow.input()).unwrap();
    let context = select(&snapshot, &overflow, 2400);
    assert_eq!(
        request.compose(&snapshot, &context).err(),
        Some(EpisodeRetrievalRequestRefusal::MalformedPending)
    );
    assert!(request.output().is_empty());
    assert!(request.pending().is_empty());
}
