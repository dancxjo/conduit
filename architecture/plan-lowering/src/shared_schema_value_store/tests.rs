extern crate std;
use super::*;
use crate::schema_value_store::SchemaValueStore;
use alloc::{vec, vec::Vec};
use conduit_core::{
    kind_id, StructuredFieldType as F, StructuredFieldValue as V, StructuredInfoType as T,
    StructuredInfoValue as I,
};
use conduit_kernel::FixedValueStore;
fn fixture(name: &str, value: u8) -> (Vec<u8>, Vec<u8>) {
    let leaf = T::leaf(kind_id("value/u8")).unwrap();
    let ty = T::record(kind_id(name), vec![F::new("value", leaf.clone()).unwrap()]).unwrap();
    let bytes = I::record(
        ty.clone(),
        vec![V::new("value", I::leaf(leaf, vec![value]).unwrap()).unwrap()],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    (ty.canonical_bytes().unwrap(), bytes)
}
fn canonical() -> FixedValueStore<2, 16> {
    FixedValueStore::new(32).unwrap()
}
fn shared<'a>(
    v: Validator<'a>,
    logical: u16,
    physical: u16,
) -> SharedSchemaValueStore<'a, FixedValueStore<2, 16>> {
    SharedSchemaValueStore::try_new(
        canonical(),
        &[(v, logical)],
        &[(v, physical)],
        v.maximum_bytes() as u32 * u32::from(logical),
        65536,
    )
    .unwrap()
}
#[test]
fn dense_trace_parity_preserves_original_identity_and_canonical_delegation() {
    let (schema, bytes) = fixture("test/shared-physical", 7);
    let v = Validator::new(&schema, bytes.len()).unwrap();
    let mut dense =
        SchemaValueStore::try_new(canonical(), &[(v, 3)], bytes.len() as u32 * 3, 65536).unwrap();
    let mut compact = shared(v, 3, 2);
    assert_eq!(compact.item_capacity(), dense.item_capacity());
    assert_eq!(compact.byte_capacity(), dense.byte_capacity());
    for _ in 0..10 {
        let a = dense.store_schema_canonical(&schema, &bytes).unwrap();
        let b = compact.store_schema_canonical(&schema, &bytes).unwrap();
        assert_eq!(a, b);
        dense.retain(a).unwrap();
        compact.retain(b).unwrap();
        assert_eq!(dense.reference_count(a), compact.reference_count(b));
        let c = dense.store_schema_canonical(&schema, &bytes).unwrap();
        let d = compact.store_schema_canonical(&schema, &bytes).unwrap();
        assert_eq!(c, d);
        let mut out = vec![0; bytes.len()];
        assert_eq!(compact.copy_canonical_into(b, &mut out).unwrap(), bytes);
        dense.release(a).unwrap();
        compact.release(b).unwrap();
        dense.release(a).unwrap();
        compact.release(b).unwrap();
        dense.release(c).unwrap();
        compact.release(d).unwrap();
        assert_eq!(dense.used_items(), compact.used_items());
        assert_eq!(dense.used_bytes(), compact.used_bytes());
        assert_eq!(dense.reference_count(a), compact.reference_count(b));
    }
    let a = dense.store(&[1, 2, 3]).unwrap();
    let b = compact.store(&[1, 2, 3]).unwrap();
    assert_eq!(a, b);
    assert_eq!(dense.get(a), compact.get(b));
    dense.release(a).unwrap();
    compact.release(b).unwrap();
    assert_eq!(
        dense.preflight_canonical_store(3),
        compact.preflight_canonical_store(3)
    );
}
#[test]
fn last_reference_alone_frees_physical_stock_and_pressure_is_atomic() {
    let (schema, bytes) = fixture("test/shared-last-reference", 1);
    let v = Validator::new(&schema, bytes.len()).unwrap();
    let mut store = shared(v, 3, 1);
    let a = store.store_schema_canonical(&schema, &bytes).unwrap();
    store.retain(a).unwrap();
    assert_eq!(store.free_physical_items(&schema), 0);
    assert_eq!(
        store.store_schema_canonical(&schema, &bytes),
        Err(E::ItemCapacityExceeded)
    );
    assert_eq!(store.logical[1].generation, 0);
    store.release(a).unwrap();
    assert_eq!(store.reference_count(a), Ok(1));
    assert_eq!(store.free_physical_items(&schema), 0);
    store.release(a).unwrap();
    assert_eq!(store.free_physical_items(&schema), 1);
    assert_eq!(store.reference_count(a), Err(E::StaleReference));
    let b = store.store_schema_canonical(&schema, &bytes).unwrap();
    assert_eq!(a.slot, b.slot);
    assert_ne!(a.generation, b.generation);
    let backing = store.reserved_node_storage_bytes();
    store.clear();
    assert_eq!(store.free_physical_items(&schema), 1);
    assert_eq!(store.used_items(), 0);
    assert_eq!(store.used_bytes(), 0);
    let c = store.store_schema_canonical(&schema, &bytes).unwrap();
    assert!(c.generation > b.generation);
    assert_eq!(store.reserved_node_storage_bytes(), backing);
}
#[test]
fn all_original_generation_domains_survive_one_physical_buffer() {
    let (schema, bytes) = fixture("test/shared-generation", 2);
    let v = Validator::new(&schema, bytes.len()).unwrap();
    let mut dense =
        SchemaValueStore::try_new(canonical(), &[(v, 2)], bytes.len() as u32 * 2, 65536).unwrap();
    let mut compact = shared(v, 2, 1);
    let mut first = None;
    for _ in 0..=u16::MAX {
        let a = dense.store_schema_canonical(&schema, &bytes).unwrap();
        let b = compact.store_schema_canonical(&schema, &bytes).unwrap();
        assert_eq!(a, b);
        first.get_or_insert(b);
        dense.release(a).unwrap();
        compact.release(b).unwrap();
    }
    assert_eq!(compact.logical[0].generation, u16::MAX);
    assert_eq!(compact.logical[1].generation, 1);
    assert_eq!(compact.free_physical_items(&schema), 1);
    assert_eq!(
        compact.reference_count(first.unwrap()),
        Err(E::StaleReference)
    );
    compact.logical[1].generation = u16::MAX;
    assert_eq!(
        compact.store_schema_canonical(&schema, &bytes),
        Err(E::GenerationExhausted)
    );
    assert_eq!(compact.free_physical_items(&schema), 1);
}
#[test]
fn exact_schema_and_malformed_or_quota_refusals_preserve_owners() {
    let (schema, bytes) = fixture("test/shared-exact", 3);
    let (foreign, other) = fixture("test/shared-foreign", 3);
    let v = Validator::new(&schema, bytes.len()).unwrap();
    let mut store = SharedSchemaValueStore::try_new(
        canonical(),
        &[(v, 3)],
        &[(v, 2)],
        bytes.len() as u32,
        65536,
    )
    .unwrap();
    assert!(!store.supports_schema_storage(&foreign));
    assert_eq!(
        store.store_schema_canonical(&foreign, &other),
        Err(E::RepresentationMismatch)
    );
    assert_eq!(
        store.store_schema_canonical(&schema, &bytes[..bytes.len() - 1]),
        Err(E::RepresentationMismatch)
    );
    assert_eq!(store.logical[0].generation, 0);
    let a = store.store_schema_canonical(&schema, &bytes).unwrap();
    assert_eq!(
        store.store_schema_canonical(&schema, &bytes),
        Err(E::ByteCapacityExceeded)
    );
    assert_eq!(store.logical[1].generation, 0);
    assert_eq!(store.free_physical_items(&schema), 1);
    store.logical[0].references = u16::MAX;
    assert_eq!(store.retain(a), Err(E::ReferenceOverflow));
    store.logical[0].references = 1;
    store.release(a).unwrap();
    assert_eq!(store.used_bytes(), 0);
}
#[test]
fn cold_funding_covers_actual_metadata_and_payload_with_no_claim_of_admission() {
    let (schema, bytes) = fixture("test/shared-funding", 4);
    let v = Validator::new(&schema, bytes.len()).unwrap();
    let store = shared(v, 3, 1);
    let backing = store.reserved_node_storage_bytes();
    assert_eq!(
        backing,
        store.reserved_metadata_bytes() + store.reserved_payload_bytes()
    );
    assert!(matches!(
        SharedSchemaValueStore::try_new(
            canonical(),
            &[(v, 3)],
            &[(v, 1)],
            bytes.len() as u32 * 3,
            backing - 1
        ),
        Err(PreparationError::Capacity { .. })
    ));
    assert!(SharedSchemaValueStore::try_new(
        canonical(),
        &[(v, 3)],
        &[(v, 1)],
        bytes.len() as u32 * 3,
        backing
    )
    .is_ok());
    assert!(matches!(
        SharedSchemaValueStore::try_new(canonical(), &[(v, 3)], &[], bytes.len() as u32 * 3, 65536),
        Err(PreparationError::Storage(E::InvalidBudget))
    ));
    std::eprintln!(
        "shared schema backing metadata={} payload={} logical={} physical={}",
        store.reserved_metadata_bytes(),
        store.reserved_payload_bytes(),
        store.item_capacity(),
        store.physical_item_capacity(&schema)
    );
}

