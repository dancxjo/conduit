//! Standalone integration-test harness; register only after source-barrier release.
use conduit_core::{
    kind_id, BorrowedStructuredValueValidator as Validator, StructuredFieldType as F,
    StructuredFieldValue as V, StructuredInfoType as T, StructuredInfoValue as I,
};
use conduit_kernel::{FixedValueStore, StorageError, ValueStorage};
use conduit_plan_lowering::shared_schema_value_store::{PreparationError, SharedSchemaValueStore};
mod probe {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../plot/tests/common/allocation_probe.rs"
    ));
}
#[global_allocator]
static ALLOCATOR: probe::Allocator = probe::Allocator;
fn fixture(name: &str) -> (Vec<u8>, Vec<u8>) {
    let leaf = T::leaf(kind_id("value/u8")).unwrap();
    let ty = T::record(kind_id(name), vec![F::new("value", leaf.clone()).unwrap()]).unwrap();
    let value = I::record(
        ty.clone(),
        vec![V::new("value", I::leaf(leaf, vec![7]).unwrap()).unwrap()],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    (ty.canonical_bytes().unwrap(), value)
}
#[test]
fn execution_and_partial_publication_rollback_allocate_nothing() {
    let (schema, bytes) = fixture("test/shared-allocation-a");
    let (other_schema, other) = fixture("test/shared-allocation-b");
    let a = Validator::new(&schema, bytes.len()).unwrap();
    let b = Validator::new(&other_schema, other.len()).unwrap();
    let mut store = SharedSchemaValueStore::try_new(
        FixedValueStore::<2, 16>::new(32).unwrap(),
        &[(a, 3), (b, 3)],
        &[(a, 1), (b, 1)],
        (bytes.len() as u32 + other.len() as u32) * 3,
        65536,
    )
    .unwrap();
    let (_, measured) = probe::observe(|| {
        for _ in 0..50 {
            let first = store.store_schema_canonical(&schema, &bytes).unwrap();
            let second = store.store_schema_canonical(&other_schema, &other).unwrap();
            assert_eq!(
                store.store_schema_canonical(&schema, &bytes),
                Err(StorageError::ItemCapacityExceeded)
            );
            // A later materialization refuses: return every unpublished owned
            // output, just as the kernel's existing rollback does.
            store.release(second).unwrap();
            store.release(first).unwrap();
            assert_eq!(store.used_items(), 0);
            assert_eq!(store.free_physical_items(&schema), 1);
            let value = store.store_schema_canonical(&schema, &bytes).unwrap();
            store.retain(value).unwrap();
            store.release(value).unwrap();
            store.release(value).unwrap();
        }
    });
    assert_eq!(measured.allocations, 0);
    assert_eq!(measured.reallocations, 0);
}
#[test]
fn refused_cold_allocation_is_explicit_and_no_store_escapes() {
    let (schema, bytes) = fixture("test/shared-cold-refusal");
    let v = Validator::new(&schema, bytes.len()).unwrap();
    let (result, measured) = probe::observe_refusing(1, || {
        SharedSchemaValueStore::try_new(
            FixedValueStore::<2, 16>::new(32).unwrap(),
            &[(v, 3)],
            &[(v, 1)],
            bytes.len() as u32 * 3,
            65536,
        )
    });
    assert!(matches!(result, Err(PreparationError::Allocation)));
    assert_eq!(measured.refused_allocations, 1);
}
