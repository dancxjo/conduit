use super::*;
fn make_layout(tag: u8) -> CompactCanonicalLayout {
    // Fixture codec: one physical octet expands to exactly 100 canonical bytes.
    unsafe { CompactCanonicalLayout::from_trusted_make([tag; 32], 1, 100).unwrap() }
}
#[test]
fn dedicated_slots_keep_logical_quota_and_canonical_reads_fenced() {
    let layout = make_layout(1);
    let mut store =
        HostedValueStore::new_with_compact_canonical_slots(&[4; 7], 2, layout, 228).unwrap();
    let backing = store.reserved_storage_bytes();
    assert_eq!(
        backing,
        HostedValueStore::compact_canonical_storage_bytes(&[4; 7], 2, layout).unwrap()
    );
    assert_eq!(store.item_capacity(), 9);
    assert_eq!(store.byte_capacity(), 228);
    let mut ordinary = Vec::new();
    for _ in 0..7 {
        ordinary.push(store.store(b"data").unwrap());
    }
    assert_eq!(
        store.store(b"more"),
        Err(StorageError::ItemCapacityExceeded)
    );
    let queued = store.store_compact_canonical(layout, b"a", 100).unwrap();
    let staged = store.store_compact_canonical(layout, b"b", 100).unwrap();
    assert_eq!((store.used_items(), store.used_bytes()), (9, 228));
    assert_eq!(
        store.resolve_identity(queued.slot, queued.generation),
        Ok(queued)
    );
    assert_eq!(store.get(queued), Err(StorageError::RepresentationMismatch));
    assert_eq!(
        store.compact_canonical(queued, layout),
        Ok(Some(b"a".as_slice()))
    );
    assert_eq!(store.retain(queued), Ok(()));
    store.release(queued).unwrap();
    assert_eq!(store.reference_count(queued), Ok(1));
    store.release(queued).unwrap();
    assert_eq!(store.get(queued), Err(StorageError::StaleReference));
    let next = store.store_compact_canonical(layout, b"c", 100).unwrap();
    assert_eq!(next.slot, queued.slot);
    assert_ne!(next.generation, queued.generation);
    assert_eq!(
        store.compact_canonical(next, make_layout(2)),
        Err(StorageError::RepresentationMismatch)
    );
    store.release(staged).unwrap();
    store.release(next).unwrap();
    for value in ordinary {
        store.release(value).unwrap();
    }
    assert_eq!((store.used_items(), store.used_bytes()), (0, 0));
    assert_eq!(store.reserved_storage_bytes(), backing);
}
#[test]
fn physical_and_logical_pressure_preserve_live_values_and_generations() {
    let layout = make_layout(1);
    let mut store =
        HostedValueStore::new_with_compact_canonical_slots(&[4], 2, layout, 104).unwrap();
    let value = store.store_compact_canonical(layout, b"x", 100).unwrap();
    for (bytes, logical, error) in [
        (b"xx".as_slice(), 100, StorageError::ValueTooLarge),
        (b"y", 101, StorageError::ValueTooLarge),
        (b"y", 100, StorageError::ByteCapacityExceeded),
    ] {
        assert_eq!(
            store.store_compact_canonical(layout, bytes, logical),
            Err(error)
        );
        assert_eq!(
            store.compact_canonical(value, layout),
            Ok(Some(b"x".as_slice()))
        );
        assert_eq!((store.used_items(), store.used_bytes()), (1, 100));
    }
    let forged = ValueRef {
        byte_len: 1,
        ..value
    };
    assert_eq!(store.release(forged), Err(StorageError::StaleReference));
    store.clear();
    assert_eq!(
        store.resolve_identity(value.slot, value.generation),
        Err(StorageError::StaleReference)
    );
    let next = store.store_compact_canonical(layout, b"y", 100).unwrap();
    assert_eq!(next.slot, value.slot);
    assert_ne!(next.generation, value.generation);
}
