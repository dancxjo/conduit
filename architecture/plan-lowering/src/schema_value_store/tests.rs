extern crate std;
use super::*;
use alloc::{vec, vec::Vec};
use conduit_core::{
    kind_id, StructuredFieldType as F, StructuredFieldValue as V, StructuredInfoType as T,
    StructuredInfoValue as I,
};
use conduit_kernel::FixedValueStore;
fn fixture(name: &str, n: u8) -> (Vec<u8>, Vec<u8>) {
    let leaf = T::leaf(kind_id("value/u8")).unwrap();
    let ty = T::record(
        kind_id(name),
        vec![F::new("reading", leaf.clone()).unwrap()],
    )
    .unwrap();
    let value = I::record(
        ty.clone(),
        vec![V::new("reading", I::leaf(leaf, vec![n]).unwrap()).unwrap()],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    (ty.canonical_bytes().unwrap(), value)
}
fn canonical() -> FixedValueStore<2, 16> {
    FixedValueStore::new(32).unwrap()
}
#[test]
fn exact_schema_pool_keeps_logical_custody_and_reconstructs_canonical_boundaries() {
    let (schema, bytes) = fixture("test/shared-schema", 42);
    let validator = Validator::new(&schema, bytes.len()).unwrap();
    let checked = validator.validate(&bytes).unwrap();
    let mut store =
        SchemaValueStore::try_new(canonical(), &[(validator, 2)], bytes.len() as u32 * 2, 4096)
            .unwrap();
    let input = store.store_structured(checked).unwrap();
    assert_eq!(input.byte_len, bytes.len() as u32);
    assert_eq!(store.used_bytes(), input.byte_len);
    assert_eq!(store.used_items(), 1);
    assert_eq!(store.get(input), Err(E::RepresentationMismatch));
    let parts = store.canonical_parts(input).unwrap().unwrap();
    assert_eq!(parts.schema, schema);
    assert_eq!(parts.node, checked.value_node());
    let mut output = vec![0xa5; bytes.len()];
    assert_eq!(
        store.copy_canonical_into(input, &mut output).unwrap(),
        bytes
    );
    output.fill(0xa5);
    assert_eq!(
        store.copy_canonical_into(input, &mut output[..bytes.len() - 1]),
        Err(E::ValueTooLarge)
    );
    assert!(output.iter().all(|b| *b == 0xa5));
    store.retain(input).unwrap();
    store.release(input).unwrap();
    assert_eq!(store.reference_count(input), Ok(1));
    let raw = store.store(&[9; 8]).unwrap();
    assert_eq!(store.copy_canonical_into(raw, &mut output).unwrap(), [9; 8]);
    store.release(input).unwrap();
    assert_eq!(store.canonical_parts(input).err(), Some(E::StaleReference));
    let next = store.store_structured(checked).unwrap();
    assert_ne!(next, input);
    store.clear();
    assert_eq!(store.used_bytes(), 0);
    assert_eq!(store.used_items(), 0);
    assert_eq!(store.reference_count(next), Err(E::StaleReference));
}
#[test]
fn foreign_schemas_pressure_and_exhausted_generations_refuse_without_mutation() {
    let (schema, bytes) = fixture("test/accepted", 7);
    let (foreign, other) = fixture("test/foreign", 7);
    let validator = Validator::new(&schema, bytes.len()).unwrap();
    let foreign_validator = Validator::new(&foreign, other.len()).unwrap();
    let mut store =
        SchemaValueStore::try_new(canonical(), &[(validator, 1)], bytes.len() as u32, 4096)
            .unwrap();
    assert_eq!(
        store.store_structured(foreign_validator.validate(&other).unwrap()),
        Err(E::RepresentationMismatch)
    );
    assert_eq!(store.used_bytes(), 0);
    let value = store
        .store_structured(validator.validate(&bytes).unwrap())
        .unwrap();
    assert_eq!(
        store.store_structured(validator.validate(&bytes).unwrap()),
        Err(E::ByteCapacityExceeded)
    );
    assert_eq!(store.reference_count(value), Ok(1));
    store.slots[0].references = u16::MAX;
    assert_eq!(store.retain(value), Err(E::ReferenceOverflow));
    store.slots[0].references = 1;
    store.release(value).unwrap();
    store.slots[0].generation = u16::MAX;
    assert_eq!(
        store.store_structured(validator.validate(&bytes).unwrap()),
        Err(E::GenerationExhausted)
    );
    assert_eq!(store.used_items(), 0);
    assert_eq!(store.used_bytes(), 0);
}
#[test]
fn preparation_funds_all_metadata_and_nodes_before_publication() {
    let (schema, bytes) = fixture("test/funding", 1);
    let validator = Validator::new(&schema, bytes.len()).unwrap();
    let selected =
        SchemaValueStore::try_new(canonical(), &[(validator, 3)], bytes.len() as u32 * 3, 4096)
            .unwrap();
    let backing = selected.reserved_node_storage_bytes();
    assert!(matches!(
        SchemaValueStore::try_new(
            canonical(),
            &[(validator, 3)],
            bytes.len() as u32 * 3,
            backing - 1
        ),
        Err(PreparationError::Capacity { .. })
    ));
    for (defs, quota) in [
        (&[][..], 1),
        (&[(validator, 0)][..], 1),
        (&[(validator, 1)][..], 0),
        (&[(validator, 1)][..], bytes.len() as u32 + 1),
    ] {
        assert!(matches!(
            SchemaValueStore::try_new(canonical(), defs, quota, 4096),
            Err(PreparationError::Storage(E::InvalidBudget))
        ));
    }
}
#[path = "scheduler_tests.rs"]
mod scheduler;

#[test]
fn explicit_producer_publication_checks_schema_before_mutating_the_pool() {
    let (schema, bytes) = fixture("test/producer-schema", 42);
    let (foreign, other) = fixture("test/foreign-schema", 42);
    let validator = Validator::new(&schema, bytes.len()).unwrap();
    let mut store =
        SchemaValueStore::try_new(canonical(), &[(validator, 1)], bytes.len() as u32, 4096)
            .unwrap();
    assert!(store.supports_schema_storage(&schema));
    assert!(!store.supports_schema_storage(&foreign));
    for (expected, payload) in [
        (&foreign[..], &bytes[..]),
        (&schema[..], &other[..]),
        (&schema[..], &bytes[..bytes.len() - 1]),
    ] {
        assert_eq!(
            store.store_schema_canonical(expected, payload),
            Err(E::RepresentationMismatch)
        );
        assert_eq!(store.used_items(), 0);
        assert_eq!(store.used_bytes(), 0);
    }
    let value = store.store_schema_canonical(&schema, &bytes).unwrap();
    assert_eq!(store.get(value), Err(E::RepresentationMismatch));
    assert_eq!(value.byte_len, bytes.len() as u32);
    assert_eq!(
        store.store_schema_canonical(&schema, &bytes),
        Err(E::ByteCapacityExceeded)
    );
    assert_eq!(store.used_items(), 1);
    store.release(value).unwrap();
    assert_eq!(store.used_bytes(), 0);
}

fn variable_values() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let leaf = T::leaf(kind_id("value/bytes")).unwrap();
    let ty = T::record(
        kind_id("test/variable-schema"),
        vec![F::new("payload", leaf.clone()).unwrap()],
    )
    .unwrap();
    let value = |length| {
        I::record(
            ty.clone(),
            vec![V::new("payload", I::leaf(leaf.clone(), vec![7; length]).unwrap()).unwrap()],
        )
        .unwrap()
        .canonical_bytes()
        .unwrap()
    };
    (ty.canonical_bytes().unwrap(), value(1), value(256))
}

