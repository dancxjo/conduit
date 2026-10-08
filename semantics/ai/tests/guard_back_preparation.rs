#![cfg(feature = "fixed-numeric-owners")]
use conduit_ai::{fixed_numeric_catalog::fixed_numeric_type, fixed_numeric_guard::*};
#[path = "../src/numeric_allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[test]
fn selected_structural_guard_reserves_full_validator_and_output() {
    let profile =
        FixedGuardProfile::prepare(fixed_numeric_type("NumericF32Vector20").unwrap()).unwrap();
    let (reservation, o) =
        allocation_probe::observe(|| FixedGuardBack::storage_reservation(&profile).unwrap());
    assert_eq!((o.allocations, o.reallocations), (0, 0));
    for which in 0..2 {
        let mut requested = reservation.preparation_requested_bytes_bound;
        let mut retained = reservation.retained_heap_bytes_bound;
        if which == 0 {
            requested -= 1;
        } else {
            retained -= 1;
        }
        let (result, o) = allocation_probe::observe(|| {
            FixedGuardBack::prepare_selected_with_storage_limits(&profile, requested, retained)
        });
        assert!(matches!(result, Err(GuardBackPreparationRefusal::Capacity)));
        assert_eq!((o.allocations, o.reallocations), (0, 0));
    }
    let (result, o) = allocation_probe::observe(|| {
        FixedGuardBack::prepare_selected_with_storage_limits(
            &profile,
            reservation.preparation_requested_bytes_bound,
            reservation.retained_heap_bytes_bound,
        )
    });
    let (back, receipt) = result.unwrap();
    assert!(o.requested_bytes <= receipt.preparation_requested_bytes_bound);
    assert!(o.live_bytes <= receipt.retained_heap_bytes_bound);
    assert_eq!(o.live_bytes, receipt.retained_accounted_heap_bytes);
    assert_eq!(o.live_bytes, back.local_accounted_heap_bytes());
    println!(
        "selected structural guard requested={} peak={} retained={} reservation={reservation:?}",
        o.requested_bytes, o.peak_bytes, o.live_bytes
    );
}
