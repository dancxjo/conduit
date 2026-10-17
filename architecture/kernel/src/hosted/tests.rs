use super::*;
#[test]
fn full_hosted_store_refuses_without_inspecting_slots_and_reopens_on_release() {
    let mut store = HostedValueStore::new(4096, 1, 4096).unwrap();
    let handles: [ValueRef; 4096] = core::array::from_fn(|i| {
        let value = store.store(&[]).unwrap();
        assert_eq!(usize::from(value.slot), i);
        value
    });
    store.slot_probes = 0;
    for _ in 0..16 {
        assert_eq!(store.store(&[]), Err(StorageError::ItemCapacityExceeded));
        assert_eq!(store.store(b"x"), Err(StorageError::ItemCapacityExceeded));
    }
    assert_eq!(store.slot_probes, 0);
    assert_eq!(store.used_items(), 4096);
    assert_eq!(store.used_bytes(), 0);
    store.retain(handles[17]).unwrap();
    store.release(handles[17]).unwrap();
    assert_eq!(store.store(&[]), Err(StorageError::ItemCapacityExceeded));
    assert_eq!(store.slot_probes, 0);
    store.release(handles[17]).unwrap();
    let reused = store.store(&[]).unwrap();
    assert_eq!(reused.slot, 17);
    assert_ne!(reused.generation, handles[17].generation);
    assert_eq!(store.get(handles[17]), Err(StorageError::StaleReference));
    assert_eq!(store.used_items(), 4096);
    store.clear();
    assert_eq!(store.used_items(), 0);
    assert_eq!(store.store(&[]).unwrap().slot, 0);
}
#[test]
fn full_hosted_store_preserves_size_and_byte_refusal_precedence() {
    let mut store = HostedValueStore::new(1, 1, 1).unwrap();
    let value = store.store(b"x").unwrap();
    store.slot_probes = 0;
    assert_eq!(store.store(b"xx"), Err(StorageError::ValueTooLarge));
    assert_eq!(store.store(b"y"), Err(StorageError::ByteCapacityExceeded));
    assert_eq!(store.store(&[]), Err(StorageError::ItemCapacityExceeded));
    assert_eq!(store.slot_probes, 0);
    assert_eq!(store.get(value), Ok(b"x".as_slice()));
    assert_eq!(store.reference_count(value), Ok(1));
}
