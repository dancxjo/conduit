//! Contract custody must survive strict decoding without changing old bytes.
use super::*;
use alloc::vec;
fn round_trip(kind: &str, maximum: u32, constraint: ValueConstraint) {
    let contract =
        CheckedValueContract::new(crate::kind_id(kind), maximum, vec![constraint]).unwrap();
    let bytes = contract.identity_bytes();
    assert_eq!(
        CheckedValueContract::from_identity_bytes(&bytes),
        Ok(contract)
    );
    for end in 0..bytes.len() {
        assert!(
            CheckedValueContract::from_identity_bytes(&bytes[..end]).is_err(),
            "truncation {end}"
        );
    }
    let mut trailing = bytes;
    trailing.push(0);
    assert!(CheckedValueContract::from_identity_bytes(&trailing).is_err());
}
#[test]
fn every_existing_constraint_retains_its_identity_and_refuses_truncation() {
    let inclusive = IntervalEndpoint::Inclusive;
    round_trip(
        crate::BYTES_INFO_ID,
        8,
        ValueConstraint::ByteLength {
            minimum: 0,
            maximum: 8,
        },
    );
    round_trip(
        crate::COUNT_INFO_ID,
        8,
        ValueConstraint::UnsignedRange {
            minimum: Some(0),
            maximum: Some(9),
            minimum_endpoint: inclusive,
            maximum_endpoint: inclusive,
        },
    );
    round_trip(
        crate::SCALAR_INFO_ID,
        8,
        ValueConstraint::SignedRange {
            minimum: None,
            maximum: Some(9),
            minimum_endpoint: inclusive,
            maximum_endpoint: inclusive,
        },
    );
    round_trip(
        "value/i64",
        8,
        ValueConstraint::FixedIntegerRange {
            minimum: Some((-1_i64).to_le_bytes().to_vec()),
            maximum: Some(9_i64.to_le_bytes().to_vec()),
            minimum_endpoint: inclusive,
            maximum_endpoint: inclusive,
        },
    );
    round_trip(
        crate::QUANTITY_INFO_ID,
        crate::QUANTITY_ENCODED_LEN as u32,
        ValueConstraint::QuantityRange {
            minimum: Some(Box::new(crate::Quantity::parse_plot_literal("0m").unwrap())),
            maximum: Some(Box::new(crate::Quantity::parse_plot_literal("9m").unwrap())),
            minimum_endpoint: inclusive,
            maximum_endpoint: inclusive,
        },
    );
    round_trip(crate::F32_INFO_ID, 4, ValueConstraint::FloatFinite);
    round_trip(
        crate::F32_INFO_ID,
        4,
        ValueConstraint::FloatRange {
            minimum: Some(0_f32.to_le_bytes().to_vec()),
            maximum: Some(9_f32.to_le_bytes().to_vec()),
            minimum_endpoint: inclusive,
            maximum_endpoint: inclusive,
        },
    );
    round_trip(
        crate::COUNT_INFO_ID,
        8,
        ValueConstraint::CanonicalMembership {
            members: vec![0_u64.to_le_bytes().to_vec(), 1_u64.to_le_bytes().to_vec()],
            negated: false,
        },
    );
    round_trip(
        crate::TEXT_INFO_ID,
        8,
        ValueConstraint::TextPattern {
            pattern: CheckedTextPattern::new(
                vec![TextPatternState {
                    accepting: true,
                    transitions: vec![],
                }],
                0,
                0,
                0,
            )
            .unwrap(),
            anchored_start: true,
            anchored_end: true,
            negated: false,
        },
    );
}
#[test]
fn encoded_counts_and_invalid_boolean_tags_refuse_before_unbounded_allocation() {
    let contract =
        CheckedValueContract::new(crate::kind_id(crate::BYTES_INFO_ID), 8, vec![]).unwrap();
    let mut bytes = contract.identity_bytes();
    let n = bytes.len();
    bytes[n - 4..].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(
        CheckedValueContract::from_identity_bytes(&bytes),
        Err(Error::InvalidCanonicalEncoding)
    );
    let contract = CheckedValueContract::new(
        crate::kind_id(crate::COUNT_INFO_ID),
        8,
        vec![ValueConstraint::CanonicalMembership {
            members: vec![0_u64.to_le_bytes().to_vec()],
            negated: false,
        }],
    )
    .unwrap();
    let mut bytes = contract.identity_bytes();
    let prefix = CheckedValueContract::new(contract.value_kind, 8, vec![])
        .unwrap()
        .identity_bytes()
        .len();
    bytes[prefix + 1] = 2;
    assert_eq!(
        CheckedValueContract::from_identity_bytes(&bytes),
        Err(Error::InvalidCanonicalEncoding)
    );
}
