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
