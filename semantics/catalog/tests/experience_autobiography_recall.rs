#[allow(dead_code)] // Shared fixtures serve independent proof surfaces.
#[path = "common/experience_evidence.rs"]
mod common;
use common::*;
use conduit_human::*;
use conduit_plot::rust_binding::{BoundedBytes, BoundedSequence, NativeRustBinding};
use conduit_semantic_catalog::*;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};
struct CountingAllocator;
thread_local! { static COUNTING:Cell<bool>=const{Cell::new(false)}; static ALLOCATIONS:Cell<usize>=const{Cell::new(0)}; }
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        COUNTING.with(|f| {
            if f.get() {
                ALLOCATIONS.with(|n| n.set(n.get() + 1));
            }
        });
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, s: usize) -> *mut u8 {
        COUNTING.with(|f| {
            if f.get() {
                ALLOCATIONS.with(|n| n.set(n.get() + 1));
            }
        });
        unsafe { System.realloc(p, l, s) }
    }
}
const RESULT:&[u8]=br#"{"provenance":"ModelDerived","payload_kind":"llm/generated-result@1","payload":[72,105],"implementation_identity":"model/fixture","request_identity":"request/1","run_identity":"run/1","confidence":null,"disposition":"Produced","determinism":"DeterministicValidationFixture","accounting":{"context_items":1,"history_items":0,"input_bytes":100,"output_bytes":2,"work_units":2}}"#;
fn event(values: Vec<ExperienceEvidenceItem>, retain: bool) -> Vec<u8> {
    let ids: Vec<_> = values.iter().map(|v| v.identity().get().as_str()).collect();
    encode(ExperienceAutobiographyEvent::new(request(&ids, values.clone()), retain).unwrap())
}
fn state(records: Vec<(u64, ExperienceEvidenceItem)>) -> Vec<u8> {
    let completed = records.last().map(|v| v.0).unwrap_or(0);
    let records = records.into_iter().map(|(episode, item)| {
        let id = item.identity().get().clone();
        ExperienceAutobiographyRecord::new(
            episode,
            request(&[&id], vec![item]),
            ExperienceAutobiographyResponse::model_derived(BoundedBytes::new(RESULT).unwrap())
                .unwrap(),
        )
        .unwrap()
    });
    encode(
        ExperienceAutobiographyState::new(
            completed,
            ExperienceAutobiographyRecords::new(BoundedSequence::try_from_iter(records).unwrap())
                .unwrap(),
        )
        .unwrap(),
    )
}
fn replace_request(update: &[u8]) -> ExperienceEvidenceRequest {
    // Exercise the ordinary integration boundary rather than decode a private representation.
    let mut integration = BoundedExperienceIntegration::prepare();
    // Replay snapshot revision guards from zero until this explicit update is current.
    let view = conduit_core::validate_canonical_structured_value(update).unwrap();
    let bytes = view
        .record_field("expected_revision")
        .unwrap()
        .unwrap()
        .value_node();
    let expected = u64::from_le_bytes(bytes[5..].try_into().unwrap());
    let empty = ExperienceEvidenceItems::new(BoundedSequence::new()).unwrap();
    for revision in 0..expected {
        let prior = encode(
            ExperienceIntegrationUpdate::new(
                revision,
                ExperienceIntegrationOperation::replace_snapshot(empty.clone()).unwrap(),
                identity("initial"),
            )
            .unwrap(),
        );
        integration.execute(&prior).unwrap();
        integration.commit();
    }
    ExperienceEvidenceRequest::decode(integration.execute(update).unwrap()).unwrap()
}
#[test]
fn later_event_recalls_latest_matching_original_with_unpromoted_provenance() {
    let memory = state(vec![
        (1, observation("first")),
        (
            2,
            item(
                "later",
                ExperienceOrigin::ModelDerived,
                ExperienceTemporalRole::Current,
                ExperienceAvailability::Present,
                7,
                "provenance/7",
                b"model interpretation",
            ),
        ),
    ]);
    let current = event(vec![observation("now")], true);
    let mut recall = BoundedExperienceAutobiographyRecall::prepare();
    recall.execute(&current, &memory).unwrap();
    let request = replace_request(recall.update());
    let items = request.evidence().get().as_slice();
    assert_eq!(items.len(), 2);
    let remembered = &items[1];
    assert_eq!(remembered.origin(), &ExperienceOrigin::Remembered);
    assert_eq!(
        remembered.temporal_role(),
        &ExperienceTemporalRole::Historical
    );
    assert_eq!(remembered.domain(), &ExperienceDomain::Recollection);
    let wrapper = core::str::from_utf8(remembered.encoded_content().get().as_slice()).unwrap();
    assert!(wrapper.contains("\"original_origin\":\"model_derived\""));
    assert!(wrapper.contains("\"original_content_kind\":\"value/text\""));
    assert!(wrapper.contains("model interpretation"));
    assert_eq!(
        remembered.sources().get().as_slice()[0],
        ExperienceEvidenceSource::memory_record(identity("autobiography/episode/2")).unwrap()
    );
    assert_eq!(
        remembered.sources().get().as_slice()[1],
        observation("later").sources().get().as_slice()[0]
    );
    let pending = ExperienceAutobiographyPending::decode(recall.pending()).unwrap();
    assert_eq!(encode(pending.previous().clone()), memory);
    assert_eq!(*pending.expected_completed(), 2);
    assert!(*pending.retain());
    assert_eq!(pending.observation().evidence().get().as_slice().len(), 1);
}
#[test]
fn empty_and_nonmatching_memory_produce_only_current_items() {
    let mut recall = BoundedExperienceAutobiographyRecall::prepare();
    let current = event(vec![observation("now")], false);
    for memory in [
        encode(initial_experience_autobiography_state()),
        state(vec![(
            1,
            item(
                "different",
                ExperienceOrigin::Observation,
                ExperienceTemporalRole::Current,
                ExperienceAvailability::Present,
                7,
                "provenance/7",
                b"historical",
            ),
        )]),
    ] {
        // Same kind/domain does match; empty memory is checked separately below.
        recall.execute(&current, &memory).unwrap();
    }
    recall
        .execute(&current, &encode(initial_experience_autobiography_state()))
        .unwrap();
    assert_eq!(
        replace_request(recall.update())
            .evidence()
            .get()
            .as_slice()
            .len(),
        1
    );
    let different = ExperienceEvidenceItem::new(
        ExperienceAvailability::Present,
        ExperienceCertainty::Uncertain,
        identity("other/kind"),
        ExperienceDomain::Visual,
        observation("old").encoded_content().clone(),
        identity("old"),
        ExperienceOrigin::Observation,
        identity("provenance/7"),
        7,
        observation("old").sources().clone(),
        ExperienceTemporalRole::Current,
    )
    .unwrap();
    recall
        .execute(&current, &state(vec![(1, different)]))
        .unwrap();
    assert_eq!(
        replace_request(recall.update())
            .evidence()
            .get()
            .as_slice()
            .len(),
        1
    );
}
#[test]
fn item_source_and_wrapper_bounds_refuse_both_outputs_atomically() {
    let mut recall = BoundedExperienceAutobiographyRecall::prepare();
    let memory = state(vec![(1, observation("old"))]);
    let current = event(vec![observation("now")], true);
    recall.execute(&current, &memory).unwrap();
    let full = event(
        (0..8).map(|i| observation(&format!("now/{i}"))).collect(),
        true,
    );
    assert_eq!(
        recall.execute(&full, &memory),
        Err(ExperienceAutobiographyRecallRefusal::ItemBound)
    );
    assert!(recall.update().is_empty() && recall.pending().is_empty());
    let many = with_sources(
        observation("old"),
        (0..4)
            .map(|i| ExperienceEvidenceSource::source(identity(&format!("source/{i}"))).unwrap())
            .collect(),
    );
    assert_eq!(
        recall.execute(&current, &state(vec![(1, many)])),
        Err(ExperienceAutobiographyRecallRefusal::SourceBound)
    );
    let huge = item(
        "old",
        ExperienceOrigin::Observation,
        ExperienceTemporalRole::Current,
        ExperienceAvailability::Present,
        7,
        "provenance/7",
        &[0; 2048],
    );
    assert_eq!(
        recall.execute(&current, &state(vec![(1, huge)])),
        Err(ExperienceAutobiographyRecallRefusal::ByteBound)
    );
    assert!(recall.update().is_empty() && recall.pending().is_empty());
}
#[test]
fn two_episodes_share_explicit_state_and_second_pending_excludes_recursive_recall() {
    let mut recall = BoundedExperienceAutobiographyRecall::prepare();
    let mut retain = BoundedExperienceAutobiographyRetainer::prepare();
    let first = event(vec![observation("first")], true);
    let empty = encode(initial_experience_autobiography_state());
    recall.execute(&first, &empty).unwrap();
    let next = retain.execute(recall.pending(), RESULT).unwrap().to_vec();
    recall
        .execute(&event(vec![observation("second")], true), &next)
        .unwrap();
    assert_eq!(
        replace_request(recall.update())
            .evidence()
            .get()
            .as_slice()
            .len(),
        2
    );
    let second = retain.execute(recall.pending(), RESULT).unwrap();
    let state = ExperienceAutobiographyState::decode(second).unwrap();
    assert_eq!(*state.completed(), 2);
    assert_eq!(state.records().get().as_slice().len(), 2);
    assert!(state.records().get().as_slice().iter().all(|r| r
        .observation()
        .evidence()
        .get()
        .as_slice()
        .len()
        == 1));
}
#[test]
fn execution_and_atomic_refusal_allocate_nothing() {
    let memory = state(vec![(1, observation("old"))]);
    let current = event(vec![observation("now")], true);
    let mut recall = BoundedExperienceAutobiographyRecall::prepare();
    ALLOCATIONS.with(|n| n.set(0));
    COUNTING.with(|f| f.set(true));
    let valid = recall.execute(&current, &memory).is_ok();
    let invalid =
        recall.execute(b"invalid", &memory) == Err(ExperienceAutobiographyRecallRefusal::Malformed);
    let empty = recall.update().is_empty() && recall.pending().is_empty();
    let recovery = recall.execute(&current, &memory).is_ok();
    COUNTING.with(|f| f.set(false));
    assert!(valid && invalid && empty && recovery);
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
}

