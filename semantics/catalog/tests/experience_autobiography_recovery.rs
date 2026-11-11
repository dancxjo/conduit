//! Recovery never substitutes a fabricated model envelope for provider loss.
#[allow(dead_code)]
#[path = "common/experience_evidence.rs"]
mod common;
use common::*;
use conduit_core::{TerminalCategory, TerminalInfo};
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
                ALLOCATIONS.with(|count| count.set(count.get() + 1));
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
                ALLOCATIONS.with(|count| count.set(count.get() + 1));
            }
        });
        unsafe { System.realloc(ptr, layout, size) }
    }
}
const RESULT: &[u8] = br#"{"provenance":"ModelDerived","payload_kind":"llm/generated-result@1","payload":[72,105],"implementation_identity":"model/fixture","request_identity":"request/1","run_identity":"run/1","confidence":null,"disposition":"Produced","determinism":"DeterministicValidationFixture","accounting":{"context_items":1,"history_items":0,"input_bytes":100,"output_bytes":2,"work_units":2}}"#;
fn pending(state: ExperienceAutobiographyState, expected: u64) -> Vec<u8> {
    ExperienceAutobiographyPending::new(
        expected,
        request(&["observation"], vec![observation("observation")]),
        state,
        true,
    )
    .unwrap()
    .encode()
    .unwrap()
}
fn history() -> ExperienceAutobiographyState {
    let mut retain = BoundedExperienceAutobiographyRetainer::prepare();
    let input = pending(initial_experience_autobiography_state(), 0);
    ExperienceAutobiographyState::decode(retain.execute(&input, RESULT).unwrap()).unwrap()
}
#[test]
fn successful_result_matches_the_existing_strict_retainer() {
    let input = pending(initial_experience_autobiography_state(), 0);
    let mut old = BoundedExperienceAutobiographyRetainer::prepare();
    let mut recovery = BoundedExperienceAutobiographyRecovery::prepare();
    assert_eq!(
        recovery.execute(&input, RESULT).unwrap(),
        old.execute(&input, RESULT).unwrap()
    );
}
#[test]
fn loss_advances_completed_and_preserves_original_records_byte_exact() {
    let before = history();
    let input = pending(before.clone(), 1);
    let terminal = TerminalInfo::new(TerminalCategory::ProviderOrResourceLoss, [7; 32]).encode();
    let mut recovery = BoundedExperienceAutobiographyRecovery::prepare();
    let after =
        ExperienceAutobiographyState::decode(recovery.recover(&input, &terminal).unwrap()).unwrap();
    assert_eq!(*after.completed(), 2);
    assert_eq!(after.records(), before.records());
    let previous = before.encode().unwrap();
    let current = after.encode().unwrap();
    let previous_view = conduit_core::validate_canonical_structured_value(&previous).unwrap();
    let current_view = conduit_core::validate_canonical_structured_value(&current).unwrap();
    assert_eq!(
        previous_view
            .record_field("records")
            .unwrap()
            .unwrap()
            .value_node(),
        current_view
            .record_field("records")
            .unwrap()
            .unwrap()
            .value_node()
    );
}
#[test]
fn other_terminals_and_stale_or_forged_pending_refuse_without_output() {
    let mut recovery = BoundedExperienceAutobiographyRecovery::prepare();
    let input = pending(history(), 1);
    let loss = TerminalInfo::new(TerminalCategory::ProviderOrResourceLoss, [3; 32]).encode();
    for category in [
        TerminalCategory::Cancelled,
        TerminalCategory::ExecutionFault,
        TerminalCategory::RefusalOrAdmission,
        TerminalCategory::Deadline,
    ] {
        assert_eq!(
            recovery.recover(&input, &TerminalInfo::new(category, [3; 32]).encode()),
            Err(ExperienceAutobiographyRefusal::InvalidResult)
        );
        assert!(recovery.output().is_empty());
    }
    assert_eq!(
        recovery.recover(&pending(history(), 0), &loss),
        Err(ExperienceAutobiographyRefusal::StaleRevision)
    );
    assert_eq!(
        recovery.recover(&pending(history(), 2), &loss),
        Err(ExperienceAutobiographyRefusal::FutureRevision)
    );
    assert_eq!(
        recovery.recover(b"broken", &loss),
        Err(ExperienceAutobiographyRefusal::Malformed)
    );
    assert!(recovery.output().is_empty());
    assert!(recovery.recover(&input, &loss).is_ok());
}
#[test]
fn recovery_execution_does_not_allocate_after_preparation() {
    let input = pending(history(), 1);
    let terminal = TerminalInfo::new(TerminalCategory::ProviderOrResourceLoss, [7; 32]).encode();
    let mut recovery = BoundedExperienceAutobiographyRecovery::prepare();
    ALLOCATIONS.with(|count| count.set(0));
    COUNTING.with(|flag| flag.set(true));
    let accepted = recovery.recover(&input, &terminal).is_ok();
    COUNTING.with(|flag| flag.set(false));
    assert!(accepted);
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
}
#[test]
fn terminal_recovery_and_loss_output_are_declared_ordinary_contracts() {
    let kind = experience_autobiography_recovery_contract();
    assert_eq!(kind.inputs[2].port_id.as_str(), "terminal");
    assert_eq!(
        kind.inputs[2].abnormal_kind.as_ref().unwrap().as_str(),
        conduit_core::TERMINAL_INFO_ID
    );
    assert!(kind.inputs[1].abnormal_kind.is_none());
    assert_eq!(kind.outputs[1].port_id.as_str(), "loss");
    assert!(kind
        .terminal_transductions()
        .any(|law| law.input_port_id.as_str() == "terminal"
            && law.abnormal == conduit_core::AbnormalTerminalTransduction::Recover));
}
