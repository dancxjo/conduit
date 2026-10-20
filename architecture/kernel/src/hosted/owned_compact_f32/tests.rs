use super::*;
const LANES: [u8; 8] = [0, 0, 128, 63, 0, 0, 0, 128];
fn layout() -> CompactF32Layout {
    CompactF32Layout {
        lanes: 2,
        logical_bytes: 20,
    }
}
fn store() -> (HostedValueStore, PreparedCompactF32Output) {
    let mut values = HostedValueStore::new(4, 24, 48).unwrap();
    let owner = values
        .prepare_owned_compact_f32_outputs(&[(layout(), 2)])
        .unwrap()
        .remove(0);
    (values, owner)
}
#[test]
fn exact_lane_buffer_moves_and_restores_without_a_payload_copy() {
    let (mut values, owner) = store();
    let mut bytes = LANES.to_vec();
    let original = bytes.as_ptr();
    let backing = values.reserved_storage_bytes();
    let capacities = values.allocation_capacities();
    let mut pending = values.swap_owned_compact_f32(&owner, &mut bytes).unwrap();
    let value = pending.value().unwrap();
    assert_eq!(
        values.compact_f32(value).unwrap().unwrap().as_ptr(),
        original
    );
    assert_eq!(values.compact_f32(value).unwrap().unwrap(), LANES);
    assert!(bytes.is_empty());
    assert_eq!(bytes.capacity(), 8);
    assert_eq!(values.used_bytes(), 20);
    assert_eq!(values.get(value), Err(StorageError::RepresentationMismatch));
    values
        .restore_owned_compact_f32(&owner, &mut pending, &mut bytes)
        .unwrap();
    assert_eq!(bytes.as_ptr(), original);
    assert_eq!(bytes, LANES);
    assert_eq!(values.used_items(), 0);
    assert_eq!(values.used_bytes(), 0);
    assert_eq!(values.compact_f32(value), Err(StorageError::StaleReference));
    assert_eq!(pending.value(), Err(StorageError::StaleReference));
    assert_eq!(values.allocation_capacities(), capacities);
    assert_eq!(values.reserved_storage_bytes(), backing);
    let canonical = values.store(&[7; 24]).unwrap();
    assert!(!owner.slots.contains(&canonical.slot));
    assert_eq!(values.get(canonical).unwrap(), [7; 24]);
}
#[test]
fn committed_fanout_retains_one_buffer_and_releases_each_reference() {
    let (mut values, owner) = store();
    let mut bytes = LANES.to_vec();
    let mut pending = values.swap_owned_compact_f32(&owner, &mut bytes).unwrap();
    let value = pending.value().unwrap();
    values.retain(value).unwrap();
    values
        .commit_owned_compact_f32(&owner, &mut pending)
        .unwrap();
    assert!(values
        .restore_owned_compact_f32(&owner, &mut pending, &mut bytes)
        .is_err());
    values.release(value).unwrap();
    assert_eq!(values.compact_f32(value).unwrap().unwrap(), LANES);
    values.release(value).unwrap();
    assert!(values.compact_f32(value).is_err());
    bytes.extend_from_slice(&LANES);
    let mut next = values.swap_owned_compact_f32(&owner, &mut bytes).unwrap();
    let next_value = next.value().unwrap();
    assert_eq!(value.slot, next_value.slot);
    assert_ne!(value.generation, next_value.generation);
    values.commit_owned_compact_f32(&owner, &mut next).unwrap();
    values.release(next_value).unwrap();
    assert_eq!(values.used_items(), 0);
}
#[test]
fn wrong_owner_denial_preserves_pending_ticket_and_both_buffers() {
    let (mut values, owner) = store();
    let (mut foreign, alien) = store();
    let mut bytes = LANES.to_vec();
    assert!(foreign.swap_owned_compact_f32(&owner, &mut bytes).is_err());
    assert!(values.swap_owned_compact_f32(&alien, &mut bytes).is_err());
    assert_eq!(bytes, LANES);
    let mut pending = values.swap_owned_compact_f32(&owner, &mut bytes).unwrap();
    assert!(values
        .restore_owned_compact_f32(&alien, &mut pending, &mut bytes)
        .is_err());
    assert!(pending.value().is_ok());
    values
        .restore_owned_compact_f32(&owner, &mut pending, &mut bytes)
        .unwrap();
    assert_eq!(bytes, LANES);
    assert_eq!(foreign.used_items(), 0);
    assert_eq!(values.used_items(), 0);
}
#[test]
fn invalid_batch_and_unadmitted_buffer_leave_store_truth_unchanged() {
    let mut values = HostedValueStore::new(2, 24, 48).unwrap();
    let backing = values.reserved_storage_bytes();
    assert!(values
        .prepare_owned_compact_f32_outputs(&[
            (layout(), 1),
            (
                CompactF32Layout {
                    lanes: 0,
                    logical_bytes: 20
                },
                1
            )
        ])
        .is_err());
    assert_eq!(values.reserved_storage_bytes(), backing);
    assert!(values.owned_compact_slots.is_empty());
    let owner = values
        .prepare_owned_compact_f32_outputs(&[(layout(), 1)])
        .unwrap()
        .remove(0);
    for mut bytes in [vec![0; 4], f32::INFINITY.to_le_bytes().repeat(2)] {
        let before = bytes.clone();
        assert!(values.swap_owned_compact_f32(&owner, &mut bytes).is_err());
        assert_eq!(bytes, before);
        assert_eq!(values.used_items(), 0);
    }
}
#[test]
fn logical_pressure_and_retired_generations_refuse_before_buffer_transfer() {
    let mut values = HostedValueStore::new(3, 24, 24).unwrap();
    let owner = values
        .prepare_owned_compact_f32_outputs(&[(layout(), 2)])
        .unwrap()
        .remove(0);
    let mut first = LANES.to_vec();
    let mut pending = values.swap_owned_compact_f32(&owner, &mut first).unwrap();
    let mut second = LANES.to_vec();
    let pointer = second.as_ptr();
    assert!(matches!(
        values.swap_owned_compact_f32(&owner, &mut second),
        Err(StorageError::ByteCapacityExceeded)
    ));
    assert_eq!(second.as_ptr(), pointer);
    assert_eq!(second, LANES);
    values
        .restore_owned_compact_f32(&owner, &mut pending, &mut first)
        .unwrap();
    for index in &owner.slots {
        values.slots[usize::from(*index)].generation = u16::MAX;
    }
    assert!(matches!(
        values.swap_owned_compact_f32(&owner, &mut second),
        Err(StorageError::GenerationExhausted)
    ));
    assert_eq!(
        values.owned_compact_f32_available(&owner),
        Err(StorageError::GenerationExhausted)
    );
    assert_eq!(second.as_ptr(), pointer);
    assert_eq!(values.used_items(), 0);
}

#[test]
fn numerically_equal_foreign_pending_receipts_cannot_commit_another_store() {
    let (mut a, a_owner) = store();
    let (mut b, b_owner) = store();
    let mut a_bytes = LANES.to_vec();
    let mut b_bytes = LANES.to_vec();
    let mut a_pending = a.swap_owned_compact_f32(&a_owner, &mut a_bytes).unwrap();
    let mut b_pending = b.swap_owned_compact_f32(&b_owner, &mut b_bytes).unwrap();
    assert_eq!(a_pending.value().unwrap(), b_pending.value().unwrap());
    assert!(b
        .commit_owned_compact_f32(&b_owner, &mut a_pending)
        .is_err());
    assert!(b
        .restore_owned_compact_f32(&b_owner, &mut a_pending, &mut b_bytes)
        .is_err());
    b.restore_owned_compact_f32(&b_owner, &mut b_pending, &mut b_bytes)
        .unwrap();
    a.restore_owned_compact_f32(&a_owner, &mut a_pending, &mut a_bytes)
        .unwrap();
    assert_eq!(a_bytes, LANES);
    assert_eq!(b_bytes, LANES);
    assert_eq!(a.used_items(), 0);
    assert_eq!(b.used_items(), 0);
}
