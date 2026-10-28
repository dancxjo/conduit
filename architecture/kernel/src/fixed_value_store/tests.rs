use super::*;
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

#[test]
fn const_reserved_borrowed_store_preserves_canonical_capacity_and_generations() {
    const INITIAL: Result<FixedValueStore<2, 8>, StorageError> = FixedValueStore::new(16);
    let mut backing = INITIAL.unwrap();
    fn exercise(mut values: impl ValueStorage) {
        let first = values.store(b"12345678").unwrap();
        let second = values.store(b"abcdefgh").unwrap();
        assert_eq!(values.store(b"x"), Err(StorageError::ByteCapacityExceeded));
        values.retain(first).unwrap();
        values.release(first).unwrap();
        assert_eq!(values.get(first).unwrap(), b"12345678");
        values.release(first).unwrap();
        let next = values.store(b"new").unwrap();
        assert_ne!(next.generation, first.generation);
        assert_eq!(values.get(first), Err(StorageError::StaleReference));
        values.release(second).unwrap();
        values.release(next).unwrap();
        assert_eq!(values.used_items(), 0);
    }
    exercise(&mut backing);
    exercise(backing);
    const ZERO: Result<FixedValueStore<0, 8>, StorageError> = FixedValueStore::new(1);
    assert!(matches!(ZERO, Err(StorageError::InvalidBudget)));
}

#[test]
fn fixed_encoding_refusals_and_unwind_release_ownership_without_reviving_generations() {
    extern crate std;
    use crate::{CanonicalEncodedValueStorage, EncodedValueRefusal};
    let mut values = FixedValueStore::<2, 8>::new(16).unwrap();
    let first = values
        .store_encoded(8, |out| {
            out[..3].copy_from_slice(b"one");
            Ok::<_, ()>(3)
        })
        .unwrap();
    assert_eq!(values.get(first).unwrap(), b"one");
    assert_eq!(
        values.store_encoded(8, |out| {
            out.fill(9);
            Err(7)
        }),
        Err(EncodedValueRefusal::Encoder(7))
    );
    assert_eq!((values.used_items(), values.used_bytes()), (1, 3));
    assert_eq!(
        values.store_encoded(8, |_| Ok::<_, ()>(9)),
        Err(EncodedValueRefusal::Length)
    );
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = values.store_encoded::<()>(8, |out| {
            out.fill(8);
            panic!("encoder")
        });
    }));
    assert!(result.is_err());
    assert_eq!((values.used_items(), values.used_bytes()), (1, 3));
    let next = values
        .store_encoded(8, |out| {
            assert_eq!(out, &[0; 8]);
            Ok::<_, ()>(0)
        })
        .unwrap();
    assert_eq!(next.generation, 4);
    values.release(first).unwrap();
    values.release(next).unwrap();
}

#[test]
fn empty_scratch_is_exclusive_wiped_and_preserves_value_identity() {
    extern crate std;
    let mut values = FixedValueStore::<2, 8>::new(16).unwrap();
    let first = values.store(b"live").unwrap();
    let mut called = false;
    assert_eq!(
        values.with_empty_scratch(|_| called = true),
        Err(StorageError::ItemCapacityExceeded)
    );
    assert!(!called);
    assert_eq!(values.get(first).unwrap(), b"live");
    values.release(first).unwrap();
    let generations = values.slots.each_ref().map(|slot| slot.generation);
    values
        .with_empty_scratch(|slices| {
            assert_eq!(slices.len(), 2);
            assert!(slices
                .iter()
                .all(|slice| slice.iter().all(|byte| *byte == 0)));
            slices[0].fill(5);
            slices[1].fill(9);
        })
        .unwrap();
    assert!(values.slots.iter().all(|slot| slot.bytes == [0; 8]));
    assert_eq!(
        values.slots.each_ref().map(|slot| slot.generation),
        generations
    );
    assert_eq!((values.used_items(), values.used_bytes()), (0, 0));
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = values.with_empty_scratch(|slices| {
            slices[0].fill(6);
            slices[1].fill(7);
            panic!("cold preparation");
        });
    }))
    .is_err());
    assert!(values.slots.iter().all(|slot| slot.bytes == [0; 8]));
    let next = values.store(b"new").unwrap();
    assert_eq!(next.slot, first.slot);
    assert_eq!(next.generation, first.generation + 1);
    assert_eq!(values.get(first), Err(StorageError::StaleReference));
    assert_eq!(values.get(next).unwrap(), b"new");
}

#[test]
fn final_generation_retires_without_wrap_on_release_or_clear() {
    for clear in [false, true] {
        let mut store = FixedValueStore::<2, 1>::new(2).unwrap();
        store.slots[0].generation = u32::MAX - 1;
        let last = store.store(b"x").unwrap();
        assert_eq!(last.generation, u32::MAX);
        assert_eq!(store.resolve_identity(last.slot, last.generation), Ok(last));
        if clear {
            store.clear();
        } else {
            store.release(last).unwrap();
        }
        assert_eq!(
            store.resolve_identity(last.slot, last.generation),
            Err(StorageError::StaleReference)
        );
        let next = store.store(b"x").unwrap();
        assert_eq!((next.slot, next.generation), (1, 1));
        store.clear();
        let next2 = store.store(b"x").unwrap();
        assert_eq!((next2.slot, next2.generation), (1, 2));
        store.clear();
        store.slots[1].generation = u32::MAX;
        assert_eq!(store.store(b"x"), Err(StorageError::GenerationExhausted));
    }
}
