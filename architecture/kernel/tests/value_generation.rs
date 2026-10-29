//! Crossing the old generation boundary must never revive stale handles.
use conduit_kernel::{FixedValueStore, StorageError, ValueStorage};
fn repeated(mut store: impl ValueStorage) {
    let first = store.store(b"x").unwrap();
    store.release(first).unwrap();
    // Two complete16-hour Mask allocations use76,800 generations in slot0.
    for generation in 2..=76_801u32 {
        let value = store.store(b"x").unwrap();
        assert_eq!(value.slot, 0);
        assert_eq!(value.generation, generation);
        assert_eq!(store.get(first), Err(StorageError::StaleReference));
        assert_eq!(
            store.resolve_identity(first.slot, first.generation),
            Err(StorageError::StaleReference)
        );
        assert_eq!(
            store.resolve_identity(value.slot, value.generation),
            Ok(value)
        );
        if generation % 19_200 == 0 {
            store.clear();
        } else {
            store.release(value).unwrap();
        }
    }
    let next = store.store(b"x").unwrap();
    assert_eq!(next.generation, 76_802);
    assert_eq!(store.get(first), Err(StorageError::StaleReference));
}
#[test]
fn fixed_generations_continue_across_old_boundary_and_clear() {
    repeated(FixedValueStore::<1, 1>::new(1).unwrap());
}
#[cfg(feature = "alloc")]
#[test]
fn hosted_generations_continue_across_old_boundary_and_clear() {
    repeated(conduit_kernel::HostedValueStore::new(1, 1, 1).unwrap());
    repeated(conduit_kernel::HostedValueStore::new_with_slot_capacities(&[1, 2], 3).unwrap());
}