#[test]
fn unselected_historical_or_current_evidence_cannot_drive_recall() {
    let unselected = request(&[], vec![observation("unselected")]);
    let response =
        ExperienceAutobiographyResponse::model_derived(BoundedBytes::new(RESULT).unwrap()).unwrap();
    let record = ExperienceAutobiographyRecord::new(1, unselected.clone(), response).unwrap();
    let history = encode(
        ExperienceAutobiographyState::new(
            1,
            ExperienceAutobiographyRecords::new(BoundedSequence::try_from_iter([record]).unwrap())
                .unwrap(),
        )
        .unwrap(),
    );
    let current = event(vec![observation("current")], true);
    let mut recall = BoundedExperienceAutobiographyRecall::prepare();
    assert_eq!(
        recall.execute(&current, &history),
        Err(ExperienceAutobiographyRecallRefusal::Evidence(
            ExperienceEvidenceRefusal::UnselectedEvidence
        ))
    );
    assert!(recall.update().is_empty() && recall.pending().is_empty());
    let bad_event = encode(ExperienceAutobiographyEvent::new(unselected, true).unwrap());
    let valid_history = state(vec![(1, observation("selected"))]);
    assert_eq!(
        recall.execute(&bad_event, &valid_history),
        Err(ExperienceAutobiographyRecallRefusal::Evidence(
            ExperienceEvidenceRefusal::UnselectedEvidence
        ))
    );
    assert!(recall.update().is_empty() && recall.pending().is_empty());
}
