#![cfg(feature = "alloc")]

use conduit_kernel::{HostedValueStore, StorageError, ValueStorage};
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

#[test]
fn heterogeneous_slots_preserve_large_slots_and_never_allocate_during_use() {
    let mut store = HostedValueStore::new_with_slot_capacities(&[128, 4, 16], 148).unwrap();
    let prepared = store.allocation_capacities();
    assert_eq!(prepared, (3, 148));
    assert!(store.reserved_storage_bytes() > 148);
    let (_, observed) = allocation_probe::observe(|| {
        for _ in 0..100 {
            let small = store.store(&[1; 4]).unwrap();
            let medium = store.store(&[2; 16]).unwrap();
            let large = store.store(&[3; 128]).unwrap();
            assert_eq!(small.slot, 1);
            assert_eq!(medium.slot, 2);
            assert_eq!(large.slot, 0);
            store.retain(large).unwrap();
            store.release(large).unwrap();
            assert_eq!(store.get(large).unwrap(), &[3; 128]);
            store.release(small).unwrap();
            store.release(medium).unwrap();
            store.release(large).unwrap();
        }
        assert_eq!(store.store(&[0; 129]), Err(StorageError::ValueTooLarge));
    });
    assert_eq!((observed.allocations, observed.reallocations), (0, 0));
    assert_eq!(store.allocation_capacities(), prepared);
}

#[test]
fn heterogeneous_live_quota_and_shape_exhaustion_are_independent() {
    let mut quota = HostedValueStore::new_with_slot_capacities(&[8, 128], 100).unwrap();
    assert_eq!(quota.allocation_capacities().1, 136);
    assert_eq!(quota.byte_capacity(), 100);
    assert_eq!(
        quota.store(&[0; 101]),
        Err(StorageError::ByteCapacityExceeded)
    );
    let mut shape = HostedValueStore::new_with_slot_capacities(&[4, 128], 132).unwrap();
    let large = shape.store(&[0; 64]).unwrap();
    assert_eq!(
        shape.store(&[0; 8]),
        Err(StorageError::ItemCapacityExceeded)
    );
    assert_eq!(shape.used_items(), 1);
    shape.release(large).unwrap();
    assert!(shape.store(&[0; 8]).is_ok());
    for capacities in [&[][..], &[0][..], &[u32::MAX, 1][..]] {
        assert!(matches!(
            HostedValueStore::new_with_slot_capacities(capacities, 1),
            Err(StorageError::InvalidBudget)
        ));
    }
}