#[test]
fn variable_schema_best_fit_and_validation_work_in_both_declaration_orders() {
    let (schema, small, large) = variable_values();
    let small_validator = Validator::new(&schema, small.len()).unwrap();
    let large_validator = Validator::new(&schema, large.len()).unwrap();
    for reverse in [false, true] {
        for producer_api in [false, true] {
            let mut definitions = [(small_validator, 1), (large_validator, 1)];
            if reverse {
                definitions.reverse();
            }
            let mut store = SchemaValueStore::try_new(
                canonical(),
                &definitions,
                (small.len() + large.len()) as u32,
                4096,
            )
            .unwrap();
            let backing = store.reserved_node_storage_bytes();
            let ((first, second), cost) = scheduler::probe::observe(|| {
                let mut publish = |bytes: &[u8]| {
                    if producer_api {
                        store.store_schema_canonical(&schema, bytes)
                    } else {
                        store.store_structured(large_validator.validate(bytes).unwrap())
                    }
                };
                (publish(&small).unwrap(), publish(&large).unwrap())
            });
            assert_eq!(cost.allocations, 0);
            assert_eq!(cost.reallocations, 0);
            assert_eq!(
                store.slots[store.index(first).unwrap()]
                    .validator
                    .maximum_bytes(),
                small.len()
            );
            assert_eq!(
                store.slots[store.index(second).unwrap()]
                    .validator
                    .maximum_bytes(),
                large.len()
            );
            assert_eq!(
                store.canonical_parts(first).unwrap().unwrap().node,
                small_validator.validate(&small).unwrap().value_node()
            );
            assert_eq!(
                store.canonical_parts(second).unwrap().unwrap().node,
                large_validator.validate(&large).unwrap().value_node()
            );
            assert_eq!(store.used_items(), 2);
            assert_eq!(store.used_bytes(), (small.len() + large.len()) as u32);
            assert_eq!(store.reserved_node_storage_bytes(), backing);
            store.release(first).unwrap();
            store.release(second).unwrap();
            assert_eq!(store.used_items(), 0);
            assert_eq!(store.used_bytes(), 0);
        }
    }
}

