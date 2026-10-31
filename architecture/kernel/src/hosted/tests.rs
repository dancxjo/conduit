use super::*;
#[test]
fn full_hosted_store_refuses_without_inspecting_slots_and_reopens_on_release() {
    let mut store = HostedValueStore::new(4096, 1, 4096).unwrap();
    let handles: [ValueRef; 4096] = core::array::from_fn(|i| {
        let value = store.store(&[]).unwrap();
        assert_eq!(usize::from(value.slot), i);
        value
    });
    assert_eq!(store.slot_probes, 4096);
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

#[test]
fn mixed_hosted_capacities_keep_smallest_fit_and_first_equal_slot() {
    let mut store = HostedValueStore::new_with_slot_capacities(&[8, 1, 4, 1], 14).unwrap();
    assert!(!store.uniform_slots);
    let values = [
        store.store(&[]).unwrap(),
        store.store(&[]).unwrap(),
        store.store(&[]).unwrap(),
        store.store(&[]).unwrap(),
    ];
    assert_eq!(values.map(|v| v.slot), [1, 3, 2, 0]);
    store.release(values[0]).unwrap();
    store.release(values[2]).unwrap();
    assert_eq!(store.store(b"xx").unwrap().slot, 2);
    assert_eq!(store.store(b"x").unwrap().slot, 1);
}

#[test]
fn final_generation_retires_without_wrap_on_release_or_clear() {
    for clear in [false, true] {
        let mut store = HostedValueStore::new(2, 1, 2).unwrap();
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
