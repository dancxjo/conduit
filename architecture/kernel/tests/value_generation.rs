//! Exhausting a finite generation domain must never revive old value handles.
use conduit_kernel::{FixedValueStore, StorageError, ValueStorage};
fn one_slot(mut store: impl ValueStorage, clear_last: bool) {
    let first = store.store(b"x").unwrap();
    store.release(first).unwrap();
    for generation in 2..=u16::MAX {
        let value = store.store(b"x").unwrap();
        assert_eq!(value.generation, generation);
        assert_eq!(store.get(first), Err(StorageError::StaleReference));
        if generation == u16::MAX && clear_last {
            store.clear();
        } else {
            if generation == u16::MAX {
                store.retain(value).unwrap();
                store.release(value).unwrap();
                assert_eq!(store.get(value), Ok(b"x".as_slice()));
            }
            store.release(value).unwrap();
        }
    }
    assert_eq!(store.store(b"x"), Err(StorageError::GenerationExhausted));
    assert_eq!(store.used_items(), 0);
    assert_eq!(store.used_bytes(), 0);
    assert_eq!(store.get(first), Err(StorageError::StaleReference));
    store.clear();
    assert_eq!(store.store(b"x"), Err(StorageError::GenerationExhausted));
}
fn two_slots(mut store: impl ValueStorage) {
    let mut first = None;
    for generation in 1..=u16::MAX {
        let value = store.store(b"x").unwrap();
        first.get_or_insert(value);
        assert_eq!((value.slot, value.generation), (0, generation));
        store.release(value).unwrap();
    }
    let next = store.store(b"x").unwrap();
    assert_eq!((next.slot, next.generation), (1, 1));
    assert_eq!(store.get(first.unwrap()), Err(StorageError::StaleReference));
    store.clear();
    let reused = store.store(b"x").unwrap();
    assert_eq!((reused.slot, reused.generation), (1, 2));
    assert_eq!(store.get(next), Err(StorageError::StaleReference));
}
#[test]
fn fixed_generations_retire_on_final_release_or_clear() {
    for clear in [false, true] {
        one_slot(FixedValueStore::<1, 1>::new(1).unwrap(), clear);
    }
    two_slots(FixedValueStore::<2, 1>::new(2).unwrap());
}
#[cfg(feature = "alloc")]
#[test]
fn hosted_generations_retire_on_final_release_or_clear() {
    use conduit_kernel::HostedValueStore;
    for clear in [false, true] {
        one_slot(HostedValueStore::new(1, 1, 1).unwrap(), clear);
    }
    two_slots(HostedValueStore::new(2, 1, 2).unwrap());
}
