//! Fixture-level incremental heap accounting; excludes preexisting shared inputs.
use conduit_core::*;
use conduit_planner::resource_cost::*;
use conduit_plot::CheckedPlot;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};
thread_local! {
    static ENABLED: Cell<bool> = const { Cell::new(false) };
    static LIVE: Cell<isize> = const { Cell::new(0) };
    static PEAK: Cell<isize> = const { Cell::new(0) };
}
struct Counting;
fn charge(delta: isize) {
    let _ = ENABLED.try_with(|enabled| {
        if enabled.get() {
            let _ = LIVE.try_with(|live| {
                let next = live.get().saturating_add(delta);
                live.set(next);
                let _ = PEAK.try_with(|peak| peak.set(peak.get().max(next)));
            });
        }
    });
}
// SAFETY: All allocations and deallocations delegate to System with the exact
// caller-provided Layout; instrumentation never dereferences returned pointers.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            charge(isize::try_from(layout.size()).unwrap_or(isize::MAX));
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        charge(-isize::try_from(layout.size()).unwrap_or(isize::MAX));
        unsafe { System.dealloc(pointer, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: Counting = Counting;

pub fn check(plot: &CheckedPlot, inputs: CostPlanningInputs<'_, '_>) {
    let budget = GlobalPlacementBudget { maximum_states: 64 };
    let objective = GlobalPlacementObjective::AggregateFulfillmentTime;
    let reference = plan_global_realizations_with_objective(
        plot,
        inputs,
        budget,
        objective,
        &mut ExactNumericWork::new(100_000_000),
    )
    .unwrap();
    LIVE.with(|value| value.set(0));
    PEAK.with(|value| value.set(0));
    ENABLED.with(|value| value.set(true));
    let mut cache = GlobalPlanCache::admit(GlobalPlanCachePolicy {
        maximum_entries: 8,
        maximum_encoded_plan_bytes: 1_048_576,
        maximum_fingerprint_bytes_per_call: 16_777_216,
    })
    .unwrap();
    let first = cache
        .plan(
            plot,
            inputs,
            budget,
            objective,
            None,
            &mut ExactNumericWork::new(100_000_000),
        )
        .unwrap();
    assert_eq!(first.plan, reference.plan);
    drop(first);
    let retained = LIVE.with(Cell::get);
    assert!(retained > 0);
    for _ in 0..64 {
        let result = cache
            .plan(
                plot,
                inputs,
                budget,
                objective,
                None,
                &mut ExactNumericWork::new(100_000_000),
            )
            .unwrap();
        assert_eq!(result.plan, reference.plan);
        drop(result);
        assert_eq!(LIVE.with(Cell::get), retained);
        assert!(cache.metrics().retained_entries <= 8);
        assert!(cache.metrics().retained_encoded_plan_bytes <= 1_048_576);
    }
    cache.discard();
    let after_discard = LIVE.with(Cell::get);
    assert!(after_discard < retained);
    drop(cache);
    let after_drop = LIVE.with(Cell::get);
    let peak = PEAK.with(Cell::get);
    ENABLED.with(|value| value.set(false));
    assert_eq!(after_drop, 0);
    println!(
        "CACHE_HEAP {}",
        serde_json::json!({"iterations":64,"retained_requested_heap_bytes":retained,
        "peak_requested_heap_bytes":peak,"after_discard_requested_heap_bytes":after_discard,"after_drop_requested_heap_bytes":after_drop,
        "scope":"two-Gear fixture, test-thread requested allocator bytes; excludes shared input retention, allocator overhead and other threads; not a universal heap bound"})
    );
}
