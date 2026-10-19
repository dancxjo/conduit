use super::*;
use crate::ValueStorage;
const LANES: [u8; 8] = [0, 0, 128, 63, 0, 0, 0, 128]; // 1.0, negative zero

#[test]
fn logical_quota_and_generation_survive_compact_reference_fanout() {
    let mut values = HostedValueStore::new(2, 24, 32).unwrap();
    assert_eq!(
        values.store_compact_f32(&LANES, 20),
        Err(StorageError::RepresentationMismatch)
    );
    values.prepare_compact_f32();
    let backing = values.reserved_storage_bytes();
    let capacities = values.allocation_capacities();
    values.prepare_compact_f32();
    assert_eq!(values.compact_f32_available(20), (2, 32));
    let value = values.store_compact_f32(&LANES, 20).unwrap();
    assert_eq!(value.byte_len, 20);
    assert_eq!(values.used_bytes(), 20);
    assert_eq!(values.compact_f32(value), Ok(Some(LANES.as_slice())));
    assert_eq!(values.native_u64(value), Ok(None));
    assert_eq!(values.get(value), Err(StorageError::RepresentationMismatch));
    let forged = ValueRef {
        byte_len: 8,
        ..value
    };
    assert_eq!(values.release(forged), Err(StorageError::StaleReference));
    assert_eq!(
        values.compact_f32(forged),
        Err(StorageError::StaleReference)
    );
    values.retain(value).unwrap();
    assert_eq!(values.reference_count(value), Ok(2));
    values.release(value).unwrap();
    assert_eq!(values.used_bytes(), 20);
    assert_eq!(
        values.store_compact_f32(&LANES, 20),
        Err(StorageError::ByteCapacityExceeded)
    );
    assert_eq!(values.used_items(), 1);
    values.release(value).unwrap();
    assert_eq!(values.used_bytes(), 0);
    let next = values.store_compact_f32(&LANES, 20).unwrap();
    assert_eq!(next.slot, value.slot);
    assert_ne!(next.generation, value.generation);
    assert_eq!(values.compact_f32(value), Err(StorageError::StaleReference));
    assert_eq!(values.reserved_storage_bytes(), backing);
    assert_eq!(values.allocation_capacities(), capacities);
}

#[test]
fn equal_physical_and_logical_lengths_never_leak_as_canonical_bytes() {
    let mut values = HostedValueStore::new(1, 8, 8).unwrap();
    values.prepare_compact_f32();
    let value = values.store_compact_f32(&LANES, 8).unwrap();
    assert_eq!(values.get(value), Err(StorageError::RepresentationMismatch));
    assert_eq!(values.compact_f32(value), Ok(Some(LANES.as_slice())));
}

#[test]
fn malformed_or_nonfinite_lanes_refuse_without_changing_ownership() {
    let mut values = HostedValueStore::new(2, 24, 48).unwrap();
    let boundary = values.store(b"boundary").unwrap();
    values.prepare_compact_f32();
    for bytes in [
        &[][..],
        &[0, 0, 0][..],
        &f32::INFINITY.to_le_bytes(),
        &f32::NAN.to_le_bytes(),
    ] {
        assert_eq!(
            values.store_compact_f32(bytes, 20),
            Err(StorageError::RepresentationMismatch)
        );
        assert_eq!((values.used_items(), values.used_bytes()), (1, 8));
    }
    assert_eq!(
        values.store_compact_f32(&LANES, 4),
        Err(StorageError::InvalidBudget)
    );
    assert_eq!(
        values.store_compact_f32(&LANES, 0),
        Err(StorageError::InvalidBudget)
    );
    assert_eq!(values.get(boundary), Ok(b"boundary".as_slice()));
    assert_eq!(values.compact_f32(boundary), Ok(None));
}

#[test]
fn canonical_compact_and_word_reuse_keep_separate_representations() {
    let mut values = HostedValueStore::new(1, 24, 24).unwrap();
    values.prepare_compact_f32();
    values.prepare_native_u64();
    let capacity = values.allocation_capacities();
    let original = values.store(b"").unwrap();
    assert_eq!(values.get(original), Ok(b"".as_slice()));
    values.release(original).unwrap();
    for _ in 0..128 {
        let compact = values.store_compact_f32(&LANES, 20).unwrap();
        values.release(compact).unwrap();
        let word = values.store_native_u64(u64::MAX).unwrap();
        assert_eq!(values.compact_f32(word), Ok(None));
        assert_eq!(values.native_u64(word), Ok(Some(u64::MAX)));
        assert_eq!(values.get(word), Err(StorageError::RepresentationMismatch));
        values.release(word).unwrap();
        let boundary = values.store(b"canonical").unwrap();
        assert_eq!(values.compact_f32(boundary), Ok(None));
        assert_eq!(values.native_u64(boundary), Ok(None));
        assert_eq!(values.get(boundary), Ok(b"canonical".as_slice()));
        values.clear();
        assert_eq!(
            values.compact_f32(boundary),
            Err(StorageError::StaleReference)
        );
    }
    assert_eq!(values.allocation_capacities(), capacity);
    assert_eq!(values.compact_f32_available(20), (1, 24));
}

#[test]
fn heterogeneous_slots_and_exhausted_generations_remain_bounded() {
    let mut values = HostedValueStore::new_with_slot_capacities(&[4, 24], 28).unwrap();
    values.prepare_compact_f32();
    assert_eq!(values.compact_f32_available(20), (1, 28));
    values.slots[1].generation = u16::MAX - 1;
    let last = values.store_compact_f32(&LANES, 20).unwrap();
    assert_eq!(last.slot, 1);
    assert_eq!(last.generation, u16::MAX);
    values.release(last).unwrap();
    assert_eq!(values.compact_f32_available(20), (0, 28));
    assert_eq!(
        values.store_compact_f32(&LANES, 20),
        Err(StorageError::GenerationExhausted)
    );
    assert_eq!(values.compact_f32(last), Err(StorageError::StaleReference));
    let small = values.store(&[1]).unwrap();
    assert_eq!(small.slot, 0);
}
