#![cfg(feature = "fixed-numeric-owners")]
use conduit_ai::{fixed_numeric_catalog::fixed_numeric_type, nominal_weakening::*};
#[path = "../src/numeric_allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[test]
fn complete_weakening_construction_is_admitted_before_allocation() {
    let profile =
        PreparedNominalWeakening::prepare(fixed_numeric_type("NumericF32Vector20").unwrap())
            .unwrap();
    let (r, o) =
        allocation_probe::observe(|| NominalWeakeningBack::storage_reservation(&profile).unwrap());
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
            NominalWeakeningBack::prepare_selected_with_storage_limits(
                &profile, true, requested, retained,
            )
        });
        assert!(matches!(
            result,
            Err(WeakeningBackPreparationRefusal::Capacity)
        ));
        assert_eq!((o.allocations, o.reallocations), (0, 0));
    }
    let (result, o) = allocation_probe::observe(|| {
        NominalWeakeningBack::prepare_selected_with_storage_limits(
            &profile,
            true,
            r.preparation_requested_bytes_bound,
            r.retained_heap_bytes_bound,
        )
    });
    let (back, receipt) = result.unwrap();
    assert!(o.requested_bytes <= receipt.preparation_requested_bytes_bound);
    assert!(o.live_bytes <= receipt.retained_heap_bytes_bound);
    assert_eq!(o.live_bytes, receipt.retained_accounted_heap_bytes);
    assert_eq!(o.live_bytes, back.local_accounted_heap_bytes());
    assert_eq!(back.committed_frames(), 0);
    println!(
        "selected weakening requested={} peak={} retained={} reservation={r:?}",
        o.requested_bytes, o.peak_bytes, o.live_bytes
    );
}