#[test]
fn schema_choice_keeps_pressure_retirement_and_validation_refusals_atomic() {
    let (schema, small, large) = variable_values();
    let small_validator = Validator::new(&schema, small.len()).unwrap();
    let large_validator = Validator::new(&schema, large.len()).unwrap();
    let mut store = SchemaValueStore::try_new(
        canonical(),
        &[(small_validator, 1), (large_validator, 1)],
        (small.len() + large.len()) as u32,
        4096,
    )
    .unwrap();
    let backing = store.reserved_node_storage_bytes();
    let first = store.store_schema_canonical(&schema, &small).unwrap();
    let before = store
        .slots
        .iter()
        .map(|slot| (slot.generation, slot.references, slot.node.clone()))
        .collect::<Vec<_>>();
    assert_eq!(
        store.store_schema_canonical(&schema, &large[..large.len() - 1]),
        Err(E::RepresentationMismatch)
    );
    assert_eq!(
        store
            .slots
            .iter()
            .map(|slot| (slot.generation, slot.references, slot.node.clone()))
            .collect::<Vec<_>>(),
        before
    );
    assert_eq!(store.reference_count(first), Ok(1));
    assert_eq!(store.used_bytes(), small.len() as u32);
    let second = store.store_schema_canonical(&schema, &small).unwrap();
    let generations = store
        .slots
        .iter()
        .map(|slot| slot.generation)
        .collect::<Vec<_>>();
    assert_eq!(
        store.store_schema_canonical(&schema, &small),
        Err(E::ItemCapacityExceeded)
    );
    assert_eq!(store.used_items(), 2);
    assert_eq!(store.used_bytes(), 2 * small.len() as u32);
    assert_eq!(
        store
            .slots
            .iter()
            .map(|slot| slot.generation)
            .collect::<Vec<_>>(),
        generations
    );
    store.release(second).unwrap();
    let large_index = store.index(second).unwrap();
    store.slots[large_index].generation = u16::MAX;
    assert_eq!(
        store.store_schema_canonical(&schema, &large),
        Err(E::GenerationExhausted)
    );
    assert_eq!(store.used_items(), 1);
    assert_eq!(store.used_bytes(), small.len() as u32);
    assert_eq!(store.reference_count(first), Ok(1));
    assert_eq!(store.slots[large_index].generation, u16::MAX);
    assert_eq!(store.slots[large_index].references, 0);
    assert!(store.slots[large_index].node.is_empty());
    assert_eq!(store.reserved_node_storage_bytes(), backing);
    store.clear();
    assert_eq!(
        store.store_schema_canonical(&schema, &large),
        Err(E::GenerationExhausted)
    );
    assert_eq!(store.used_items(), 0);
    assert_eq!(store.used_bytes(), 0);
    // A known schema is not permission to exceed every selected envelope.
    let mut oversized = large.clone();
    oversized.push(0);
    assert_eq!(
        store.store_schema_canonical(&schema, &oversized),
        Err(E::RepresentationMismatch)
    );
    assert_eq!(store.slots[large_index].generation, u16::MAX);
}
