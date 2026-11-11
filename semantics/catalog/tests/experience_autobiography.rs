#[allow(dead_code)] // Shared fixtures serve independent proof surfaces.
#[path = "common/experience_evidence.rs"]
mod common;
use common::*;
use conduit_human::*;
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_semantic_catalog::*;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};
struct CountingAllocator;
thread_local! { static COUNTING: Cell<bool> = const { Cell::new(false) }; static ALLOCATIONS: Cell<usize> = const { Cell::new(0) }; }
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        COUNTING.with(|flag| {
            if flag.get() {
                ALLOCATIONS.with(|n| n.set(n.get() + 1));
            }
        });
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        COUNTING.with(|flag| {
            if flag.get() {
                ALLOCATIONS.with(|n| n.set(n.get() + 1));
            }
        });
        unsafe { System.realloc(ptr, layout, size) }
    }
}
// Exact established serde ModelDerivedResult carrier; field ordering verified
// against its owner and generated ModelWorkAccounting record.
const RESULT: &[u8] = br#"{"provenance":"ModelDerived","payload_kind":"llm/generated-result@1","payload":[72,105],"implementation_identity":"model/fixture","request_identity":"request/1","run_identity":"run/1","confidence":null,"disposition":"Produced","determinism":"DeterministicValidationFixture","accounting":{"context_items":1,"history_items":0,"input_bytes":100,"output_bytes":2,"work_units":2}}"#;
fn pending(state: ExperienceAutobiographyState, retain: bool) -> Vec<u8> {
    let completed = *state.completed();
    encode(
        ExperienceAutobiographyPending::new(
            completed,
            request(&["observation"], vec![observation("observation")]),
            state,
            retain,
        )
        .unwrap(),
    )
}
fn decode(bytes: &[u8]) -> ExperienceAutobiographyState {
    ExperienceAutobiographyState::decode(bytes).unwrap()
}
#[test]
fn retention_preserves_original_evidence_and_actual_derived_envelope() {
    let initial = initial_experience_autobiography_state();
    assert_eq!(*initial.completed(), 0);
    assert!(initial.records().get().as_slice().is_empty());
    let input = pending(initial, true);
    let original = ExperienceAutobiographyPending::decode(&input).unwrap();
    let mut retain = BoundedExperienceAutobiographyRetainer::prepare();
    let state = decode(retain.execute(&input, RESULT).unwrap());
    assert_eq!(*state.completed(), 1);
    let record = &state.records().get().as_slice()[0];
    assert_eq!(*record.episode(), 1);
    assert_eq!(record.observation(), original.observation());
    assert_eq!(
        record.response(),
        &ExperienceAutobiographyResponse::model_derived(
            conduit_plot::rust_binding::BoundedBytes::new(RESULT).unwrap()
        )
        .unwrap()
    );
    assert_eq!(
        record.observation().evidence().get().as_slice()[0].origin(),
        &ExperienceOrigin::Observation
    );
    // No hidden memory: identical explicit inputs reproduce identical state.
    let repeated = decode(retain.execute(&input, RESULT).unwrap());
    assert_eq!(state, repeated);
}
#[test]
fn explicit_selection_advances_completed_and_bounded_ring_evicts_oldest() {
    let mut retain = BoundedExperienceAutobiographyRetainer::prepare();
    let mut state = initial_experience_autobiography_state();
    for selected in [true, false, true, true] {
        state = decode(retain.execute(&pending(state, selected), RESULT).unwrap());
    }
    assert_eq!(*state.completed(), 4);
    let records = state.records().get().as_slice();
    assert_eq!(records.len(), 2);
    assert_eq!(*records[0].episode(), 3);
    assert_eq!(*records[1].episode(), 4);
}
#[test]
fn malformed_forged_noncanonical_and_failed_results_refuse_atomically() {
    let input = pending(initial_experience_autobiography_state(), true);
    let mut retain = BoundedExperienceAutobiographyRetainer::prepare();
    retain.execute(&input, RESULT).unwrap();
    let text = core::str::from_utf8(RESULT).unwrap();
    for bad in [
        text.replace("ModelDerived", "Observation"),
        text.replace("llm/generated-result@1", "value/text"),
        text.replace("Produced", "ProviderLost"),
        text.replace("\"output_bytes\":2", "\"output_bytes\":3"),
        text.replace("\"work_units\":2", "\"work_units\":4097"),
        text.replace("\"payload\":[72,105]", "\"payload\":[256]"),
        text.replace(
            "\"request_identity\":\"request/1\"",
            "\"request_identity\":\"\"",
        ),
        text.replace("{\"provenance\"", "{ \"provenance\""),
    ] {
        assert_eq!(
            retain.execute(&input, bad.as_bytes()),
            Err(ExperienceAutobiographyRefusal::InvalidResult)
        );
        assert!(retain.output().is_empty());
    }
    assert_eq!(
        retain.execute(b"broken", RESULT),
        Err(ExperienceAutobiographyRefusal::Malformed)
    );
    assert!(retain.execute(&input, RESULT).is_ok());
}
#[test]
fn revision_and_evidence_guards_preserve_explicit_state() {
    let mut retain = BoundedExperienceAutobiographyRetainer::prepare();
    let state = decode(
        retain
            .execute(
                &pending(initial_experience_autobiography_state(), true),
                RESULT,
            )
            .unwrap(),
    );
    for (expected, error) in [
        (0, ExperienceAutobiographyRefusal::StaleRevision),
        (2, ExperienceAutobiographyRefusal::FutureRevision),
    ] {
        let input = encode(
            ExperienceAutobiographyPending::new(
                expected,
                request(&["a"], vec![observation("a")]),
                state.clone(),
                true,
            )
            .unwrap(),
        );
        assert_eq!(retain.execute(&input, RESULT), Err(error));
        assert!(retain.output().is_empty());
    }
    let invalid = item(
        "memory",
        ExperienceOrigin::Remembered,
        ExperienceTemporalRole::Current,
        ExperienceAvailability::Present,
        7,
        "provenance/7",
        b"invalid memory",
    );
    let input = encode(
        ExperienceAutobiographyPending::new(1, request(&["memory"], vec![invalid]), state, true)
            .unwrap(),
    );
    assert_eq!(
        retain.execute(&input, RESULT),
        Err(ExperienceAutobiographyRefusal::Evidence(
            ExperienceEvidenceRefusal::InvalidEpistemicCombination
        ))
    );
}
#[test]
fn execution_and_atomic_refusal_allocate_nothing() {
    let mut retain = BoundedExperienceAutobiographyRetainer::prepare();
    let initial = pending(initial_experience_autobiography_state(), true);
    let state = decode(retain.execute(&initial, RESULT).unwrap());
    let next = pending(state, false);
    ALLOCATIONS.with(|n| n.set(0));
    COUNTING.with(|flag| flag.set(true));
    let first = retain.execute(&initial, RESULT).is_ok();
    let second = retain.execute(&next, RESULT).is_ok();
    let refused =
        retain.execute(&next, b"invalid") == Err(ExperienceAutobiographyRefusal::InvalidResult);
    let empty = retain.output().is_empty();
    COUNTING.with(|flag| flag.set(false));
    assert!(first && second && refused && empty);
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
}
