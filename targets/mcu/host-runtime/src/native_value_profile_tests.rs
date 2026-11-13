//! Prepared physical ownership, not a Native witness or execution grant.
use super::*;
use crate::execution::ExecutionValues;
use alloc::{boxed::Box, vec};
use conduit_core::{
    kind_id, StructuredFieldType as F, StructuredFieldValue as V, StructuredInfoType as T,
    StructuredInfoValue as I,
};
use conduit_kernel::ValueStorage;
#[test]
fn schema_profiles_fund_actual_backing_and_preserve_logical_quotas_and_clocks() {
    let leaf = T::leaf(kind_id("value/u64")).unwrap();
    let ty = T::record(
        kind_id("test/native-value-storage-profile"),
        vec![F::new("temperature_reading_fahrenheit", leaf.clone()).unwrap()],
    )
    .unwrap();
    let bytes = I::record(
        ty.clone(),
        vec![V::new(
            "temperature_reading_fahrenheit",
            I::leaf(leaf, 42u64.to_le_bytes().to_vec()).unwrap(),
        )
        .unwrap()],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let schema = Box::leak(ty.canonical_bytes().unwrap().into_boxed_slice());
    let profiles = [
        ValueSlotProfile::Schema {
            canonical: schema,
            maximum_bytes: bytes.len() as u32,
        },
        ValueSlotProfile::Canonical(8),
    ];
    let clocks = [(7, 1000, 4)];
    let (mut store, parameters) = prepare_schema(
        &profiles,
        &[ValueSlotProfile::Canonical(8)],
        &[16],
        &clocks,
        4096,
    )
    .unwrap();
    let backing = store.external_backing_bytes().unwrap();
    assert_eq!(
        backing,
        store.canonical_storage().reserved_storage_bytes() + store.reserved_node_storage_bytes()
    );
    assert!(backing <= 4096);
    let (ordinary, _) = prepare(&[bytes.len() as u32, 8], &[8], &[16], &clocks, 4096).unwrap();
    assert!(backing < ordinary.reserved_storage_bytes());
    assert_eq!(store.item_capacity(), 5);
    assert_eq!(store.byte_capacity(), bytes.len() as u32 + 40);
    assert_eq!(parameters[0].0, 7);
    assert_eq!(store.get(parameters[0].1).unwrap(), 1000u64.to_le_bytes());
    let value = store.store_schema_canonical(schema, &bytes).unwrap();
    assert_eq!(value.byte_len, bytes.len() as u32);
    assert!(store.get(value).is_err());
    let parts = store.canonical_parts(value).unwrap().unwrap();
    assert_eq!(parts.schema, schema);
    assert_eq!(parts.node, &bytes[schema.len()..]);
    store.release(value).unwrap();
    assert_eq!(store.used_items(), 1);
    assert!(matches!(
        prepare_schema(
            &profiles,
            &[ValueSlotProfile::Canonical(8)],
            &[16],
            &clocks,
            backing - 1
        ),
        Err("value-schema-storage")
    ));
    assert!(matches!(
        prepare_schema(
            &[ValueSlotProfile::Schema {
                canonical: &[],
                maximum_bytes: 8
            }],
            &[ValueSlotProfile::Canonical(8)],
            &[],
            &[],
            4096
        ),
        Err("value-schema")
    ));
    assert!(matches!(
        prepare_schema(
            &profiles,
            &[ValueSlotProfile::Canonical(8)],
            &[16],
            &[(7, 0, 4)],
            4096
        ),
        Err("clock-geometry")
    ));
}

#[test]
fn typed_staging_and_queued_value_have_separate_finite_slots() {
    let ty = T::record(
        kind_id("test/staging"),
        vec![F::new("value", T::leaf(kind_id("value/u64")).unwrap()).unwrap()],
    )
    .unwrap();
    let bytes = I::record(
        ty.clone(),
        vec![V::new(
            "value",
            I::leaf(
                T::leaf(kind_id("value/u64")).unwrap(),
                7u64.to_le_bytes().to_vec(),
            )
            .unwrap(),
        )
        .unwrap()],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let schema = Box::leak(ty.canonical_bytes().unwrap().into_boxed_slice());
    let profile = ValueSlotProfile::Schema {
        canonical: schema,
        maximum_bytes: bytes.len() as u32,
    };
    let (mut store, _) = prepare_schema(&[profile], &[profile], &[8], &[], 4096).unwrap();
    let queued = store.store_schema_canonical(schema, &bytes).unwrap();
    let staged = store.store_schema_canonical(schema, &bytes).unwrap();
    assert_ne!(queued, staged);
    assert!(store.store_schema_canonical(schema, &bytes).is_err());
    store.release(queued).unwrap();
    assert_eq!(
        store.canonical_parts(staged).unwrap().unwrap().schema,
        schema
    );
    let next = store.store_schema_canonical(schema, &bytes).unwrap();
    store.release(staged).unwrap();
    store.release(next).unwrap();
    assert_eq!(store.used_items(), 0);
}
