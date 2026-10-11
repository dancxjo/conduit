//! Exact finite record sequences use the shared canonical codec without growth.
use conduit_core::*;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
thread_local! { static ALLOCATIONS: Cell<Option<usize>> = const { Cell::new(None) }; }
struct Counted;
#[global_allocator]
static ALLOCATOR: Counted = Counted;
fn count() {
    let _ = ALLOCATIONS.try_with(|counter| {
        if let Some(value) = counter.get() {
            counter.set(Some(value + 1));
        }
    });
}
// SAFETY: unchanged allocation arguments/results delegate to System.
unsafe impl GlobalAlloc for Counted {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count();
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count();
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        count();
        unsafe { System.realloc(pointer, layout, size) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
}
fn record(schema: &str) -> (StructuredInfoType, Vec<u8>) {
    let leaf = StructuredInfoType::leaf(kind_id("value/u64")).unwrap();
    let ty = StructuredInfoType::record(
        kind_id(schema),
        vec![StructuredFieldType::new("actual", leaf.clone()).unwrap()],
    )
    .unwrap();
    let encoded = StructuredInfoValue::record(
        ty.clone(),
        vec![StructuredFieldValue::new(
            "actual",
            StructuredInfoValue::leaf(leaf, 256_u64.to_le_bytes().to_vec()).unwrap(),
        )
        .unwrap()],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    (ty, encoded)
}
#[test]
fn zero_one_eight_records_match_owned_codec_and_allocate_nothing_on_reuse() {
    let (element, encoded) = record("test/sequence-item");
    let ty = StructuredInfoType::sequence(element.clone(), 8).unwrap();
    let mut prepared = PreparedStructuredComposer::new(&ty, 4096).unwrap();
    let child = validate_canonical_structured_value(&encoded).unwrap();
    let children = [child; 8];
    let pointer = prepared.collection(&children).unwrap().as_ptr();
    for count in [0, 1, 8, 1, 0, 8] {
        let values = (0..count)
            .map(|_| StructuredInfoValue::from_canonical_bytes(&encoded).unwrap())
            .collect();
        let owned = StructuredInfoValue::sequence(ty.clone(), values)
            .unwrap()
            .canonical_bytes()
            .unwrap();
        ALLOCATIONS.with(|counter| counter.set(Some(0)));
        let actual = prepared.collection(&children[..count]).unwrap();
        assert_eq!(actual, owned);
        assert_eq!(actual.as_ptr(), pointer);
        assert_eq!(ALLOCATIONS.with(|counter| counter.replace(None)), Some(0));
    }
}
#[test]
fn count_pressure_and_foreign_schema_preserve_previous_value() {
    let (element, encoded) = record("test/sequence-item");
    let (_, foreign) = record("test/stale-sequence-item");
    let ty = StructuredInfoType::bounded_sequence(element, 1, 8).unwrap();
    let mut prepared = PreparedStructuredComposer::new(&ty, 4096).unwrap();
    let child = validate_canonical_structured_value(&encoded).unwrap();
    let children = [child; 9];
    let previous = prepared.collection(&children[..1]).unwrap().to_vec();
    for count in [0, 9] {
        assert_eq!(
            prepared.collection(&children[..count]),
            Err(StructuredInfoRefusal::WrongCollectionLength)
        );
        assert_eq!(prepared.encoded(), previous);
    }
    let foreign = validate_canonical_structured_value(&foreign).unwrap();
    assert_eq!(
        prepared.collection(&[foreign]),
        Err(StructuredInfoRefusal::WrongType)
    );
    assert_eq!(prepared.encoded(), previous);
    assert!(prepared.collection(&children[..8]).unwrap().len() > previous.len());
}
#[test]
fn exact_collection_refuses_wrong_count_and_byte_extent_before_mutation() {
    let (element, encoded) = record("test/sequence-item");
    let ty = StructuredInfoType::collection(element.clone(), Some(1)).unwrap();
    let mut exact = PreparedStructuredComposer::new(&ty, 4096).unwrap();
    let child = validate_canonical_structured_value(&encoded).unwrap();
    assert_eq!(
        exact.collection(&[]),
        Err(StructuredInfoRefusal::WrongCollectionLength)
    );
    exact.collection(&[child]).unwrap();
    let sequence = StructuredInfoType::sequence(element, 8).unwrap();
    let mut small =
        PreparedStructuredComposer::new(&sequence, sequence.canonical_bytes().unwrap().len() + 6)
            .unwrap();
    let empty = small.collection(&[]).unwrap().to_vec();
    assert_eq!(
        small.collection(&[child]),
        Err(StructuredInfoRefusal::CanonicalEncodingTooLarge)
    );
    assert_eq!(small.encoded(), empty);
}
#[test]
fn malformed_children_cannot_be_supplied_as_validated_composition_input() {
    let (_, encoded) = record("test/sequence-item");
    for end in [0, encoded.len() - 1] {
        assert!(validate_canonical_structured_value(&encoded[..end]).is_err());
    }
    let mut corrupted = encoded;
    corrupted.push(0);
    assert!(validate_canonical_structured_value(&corrupted).is_err());
}

#[test]
fn aggregate_node_refusal_clears_output_and_restart_allocates_nothing() {
    let leaf = StructuredInfoType::leaf(kind_id("value/empty")).unwrap();
    let mut fields = Vec::new();
    let mut values = Vec::new();
    for index in 0..16 {
        let name = format!("f{index:02}");
        fields.push(StructuredFieldType::new(&name, leaf.clone()).unwrap());
        values.push(
            StructuredFieldValue::new(
                &name,
                StructuredInfoValue::leaf(leaf.clone(), vec![]).unwrap(),
            )
            .unwrap(),
        );
    }
    let element = StructuredInfoType::record(kind_id("test/many-nodes"), fields).unwrap();
    let encoded = StructuredInfoValue::record(element.clone(), values)
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let child = validate_canonical_structured_value(&encoded).unwrap();
    let ty = StructuredInfoType::sequence(element, 1024).unwrap();
    let mut prepared = PreparedStructuredComposer::new(&ty, 262144).unwrap();
    prepared.collection(&[child]).unwrap();
    let children = vec![child; 1024];
    assert!(prepared.collection(&children).is_err());
    assert!(prepared.encoded().is_empty());
    ALLOCATIONS.with(|counter| counter.set(Some(0)));
    assert!(prepared.collection(&[child]).is_ok());
    assert_eq!(ALLOCATIONS.with(|counter| counter.replace(None)), Some(0));
    assert!(validate_canonical_structured_value(prepared.encoded()).is_ok());
}
