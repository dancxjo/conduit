#![cfg(feature = "fixed-numeric-owners")]
use conduit_ai::fixed_numeric_u16_profile::*;
#[path = "../src/numeric_allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[test]
fn complete_contract_only_constructor_is_reserved_before_clones() {
    for source in [
        "type FarganPeriod = U16 in 32..=255\n",
        "type Full = U16 in 0..=65535\n",
    ] {
        let profile = PreparedU16Profile::check_definition(source).unwrap();
        let (r, o) = allocation_probe::observe(|| {
            U16ProfileBack::storage_reservation_contract_only(&profile).unwrap()
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
                U16ProfileBack::prepare_contract_only_with_storage_limits(
                    &profile, true, requested, retained,
                )
            });
            assert!(matches!(
                result,
                Err(U16ContractOnlyPreparationRefusal::Capacity)
            ));
            assert_eq!((o.allocations, o.reallocations), (0, 0));
        }
        let (result, o) = allocation_probe::observe(|| {
            U16ProfileBack::prepare_contract_only_with_storage_limits(
                &profile,
                true,
                r.preparation_requested_bytes_bound,
                r.retained_heap_bytes_bound,
            )
        });
        let (back, receipt) = result.unwrap();
        assert!(
            o.requested_bytes <= receipt.preparation_requested_bytes_bound,
            "actual {} bound {}",
            o.requested_bytes,
            receipt.preparation_requested_bytes_bound
        );
        assert!(o.live_bytes <= receipt.retained_heap_bytes_bound);
        assert_eq!(o.live_bytes, receipt.retained_accounted_heap_bytes);
        assert_eq!(o.live_bytes, back.local_accounted_heap_bytes());
        assert_eq!(back.committed_frames(), 0);
        println!(
            "{source:?} contract-only requested={} peak={} retained={} reservation={r:?}",
            o.requested_bytes, o.peak_bytes, o.live_bytes
        );
    }
}
#[test]
fn valid_general_invariant_profile_refuses_limited_preparation_without_allocation() {
    let profile =
        PreparedU16Profile::check_definition("type Allowed = U16 in 32..=255 where . % 2 == 0\n")
            .unwrap();
    let (r, o) = allocation_probe::observe(|| {
        U16ProfileBack::prepare_contract_only_with_storage_limits(
            &profile,
            true,
            usize::MAX,
            usize::MAX,
        )
    });
    assert!(matches!(
        r,
        Err(U16ContractOnlyPreparationRefusal::UnsupportedInvariantPreparation)
    ));
    assert_eq!((o.allocations, o.reallocations), (0, 0));
}
