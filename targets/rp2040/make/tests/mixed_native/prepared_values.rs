//! The shared preallocated store's fallible preparation and immutable capacities.
use conduit_kernel::{
    HostedValueStore, StorageError, ValueStorage, ValueStorePreparationError as E,
};
pub fn run() {
    let caps = [17, 65, 841];
    let budget = HostedValueStore::canonical_storage_bytes(caps).unwrap();
    let (result, cost) = crate::probe::observe(|| {
        HostedValueStore::try_new_with_slot_capacities(&caps, 923, budget - 1)
    });
    assert!(matches!(result, Err(E::Capacity { .. })));
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    for nth in 1..=4 {
        let (result, cost) = crate::probe::observe_refusing(nth, || {
            HostedValueStore::try_new_with_slot_capacities(&caps, 923, budget)
        });
        assert!(matches!(result, Err(E::Allocation)));
        assert_eq!(cost.refused_allocations, 1);
        assert_eq!(cost.allocations, cost.deallocations);
    }
    let (mut store, cost) = crate::probe::observe(|| {
        HostedValueStore::try_new_with_slot_capacities(&caps, 923, budget).unwrap()
    });
    assert_eq!(cost.allocations, 4);
    assert_eq!(cost.reallocations, 0);
    assert_eq!(cost.requested_bytes, budget);
    assert_eq!(store.reserved_storage_bytes(), budget);
    let original = store.allocation_capacities();
    let scratch = [7u8; 841];
    let ((), cost) = crate::probe::observe(|| {
        for _ in 0..1000 {
            assert_eq!(store.preflight_canonical_store(17), Ok(()));
            let a = store.store(&scratch[..17]).unwrap();
            assert_eq!(store.preflight_canonical_store(65), Ok(()));
            let b = store.store(&scratch[..16]).unwrap();
            assert_eq!(store.preflight_canonical_store(841), Ok(()));
            let c = store.store(&scratch).unwrap();
            assert_eq!(
                store.preflight_canonical_store(1),
                Err(StorageError::ItemCapacityExceeded)
            );
            assert_eq!(
                store.preflight_canonical_store(50),
                Err(StorageError::ByteCapacityExceeded)
            );
            assert_eq!(store.get(c).unwrap(), scratch);
            store.release(a).unwrap();
            assert_eq!(
                store.preflight_canonical_store(65),
                Err(StorageError::ItemCapacityExceeded)
            );
            assert_eq!(store.preflight_canonical_store(17), Ok(()));
            store.release(b).unwrap();
            store.release(c).unwrap();
            assert_eq!(store.used_items(), 0);
            assert_eq!(store.used_bytes(), 0);
            assert_eq!(store.allocation_capacities(), original);
        }
    });
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    println!("fallible canonical store: four allocation refusals reclaim partial preparation; one-byte shortage refuses without allocation; 1000 bounded best-fit/preflight cycles without allocation/growth");
}
