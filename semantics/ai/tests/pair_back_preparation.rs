#![cfg(feature = "fixed-numeric-owners")]
use conduit_ai::{closing_structured_pair::*, fixed_numeric_catalog::fixed_numeric_type};
use conduit_core::*;
#[path = "../src/numeric_allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[test]
fn complete_pair_construction_is_reserved_before_types_are_cloned() {
    let left = StructuredInfoType::leaf(kind_id("value/u16")).unwrap();
    let right = fixed_numeric_type("NumericF32Vector20").unwrap();
    let profile = ClosingStructuredPairProfile::prepare(left, right).unwrap();
    let (r, o) = allocation_probe::observe(|| {
        ClosingStructuredPairBack::storage_reservation(&profile).unwrap()
    });
    assert_eq!((o.allocations, o.reallocations), (0, 0));
    for which in 0..2 {
        let mut requested = r.preparation_requested_bytes_bound;
        let mut retained = r.retained_heap_bytes_bound;
        if which == 0 {
            requested -= 1;
        } else {
            retained -= 1;
        }
        let (result, o) = allocation_probe::observe(|| {
            ClosingStructuredPairBack::prepare_selected_with_storage_limits(
                &profile, requested, retained,
            )
        });
        assert!(matches!(
            result,
            Err(PairBackPreparationRefusal::Composition(
                PreparedStructuredCompositionStorageRefusal::Capacity
            ))
        ));
        assert_eq!((o.allocations, o.reallocations), (0, 0));
    }
    let (result, o) = allocation_probe::observe(|| {
        ClosingStructuredPairBack::prepare_selected_with_storage_limits(
            &profile,
            r.preparation_requested_bytes_bound,
            r.retained_heap_bytes_bound,
        )
    });
    let (back, receipt) = result.unwrap();
    assert!(o.requested_bytes <= receipt.preparation_requested_bytes_bound);
    assert!(o.live_bytes <= receipt.retained_heap_bytes_bound);
    assert_eq!(o.live_bytes, back.local_accounted_heap_bytes());
    assert_eq!(back.committed_pairs(), 0);
    println!(
        "selected pair requested={} peak={} retained={} reservation={r:?}",
        o.requested_bytes, o.peak_bytes, o.live_bytes
    );
}
