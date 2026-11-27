//! Canonical value encoding reserves its complete Type and payload together.
use conduit_core::*;
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn boolean() -> StructuredInfoValue {
    StructuredInfoValue::leaf(
        StructuredInfoType::leaf(kind_id("value/bool")).unwrap(),
        vec![1],
    )
    .unwrap()
}
#[test]
fn every_value_node_encodes_in_one_exact_allocation_without_growth() {
    let leaf = boolean();
    let collection = StructuredInfoValue::collection(
        StructuredInfoType::collection(leaf.value_type().clone(), Some(2)).unwrap(),
        vec![boolean(), boolean()],
    )
    .unwrap();
    let sequence = StructuredInfoValue::sequence(
        StructuredInfoType::bounded_sequence(leaf.value_type().clone(), 0, 2).unwrap(),
        vec![boolean()],
    )
    .unwrap();
    let record = StructuredInfoValue::record(
        StructuredInfoType::record(
            kind_id("fixture/record"),
            vec![StructuredFieldType::new("naïve", leaf.value_type().clone()).unwrap()],
        )
        .unwrap(),
        vec![StructuredFieldValue::new("naïve", boolean()).unwrap()],
    )
    .unwrap();
    let variant = StructuredInfoValue::variant(
        optional_info_type(leaf.value_type().clone()).unwrap(),
        "some",
        boolean(),
    )
    .unwrap();
    let nominal = StructuredInfoValue::nominal(
        StructuredInfoType::nominal(kind_id("fixture/nominal"), leaf.value_type().clone()).unwrap(),
        boolean(),
    )
    .unwrap();
    let opaque = StructuredInfoValue::leaf(
        StructuredInfoType::leaf(kind_id("fixture/opaque")).unwrap(),
        vec![0; 32768],
    )
    .unwrap();
    for value in [leaf, collection, sequence, record, variant, nominal, opaque] {
        let (encoded, allocation) = allocation_probe::observe(|| value.canonical_bytes());
        let encoded = encoded.unwrap();
        assert_eq!(allocation.allocations, 1);
        assert_eq!(allocation.reallocations, 0);
        assert_eq!(allocation.peak_bytes, encoded.len());
        assert_eq!(encoded.capacity(), encoded.len());
        assert_eq!(
            StructuredInfoValue::from_canonical_bytes(&encoded).unwrap(),
            value
        );
    }
}

#[test]
fn profile_identity_keeps_its_exact_bytes_without_formatting_growth() {
    let ty = StructuredInfoType::leaf(kind_id("value/bool")).unwrap();
    let expected = format!(
        "structured-info/profile-{}@1",
        ty.semantic_digest()
            .unwrap()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    let (profile, allocation) = allocation_probe::observe(|| ty.profile());
    let profile = profile.unwrap();
    assert_eq!(profile.value_kind().as_str(), expected);
    assert_eq!(profile.value_kind().0.capacity(), expected.len());
    assert_eq!(allocation.reallocations, 0);
}
