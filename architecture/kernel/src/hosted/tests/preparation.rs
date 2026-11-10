use super::*;
use crate::ValueStorePreparationError as E;
#[test]
fn fallible_canonical_preparation_rejects_invalid_geometry_and_unfunded_backing() {
    for (capacities, live) in [
        (&[][..], 1),
        (&[0][..], 1),
        (&[8][..], 0),
        (&[8][..], 9),
        (&[u32::MAX, 1][..], 1),
    ] {
        assert!(matches!(
            HostedValueStore::try_new_with_slot_capacities(capacities, live, usize::MAX),
            Err(E::Storage(StorageError::InvalidBudget))
        ));
    }
    let bound = HostedValueStore::canonical_storage_bytes([2, 8]).unwrap();
    assert!(
        matches!(HostedValueStore::try_new_with_slot_capacities(&[2,8],10,bound-1),Err(E::Capacity{required,available}) if required==bound && available==bound-1)
    );
    let store = HostedValueStore::try_new_with_slot_capacities(&[2, 8], 10, bound).unwrap();
    assert_eq!(store.reserved_storage_bytes(), bound);
    assert_eq!(store.used_items(), 0);
}
#[test]
fn nonuniform_canonical_preflight_preserves_retired_slots_and_refusal_precedence() {
    let bound = HostedValueStore::canonical_storage_bytes([2, 8]).unwrap();
    let mut store = HostedValueStore::try_new_with_slot_capacities(&[2, 8], 10, bound).unwrap();
    let small = store.store(&[1; 2]).unwrap();
    let large = store.store(&[2; 8]).unwrap();
    assert_eq!(
        store.preflight_canonical_store(9),
        Err(StorageError::ValueTooLarge)
    );
    assert_eq!(
        store.preflight_canonical_store(1),
        Err(StorageError::ByteCapacityExceeded)
    );
    store.release(small).unwrap();
    assert_eq!(
        store.preflight_canonical_store(3),
        Err(StorageError::ByteCapacityExceeded)
    );
    assert_eq!(store.preflight_canonical_store(2), Ok(()));
    store.release(large).unwrap();
    store.slots[1].generation = u16::MAX;
    store.has_retired_slots = true;
    let snapshot = (
        store.used_items(),
        store.used_bytes(),
        store.free_search_start,
        store.slot_probes,
        store.allocation_capacities(),
    );
    assert_eq!(
        store.preflight_canonical_store(8),
        Err(StorageError::GenerationExhausted)
    );
    assert_eq!(
        (
            store.used_items(),
            store.used_bytes(),
            store.free_search_start,
            store.slot_probes,
            store.allocation_capacities()
        ),
        snapshot
    );
    assert_eq!(store.store(&[3; 8]), Err(StorageError::GenerationExhausted));
    assert_eq!(store.preflight_canonical_store(2), Ok(()));
    assert_eq!(store.store(&[3; 2]).unwrap().slot, 0);
}

#[test]
fn canonical_preflight_matches_store_across_bounded_mixed_capacity_states() {
    let bound = HostedValueStore::canonical_storage_bytes([1, 4, 16]).unwrap();
    let mut store = HostedValueStore::try_new_with_slot_capacities(&[1, 4, 16], 15, bound).unwrap();
    let mut held = [None; 3];
    let mut state = 19u32;
    let bytes = [3u8; 19];
    for _ in 0..2000 {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        let index = (state as usize) % 3;
        if state & 3 == 0 {
            if let Some(value) = held[index].take() {
                store.release(value).unwrap();
            }
        }
        let length = ((state >> 8) % 20) as usize;
        let preflight = store.preflight_canonical_store(length as u32);
        let actual = store.store(&bytes[..length]);
        assert_eq!(
            preflight,
            actual.as_ref().map(|_| ()).map_err(|error| *error)
        );
        if let Ok(value) = actual {
            assert!(held[usize::from(value.slot)].replace(value).is_none());
        }
    }
}