#[test]
fn heterogeneous_logical_bounds_keep_dense_best_fit_and_original_order() {
    let leaf = T::leaf(kind_id("value/bytes")).unwrap();
    let ty = T::record(
        kind_id("test/shared-variable"),
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
    let schema = ty.canonical_bytes().unwrap();
    let small = value(1);
    let large = value(256);
    let short = Validator::new(&schema, small.len()).unwrap();
    let long = Validator::new(&schema, large.len()).unwrap();
    for reverse in [false, true] {
        let mut definitions = [(short, 1), (long, 1)];
        if reverse {
            definitions.reverse();
        }
        let quota = (small.len() + large.len()) as u32;
        let mut dense = SchemaValueStore::try_new(canonical(), &definitions, quota, 65536).unwrap();
        let mut compact =
            SharedSchemaValueStore::try_new(canonical(), &definitions, &[(long, 2)], quota, 65536)
                .unwrap();
        let a = dense.store_schema_canonical(&schema, &small).unwrap();
        let b = compact.store_schema_canonical(&schema, &small).unwrap();
        assert_eq!(a, b);
        let c = dense.store_schema_canonical(&schema, &large).unwrap();
        let d = compact.store_schema_canonical(&schema, &large).unwrap();
        assert_eq!(c, d);
        assert!(
            compact.logical[compact.index(b).unwrap()].maximum_bytes
                < compact.logical[compact.index(d).unwrap()].maximum_bytes
        );
        dense.release(a).unwrap();
        compact.release(b).unwrap();
        dense.release(c).unwrap();
        compact.release(d).unwrap();
        assert_eq!(dense.used_items(), compact.used_items());
        assert_eq!(
            dense.store_schema_canonical(&schema, &large),
            compact.store_schema_canonical(&schema, &large)
        );
    }
}

#[test]
fn held_state_queue_and_staged_replacement_need_independently_funded_stock() {
    let (schema, bytes) = fixture("test/shared-held-stock", 8);
    let v = Validator::new(&schema, bytes.len()).unwrap();
    for physical in [2, 3] {
        let mut store = shared(v, 5, physical);
        let held = store.store_schema_canonical(&schema, &bytes).unwrap();
        let queued = store.store_schema_canonical(&schema, &bytes).unwrap();
        let staged = store.store_schema_canonical(&schema, &bytes);
        if physical == 2 {
            assert_eq!(staged, Err(E::ItemCapacityExceeded));
            assert_eq!(store.logical[2].generation, 0);
        } else {
            let staged = staged.unwrap();
            // A rejected materialization/sign commit returns only its new
            // output; held state and the unconsumed queue are still owned.
            store.release(staged).unwrap();
            assert_eq!(store.reference_count(held), Ok(1));
            assert_eq!(store.reference_count(queued), Ok(1));
        }
        store.release(queued).unwrap();
        store.release(held).unwrap();
        assert_eq!(store.free_physical_items(&schema), physical);
    }
}
