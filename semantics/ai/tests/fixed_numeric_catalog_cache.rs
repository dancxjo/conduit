#![cfg(feature = "hosted-catalog-cache")]
use conduit_ai::{fixed_numeric_catalog::*, fixed_numeric_pair_catalog::*};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
};
struct CountingAllocator;
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
// Isolated test instrumentation, never part of a product allocator or Step.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;
#[test]
fn hosted_cache_retains_checked_immutable_metadata_and_counts_warm_clones() {
    let start = Instant::now();
    let count = ALLOCATIONS.load(Ordering::Relaxed);
    let types = fixed_numeric_types().unwrap();
    let kinds = fixed_numeric_contracts().unwrap();
    let pairs = fixed_numeric_pair_contracts().unwrap();
    let cold = start.elapsed();
    let cold_allocations = ALLOCATIONS.load(Ordering::Relaxed) - count;
    let start = Instant::now();
    let count = ALLOCATIONS.load(Ordering::Relaxed);
    let warm_types = fixed_numeric_types().unwrap();
    let warm_kinds = fixed_numeric_contracts().unwrap();
    let warm_pairs = fixed_numeric_pair_contracts().unwrap();
    let warm = start.elapsed();
    let warm_allocations = ALLOCATIONS.load(Ordering::Relaxed) - count;
    assert_eq!(types, warm_types);
    assert_eq!(kinds, warm_kinds);
    assert_eq!(pairs, warm_pairs);
    // Independently check the retained Type source, including owner aliases.
    let mut startup = conduit_plot::StartupCatalog::new();
    startup
        .insert_value_kind_alias(
            "ResourceRef",
            conduit_core::kind_id(conduit_core::RESOURCE_REFERENCE_INFO_ID),
        )
        .unwrap();
    let fresh = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(&format!(
            "{FIXED_NUMERIC_SOURCE}\n{FIXED_NUMERIC_SIGNAL_SOURCE}"
        )),
        &startup,
    )
    .unwrap();
    assert_eq!(types, fresh.native_types);
    assert!(
        warm_allocations > 0,
        "returned owned metadata clones allocate during preparation"
    );
    assert!(warm_allocations < cold_allocations);
    eprintln!("checked metadata {} Types, {} Kinds, {} pairs: cold={cold:?}/{cold_allocations} allocations, warm={warm:?}/{warm_allocations} clone allocations; no resource/model custody cached",types.len(),kinds.len(),pairs.len());
}
