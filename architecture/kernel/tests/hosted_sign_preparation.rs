//! Exercise real allocation refusal at each fresh Sign-storage reservation.
#![cfg(feature = "alloc")]
use conduit_kernel::{
    remote_sign_storage_bytes, HostedSignLog, HostedSignPreparationError, KernelEvent,
    KernelEventKind, NodeId, SignError, SignQuery, SignSink,
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    mem::size_of,
};

thread_local! {
    static FAIL_AT: Cell<usize> = const { Cell::new(0) };
    static CALLS: Cell<usize> = const { Cell::new(0) };
    static LIVE: Cell<isize> = const { Cell::new(0) };
}
struct Allocator;
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let armed = FAIL_AT.try_with(Cell::get).unwrap_or(0);
        if armed != 0 {
            let call = CALLS.with(|n| {
                n.set(n.get() + 1);
                n.get()
            });
            if call == armed {
                return std::ptr::null_mut();
            }
        }
        // SAFETY: forward the allocator's unchanged layout to System.
        let pointer = unsafe { System.alloc(layout) };
        if armed != 0 && !pointer.is_null() {
            LIVE.with(|n| n.set(n.get() + layout.size() as isize));
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        if FAIL_AT.try_with(Cell::get).unwrap_or(0) != 0 {
            LIVE.with(|n| n.set(n.get() - layout.size() as isize));
        }
        // SAFETY: forward the live allocation and its original layout.
        unsafe { System.dealloc(pointer, layout) }
    }
}

fn observe<T>(fail_at: usize, action: impl FnOnce() -> T) -> (T, usize, isize) {
    CALLS.with(|n| n.set(0));
    LIVE.with(|n| n.set(0));
    FAIL_AT.with(|n| n.set(fail_at));
    let result = action();
    FAIL_AT.with(|n| n.set(0));
    (result, CALLS.with(Cell::get), LIVE.with(Cell::get))
}
fn byte_budget(items: u16) -> u32 {
    u32::from(items) * size_of::<KernelEvent>() as u32
}

#[test]
fn either_reservation_can_refuse_without_leaking_or_becoming_a_budget_error() {
    for failure in [1, 2] {
        let (result, calls, live) = observe(failure, || {
            HostedSignLog::try_new_with_remote_storage(
                8,
                byte_budget(8),
                2,
                remote_sign_storage_bytes(2).unwrap(),
            )
        });
        assert!(matches!(
            result,
            Err(HostedSignPreparationError::Allocation)
        ));
        assert_eq!(calls, failure);
        assert_eq!(
            live, 0,
            "first reservation must be released if the second refuses"
        );
    }
    // A refusal cannot poison a subsequent preparation or spend its sequence.
    let mut log = HostedSignLog::try_new(8, byte_budget(8)).unwrap();
    let event = log
        .record(NodeId(0), None, None, KernelEventKind::BackCompleted)
        .unwrap();
    assert_eq!(event.sequence, 0);
    assert_eq!(log.len(), 1);
}

#[test]
fn malformed_budgets_match_legacy_validation_before_any_allocation() {
    for (items, bytes, remote_items, remote_bytes) in [
        (0, 1, 0, 0),
        (1, 0, 0, 0),
        (1, byte_budget(1) + 1, 0, 0),
        (1, byte_budget(1), 2, 1),
        (1, byte_budget(1), 0, 1),
        (1, byte_budget(1), 1, 0),
        (
            1,
            byte_budget(1),
            1,
            remote_sign_storage_bytes(1).unwrap() + 1,
        ),
    ] {
        assert!(matches!(
            HostedSignLog::new_with_remote_storage(items, bytes, remote_items, remote_bytes,),
            Err(SignError::InvalidBudget)
        ));
        let (result, calls, live) = observe(1, || {
            HostedSignLog::try_new_with_remote_storage(items, bytes, remote_items, remote_bytes)
        });
        assert!(matches!(
            result,
            Err(HostedSignPreparationError::Budget(SignError::InvalidBudget))
        ));
        assert_eq!((calls, live), (0, 0));
    }
}

#[test]
fn successful_reservation_keeps_existing_finite_sign_and_retention_semantics() {
    let mut legacy = HostedSignLog::new(4, byte_budget(4)).unwrap();
    let mut fallible = HostedSignLog::try_new(4, byte_budget(4)).unwrap();
    for _ in 0..8 {
        assert_eq!(
            legacy.record(NodeId(0), None, None, KernelEventKind::ValueConsumed),
            fallible.record(NodeId(0), None, None, KernelEventKind::ValueConsumed),
        );
        assert!(legacy.events().eq(fallible.events()));
        assert_eq!(legacy.retention_gap(), fallible.retention_gap());
        assert_eq!(legacy.used_bytes(), fallible.used_bytes());
    }
    assert_eq!(legacy.ensure_capacity(4), fallible.ensure_capacity(4));
}
