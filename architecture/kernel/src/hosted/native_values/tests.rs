use super::*;
#[test]
fn native_preparation_preserves_resources_and_never_copies_word_bytes() {
    let mut store = HostedValueStore::new(3, 8, 24).unwrap();
    let resource = store.store(b"boundary").unwrap();
    assert_eq!(store.native_u64_available(), (0, 0));
    assert_eq!(
        store.store_native_u64(1),
        Err(StorageError::RepresentationMismatch)
    );
    store.prepare_native_u64();
    let capacity = store.allocation_capacities();
    assert_eq!(store.native_u64_available(), (2, 16));
    store.prepare_native_u64();
    assert_eq!(store.allocation_capacities(), capacity);
    assert_eq!(store.get(resource), Ok(b"boundary".as_slice()));
    let value = store.store_native_u64(u64::MAX).unwrap();
    assert!(store.slots[usize::from(value.slot)].bytes.is_empty());
    assert_eq!(store.native_u64(value), Ok(Some(u64::MAX)));
    let forged = ValueRef {
        byte_len: 0,
        ..value
    };
    assert_eq!(store.get(forged), Err(StorageError::StaleReference));
    assert_eq!(store.retain(forged), Err(StorageError::StaleReference));
    assert_eq!(store.release(forged), Err(StorageError::StaleReference));
    assert_eq!(store.get(value), Err(StorageError::RepresentationMismatch));
    assert_eq!(store.used_bytes(), 16);
    store.retain(value).unwrap();
    store.release(value).unwrap();
    assert_eq!(store.reference_count(value), Ok(1));
    store.release(value).unwrap();
    let canonical = store.store(b"newbytes").unwrap();
    assert_eq!(canonical.slot, value.slot);
    assert_ne!(canonical.generation, value.generation);
    assert_eq!(store.native_u64(value), Err(StorageError::StaleReference));
    assert_eq!(store.native_u64(canonical), Ok(None));
    assert_eq!(store.get(canonical), Ok(b"newbytes".as_slice()));
    store.clear();
    assert_eq!(store.native_u64_available(), (3, 24));
    assert_eq!(store.allocation_capacities(), capacity);
    assert_eq!(store.get(resource), Err(StorageError::StaleReference));
}
#[test]
fn native_admission_respects_heterogeneous_capacity_live_bytes_and_retirement() {
    let mut store = HostedValueStore::new_with_slot_capacities(&[1, 8, 16], 9).unwrap();
    store.prepare_native_u64();
    assert_eq!(store.native_u64_available(), (2, 9));
    let word = store.store_native_u64(7).unwrap();
    assert_eq!(store.native_u64_available(), (1, 1));
    assert_eq!(
        store.store_native_u64(9),
        Err(StorageError::ByteCapacityExceeded)
    );
    let bit = store.store(&[1]).unwrap();
    assert_eq!(store.used_bytes(), 9);
    store.release(word).unwrap();
    store.release(bit).unwrap();
    store.slots[1].generation = u16::MAX;
    store.slots[2].generation = u16::MAX;
    assert_eq!(store.native_u64_available(), (0, 9));
    assert_eq!(
        store.store_native_u64(11),
        Err(StorageError::GenerationExhausted)
    );
    let bit = store.store(&[0]).unwrap();
    assert_eq!(store.get(bit), Ok([0].as_slice()));
}
