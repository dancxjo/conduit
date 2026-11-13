//! Resident schema/node custody uses the existing canonical borrowed validator.
use conduit_core::*;
#[path = "common/numerical_allocation.rs"]
mod allocation;
const BOOL_TYPE: &[u8] = b"\0\x0a\0\0\0value/bool";
const TRUE_VALUE: &[u8] = b"\0\x0a\0\0\0value/bool\0\x01\0\0\0\x01";

#[test]
fn static_schema_preparation_and_separate_node_validation_allocate_nothing() {
    assert_eq!(
        BOOL_TYPE,
        StructuredInfoType::leaf(kind_id(BOOL_INFO_ID))
            .unwrap()
            .canonical_bytes()
            .unwrap()
    );
    let (_, allocations) = allocation::observe(|| {
        let validator = BorrowedStructuredValueValidator::new(BOOL_TYPE, TRUE_VALUE.len()).unwrap();
        let whole = validator.validate(TRUE_VALUE).unwrap();
        assert_eq!(whole.type_bytes().as_ptr(), BOOL_TYPE.as_ptr());
        let node = &TRUE_VALUE[BOOL_TYPE.len()..];
        let separate = validator.validate_node(node).unwrap();
        assert_eq!(separate, whole);
        assert_eq!(separate.value_node().as_ptr(), node.as_ptr());
        assert_eq!(validator.type_bytes(), BOOL_TYPE);
        assert!(validator.validate_node(&[0, 1, 0, 0, 0, 2]).is_err());
        assert!(validator.validate_node(&[0, 1, 0, 0, 0]).is_err());
    });
    assert_eq!(allocations, 0);
}

#[test]
fn incomplete_trailing_or_unbounded_schemas_refuse_without_allocating() {
    let mut trailing = BOOL_TYPE.to_vec();
    trailing.push(0);
    let (_, allocations) = allocation::observe(|| {
        for end in 0..BOOL_TYPE.len() {
            assert!(BorrowedStructuredValueValidator::new(&BOOL_TYPE[..end], 64).is_err());
        }
        assert!(BorrowedStructuredValueValidator::new(&trailing, 64).is_err());
        for bound in [0, BOOL_TYPE.len(), MAXIMUM_STRUCTURED_CANONICAL_BYTES + 1] {
            assert!(BorrowedStructuredValueValidator::new(BOOL_TYPE, bound).is_err());
        }
        let tight = BorrowedStructuredValueValidator::new(BOOL_TYPE, TRUE_VALUE.len() - 1).unwrap();
        assert_eq!(
            tight.validate(TRUE_VALUE),
            Err(StructuredInfoRefusal::CanonicalEncodingTooLarge)
        );
        assert_eq!(
            tight.validate_node(&TRUE_VALUE[BOOL_TYPE.len()..]),
            Err(StructuredInfoRefusal::CanonicalEncodingTooLarge)
        );
    });
    assert_eq!(allocations, 0);
}

#[test]
fn nested_records_variants_sequences_and_refusals_share_the_canonical_walker() {
    let leaf = StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap();
    let sequence = StructuredInfoType::bounded_sequence(leaf.clone(), 1, 3).unwrap();
    let record = StructuredInfoType::record(
        kind_id("test/state@1"),
        vec![StructuredFieldType::new("flags", sequence.clone()).unwrap()],
    )
    .unwrap();
    let ty = StructuredInfoType::variant(
        kind_id("test/reply@1"),
        vec![StructuredVariantCase::new("ready", record.clone()).unwrap()],
    )
    .unwrap();
    let flags = StructuredInfoValue::sequence(
        sequence,
        vec![StructuredInfoValue::leaf(leaf, vec![1]).unwrap()],
    )
    .unwrap();
    let state = StructuredInfoValue::record(
        record,
        vec![StructuredFieldValue::new("flags", flags).unwrap()],
    )
    .unwrap();
    let encoded = StructuredInfoValue::variant(ty.clone(), "ready", state)
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let prefix = ty.canonical_bytes().unwrap();
    let profile = ty.profile().unwrap().value_kind().clone();
    let mut altered = encoded.clone();
    altered.push(0);
    let (_, allocations) = allocation::observe(|| {
        let validator = BorrowedStructuredValueValidator::new(&prefix, encoded.len() + 1).unwrap();
        assert!(validator.matches_profile(profile.as_str()));
        assert!(!validator.matches_profile(BOOL_INFO_ID));
        assert!(!validator.matches_profile("structured-info/profile-00@1"));
        let value = validator.validate(&encoded).unwrap();
        assert_eq!(
            value,
            validate_canonical_structured_value(&encoded).unwrap()
        );
        let flags = value
            .variant_payload("ready")
            .unwrap()
            .unwrap()
            .record_field("flags")
            .unwrap()
            .unwrap();
        assert_eq!(flags.collection_length().unwrap(), 1);
        assert_eq!(
            flags.collection_index(0).unwrap().unwrap().value_node(),
            &[0, 1, 0, 0, 0, 1]
        );
        for end in 0..encoded.len() {
            assert!(validator.validate(&encoded[..end]).is_err());
        }
        assert!(validator.validate(TRUE_VALUE).is_err());
        assert_eq!(
            validator.validate(&altered),
            validate_canonical_structured_value(&altered)
        );
    });
    assert_eq!(allocations, 0);
}

#[test]
fn borrowed_profile_matches_owned_identity_without_allocating_or_accepting_foreign_types() {
    let ty = StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap();
    let profile = ty.profile().unwrap().value_kind().clone();
    let foreign = StructuredInfoType::leaf(kind_id(COUNT_INFO_ID))
        .unwrap()
        .profile()
        .unwrap()
        .value_kind()
        .clone();
    let (_, allocations) = allocation::observe(|| {
        let validator = BorrowedStructuredValueValidator::new(BOOL_TYPE, TRUE_VALUE.len()).unwrap();
        assert!(validator.matches_profile(profile.as_str()));
        assert!(!validator.matches_profile(foreign.as_str()));
        assert!(!validator.matches_profile("structured-info/profile-ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff@1"));
        assert!(!validator.matches_profile(BOOL_INFO_ID));
    });
    assert_eq!(allocations, 0);
}
