use super::*;

#[test]
fn canonical_preflight_matches_pressure_and_retired_slot_refusals() {
    let mut store = FixedValueStore::<1, 8>::new(8).unwrap();
    assert_eq!(
        store.preflight_canonical_store(9),
        Err(StorageError::ValueTooLarge)
    );
    assert_eq!(store.preflight_canonical_store(8), Ok(()));
    let small = store.store(&[1]).unwrap();
    assert_eq!(
        store.preflight_canonical_store(1),
        Err(StorageError::ItemCapacityExceeded)
    );
    store.release(small).unwrap();
    let held = store.store(&[1; 8]).unwrap();
    assert_eq!(
        store.preflight_canonical_store(1),
        Err(StorageError::ByteCapacityExceeded)
    );
    assert_eq!(store.get(held).unwrap(), &[1; 8]);
    store.release(held).unwrap();
    assert_eq!(store.preflight_canonical_store(8), Ok(()));
    store.slots[0].generation = u16::MAX;
    store.has_retired_slots = true;
    assert_eq!(
        store.preflight_canonical_store(1),
        Err(StorageError::GenerationExhausted)
    );
    assert_eq!(store.store(&[1]), Err(StorageError::GenerationExhausted));
}
#[test]
fn dense_store_and_released_holes_preserve_first_free_slot_identity() {
    let mut store = FixedValueStore::<4096, 1>::new(1).unwrap();
    let values: [ValueRef; 4096] = core::array::from_fn(|i| {
        let value = store.store(&[]).unwrap();
        assert_eq!(usize::from(value.slot), i);
        value
    });
    assert_eq!(store.slot_probes, 4096);
    assert_eq!(store.store(&[]), Err(StorageError::ItemCapacityExceeded));
    assert_eq!(store.slot_probes, 4096);
    store.retain(values[0]).unwrap();
    store.release(values[0]).unwrap();
    assert_eq!(store.free_search_start, 4096);
    store.release(values[4000]).unwrap();
    store.release(values[100]).unwrap();
    assert_eq!(store.store(&[]).unwrap().slot, 100);
    assert_eq!(store.store(&[]).unwrap().slot, 4000);
    store.release(values[0]).unwrap();
    let reused = store.store(&[]).unwrap();
    assert_eq!(reused.slot, 0);
    assert_ne!(reused.generation, values[0].generation);
    assert_eq!(store.get(values[0]), Err(StorageError::StaleReference));
    store.clear();
    assert_eq!(store.free_search_start, 0);
    assert_eq!(store.used_items(), 0);
    assert_eq!(store.get(reused), Err(StorageError::StaleReference));
    let after_clear = store.store(&[]).unwrap();
    assert_eq!(after_clear.slot, 0);
    assert_ne!(after_clear.generation, reused.generation);
}
#[test]
fn refused_storage_and_stale_release_do_not_move_free_bound() {
    let mut store = FixedValueStore::<4, 1>::new(1).unwrap();
    let value = store.store(b"x").unwrap();
    assert_eq!(store.free_search_start, 1);
    for bytes in [b"y".as_slice(), b"too large".as_slice()] {
        assert!(store.store(bytes).is_err());
        assert_eq!(store.free_search_start, 1);
        assert_eq!(store.slot_probes, 1);
    }
    let forged = ValueRef {
        generation: value.generation + 1,
        ..value
    };
    assert_eq!(store.release(forged), Err(StorageError::StaleReference));
    assert_eq!(store.free_search_start, 1);
    assert_eq!(store.store(&[]).unwrap().slot, 1);
    assert_eq!(store.get(value), Ok(b"x".as_slice()));
}
