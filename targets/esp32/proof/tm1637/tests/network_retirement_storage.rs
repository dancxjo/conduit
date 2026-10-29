//! Actual completed NativePlay keeps custody across final-slot allocation refusal.
use super::*;
use crate::{
    allocation_probe,
    support::{fixture, immediate},
};

#[test]
fn actual_restored_play_keeps_all_owners_on_capacity_refusal_then_retries_once() {
    let (mut play, state, _) = fixture();
    immediate(play.run());
    immediate(play.restore_for_retirement()).unwrap();
    let active = play.active().unwrap().clone();
    let source = play.source.as_ref().unwrap() as *const _;
    let join = play.join.as_ref().unwrap().inspection() as *const _;
    let signs = play
        .completed
        .as_ref()
        .unwrap()
        .signs
        .events()
        .collect::<Vec<_>>();
    let provider_references = std::rc::Rc::strong_count(&state);
    let (refused, memory) = allocation_probe::observe(|| {
        let _refusal = allocation_probe::refuse_next_allocation_size(core::mem::size_of::<
            crate::native_play::RetiredNetworkControl,
        >());
        crate::native_network_retirement_backing::retain_restored(&mut play)
    });
    assert!(matches!(refused, Err("network-retirement-capacity")));
    assert_eq!(
        memory.allocations + memory.reallocations + memory.deallocations,
        0
    );
    assert_eq!(play.status(), "access-point-restored");
    assert_eq!(play.active(), Some(&active));
    assert_eq!(play.source.as_ref().unwrap() as *const _, source);
    assert_eq!(play.join.as_ref().unwrap().inspection() as *const _, join);
    assert_eq!(
        play.completed
            .as_ref()
            .unwrap()
            .signs
            .events()
            .collect::<Vec<_>>(),
        signs
    );
    assert_eq!(std::rc::Rc::strong_count(&state), provider_references);
    assert_eq!(state.calls.get(), [1, 1, 1]);
    // Retry only the same storage/transfer gate, without cleanup or Play again.
    let (retired, memory) = allocation_probe::observe(|| {
        crate::native_network_retirement_backing::retain_restored(&mut play).unwrap()
    });
    assert_eq!(memory.allocations, 1);
    assert_eq!(
        memory.requested_bytes,
        core::mem::size_of::<crate::native_play::RetiredNetworkControl>()
    );
    assert_eq!(memory.reallocations, 0);
    assert_eq!(retired.active(), &active);
    assert_eq!(retired.evidence().events().collect::<Vec<_>>(), signs);
    assert!(retired.owners_recoverable());
    assert!(!retired.evidence_is_committed());
    assert_eq!(std::rc::Rc::strong_count(&state), provider_references);
    assert_eq!(state.calls.get(), [1, 1, 1]);
    assert!(!play.installed());
}

#[test]
fn reserved_slot_cannot_bypass_actual_unrestored_play_retirement_gates() {
    let (mut play, state, _) = fixture();
    let active = play.active().unwrap().clone();
    let kernel = &**play.prepared.as_ref().unwrap() as *const PreparedNetworkKernel;
    let references = std::rc::Rc::strong_count(&state);
    let (result, memory) = allocation_probe::observe(|| {
        crate::native_network_retirement_backing::retain_restored(&mut play)
    });
    assert!(matches!(result, Err("network-not-retirable")));
    assert_eq!(memory.allocations, 1);
    assert_eq!(memory.deallocations, 1);
    assert_eq!(memory.retained_bytes(), 0);
    assert_eq!(play.active(), Some(&active));
    assert_eq!(&**play.prepared.as_ref().unwrap() as *const _, kernel);
    assert!(play.source.is_some() && play.join.is_some());
    assert_eq!(std::rc::Rc::strong_count(&state), references);
    assert_eq!(state.calls.get(), [0, 0, 0]);
}
