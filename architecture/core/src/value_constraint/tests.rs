//! Existing finite-constraint contract regressions.
use super::*;
use alloc::vec;

fn literal_ab() -> CheckedTextPattern {
    CheckedTextPattern::new(
        vec![
            TextPatternState {
                accepting: false,
                transitions: vec![TextPatternTransition {
                    first_scalar: 'a' as u32,
                    last_scalar: 'a' as u32,
                    target_state: 1,
                }],
            },
            TextPatternState {
                accepting: false,
                transitions: vec![TextPatternTransition {
                    first_scalar: 'b' as u32,
                    last_scalar: 'b' as u32,
                    target_state: 2,
                }],
            },
            TextPatternState {
                accepting: true,
                transitions: Vec::new(),
            },
        ],
        0,
        2,
        2,
    )
    .unwrap()
}

#[test]
fn deterministic_text_pattern_is_full_match_and_work_bounded() {
    let pattern = literal_ab();
    assert!(pattern.is_match("ab"));
    assert!(!pattern.is_match("a"));
    assert!(!pattern.is_match("abc"));
    assert!(!pattern.is_match("xb"));
}

#[test]
fn zero_byte_unit_and_exact_empty_text_remain_expressible() {
    let unit = CheckedValueContract::new(crate::kind_id(crate::EMPTY_INFO_ID), 0, vec![])
        .expect("unit has an exact zero-byte canonical encoding");
    assert_eq!(unit.validate(&[]), Ok(()));

    let empty = CheckedTextPattern::new(
        vec![TextPatternState {
            accepting: true,
            transitions: vec![],
        }],
        0,
        0,
        0,
    )
    .expect("an exact empty-text language requires no matching work");
    assert!(empty.is_match(""));
    assert!(!empty.is_match("x"));
}

#[test]
fn overlapping_transitions_refuse_as_nondeterministic() {
    let error = CheckedTextPattern::new(
        vec![TextPatternState {
            accepting: true,
            transitions: vec![
                TextPatternTransition {
                    first_scalar: 'a' as u32,
                    last_scalar: 'z' as u32,
                    target_state: 0,
                },
                TextPatternTransition {
                    first_scalar: 'm' as u32,
                    last_scalar: 'q' as u32,
                    target_state: 0,
                },
            ],
        }],
        0,
        8,
        16,
    )
    .unwrap_err();
    assert_eq!(
        error,
        ConstraintDefinitionError::NondeterministicTransitions
    );
}

#[test]
fn exact_contract_distinguishes_structure_from_constraint_refusal() {
    let count = CheckedValueContract::new(
        crate::kind_id(crate::COUNT_INFO_ID),
        crate::COUNT_ENCODED_LEN as u32,
        vec![ValueConstraint::UnsignedRange {
            minimum: Some(2),
            maximum: Some(4),
            minimum_endpoint: IntervalEndpoint::Inclusive,
            maximum_endpoint: IntervalEndpoint::Inclusive,
        }],
    )
    .unwrap();
    assert_eq!(count.validate(&crate::encode_count(3)), Ok(()));
    assert_eq!(
        count.validate(&crate::encode_count(7)),
        Err(ValueConstraintRefusal::UnsignedRange)
    );
    assert!(matches!(
        count.validate(&[0]),
        Err(ValueConstraintRefusal::MalformedPrimitive(_))
    ));
}

#[test]
fn signed_and_quantity_ranges_compare_semantic_values() {
    let scalar = CheckedValueContract::new(
        crate::kind_id(crate::SCALAR_INFO_ID),
        crate::SCALAR_ENCODED_LEN as u32,
        vec![ValueConstraint::SignedRange {
            minimum: Some(-2_000_000),
            maximum: Some(2_000_000),
            minimum_endpoint: IntervalEndpoint::Inclusive,
            maximum_endpoint: IntervalEndpoint::Inclusive,
        }],
    )
    .unwrap();
    assert_eq!(scalar.validate(&crate::Scalar::ZERO.encode()), Ok(()));
    assert_eq!(
        scalar.validate(&crate::Scalar::from_raw_microunits(3_000_000).encode()),
        Err(ValueConstraintRefusal::SignedRange)
    );

    let distance = CheckedValueContract::new(
        crate::kind_id(crate::DISTANCE_INFO_ID),
        crate::QUANTITY_ENCODED_LEN as u32,
        vec![ValueConstraint::QuantityRange {
            minimum: Some(crate::Quantity::new(1, crate::Unit::Meter).into()),
            maximum: Some(crate::Quantity::new(2, crate::Unit::Meter).into()),
            minimum_endpoint: IntervalEndpoint::Inclusive,
            maximum_endpoint: IntervalEndpoint::Inclusive,
        }],
    )
    .unwrap();
    assert_eq!(
        distance.validate(&crate::Quantity::new(150, crate::Unit::Centimeter).encode()),
        Ok(())
    );
    assert_eq!(
        distance.validate(&crate::Quantity::new(3, crate::Unit::Meter).encode()),
        Err(ValueConstraintRefusal::QuantityRange)
    );
}

#[test]
fn open_semantic_ranges_keep_finite_carrier_admission_separate() {
    let positive = CheckedValueContract::new(
        crate::kind_id(crate::SCALAR_INFO_ID),
        crate::SCALAR_ENCODED_LEN as u32,
        vec![ValueConstraint::SignedRange {
            minimum: Some(0),
            maximum: None,
            minimum_endpoint: IntervalEndpoint::Inclusive,
            maximum_endpoint: IntervalEndpoint::Inclusive,
        }],
    )
    .unwrap();

    assert_eq!(
        positive.validate(&crate::Scalar::from_raw_microunits(1).encode()),
        Ok(())
    );
    assert_eq!(
        positive.validate(&crate::Scalar::from_raw_microunits(-1).encode()),
        Err(ValueConstraintRefusal::SignedRange)
    );
    assert_eq!(
        positive.validate(&[0; crate::SCALAR_ENCODED_LEN + 1]),
        Err(ValueConstraintRefusal::Oversize {
            actual: (crate::SCALAR_ENCODED_LEN + 1) as u32,
            maximum: crate::SCALAR_ENCODED_LEN as u32,
        })
    );
}

#[test]
fn semantic_openness_has_identity_distinct_from_carrier_extrema() {
    let contract = |maximum| {
        CheckedValueContract::new(
            crate::kind_id(crate::COUNT_INFO_ID),
            crate::COUNT_ENCODED_LEN as u32,
            vec![ValueConstraint::UnsignedRange {
                minimum: Some(0),
                maximum,
                minimum_endpoint: IntervalEndpoint::Inclusive,
                maximum_endpoint: IntervalEndpoint::Inclusive,
            }],
        )
        .unwrap()
    };
    assert_ne!(
        contract(None).identity_bytes(),
        contract(Some(u64::MAX)).identity_bytes()
    );
}

#[test]
fn exclusive_interval_endpoints_are_exact_and_empty_ranges_refuse() {
    let open_count = CheckedValueContract::new(
        crate::kind_id(crate::COUNT_INFO_ID),
        crate::COUNT_ENCODED_LEN as u32,
        vec![ValueConstraint::UnsignedRange {
            minimum: Some(2),
            maximum: Some(4),
            minimum_endpoint: IntervalEndpoint::Exclusive,
            maximum_endpoint: IntervalEndpoint::Exclusive,
        }],
    )
    .unwrap();
    assert_eq!(
        open_count.validate(&crate::encode_count(2)),
        Err(ValueConstraintRefusal::UnsignedRange)
    );
    assert_eq!(open_count.validate(&crate::encode_count(3)), Ok(()));
    assert_eq!(
        open_count.validate(&crate::encode_count(4)),
        Err(ValueConstraintRefusal::UnsignedRange)
    );

    assert_eq!(
        CheckedValueContract::new(
            crate::kind_id(crate::COUNT_INFO_ID),
            crate::COUNT_ENCODED_LEN as u32,
            vec![ValueConstraint::UnsignedRange {
                minimum: Some(2),
                maximum: Some(2),
                minimum_endpoint: IntervalEndpoint::Inclusive,
                maximum_endpoint: IntervalEndpoint::Exclusive,
            }],
        ),
        Err(ConstraintDefinitionError::InvalidUnsignedRange)
    );
}

#[test]
fn quantity_range_refuses_a_mismatched_kind_or_dimension() {
    assert_eq!(
        CheckedValueContract::new(
            crate::kind_id(crate::DISTANCE_INFO_ID),
            crate::QUANTITY_ENCODED_LEN as u32,
            vec![ValueConstraint::QuantityRange {
                minimum: Some(crate::Quantity::new(1, crate::Unit::Second).into()),
                maximum: Some(crate::Quantity::new(2, crate::Unit::Second).into()),
                minimum_endpoint: IntervalEndpoint::Inclusive,
                maximum_endpoint: IntervalEndpoint::Inclusive,
            }],
        ),
        Err(ConstraintDefinitionError::WrongConstraintKind)
    );
    assert_eq!(
        CheckedValueContract::new(
            crate::kind_id(crate::QUANTITY_INFO_ID),
            crate::QUANTITY_ENCODED_LEN as u32,
            vec![ValueConstraint::QuantityRange {
                minimum: Some(crate::Quantity::new(1, crate::Unit::Meter).into()),
                maximum: Some(crate::Quantity::new(2, crate::Unit::Second).into()),
                minimum_endpoint: IntervalEndpoint::Inclusive,
                maximum_endpoint: IntervalEndpoint::Inclusive,
            }],
        ),
        Err(ConstraintDefinitionError::InvalidQuantityRange)
    );
}

#[test]
fn text_contract_combines_bound_membership_and_pattern() {
    let contract = CheckedValueContract::new(
        crate::kind_id(crate::TEXT_INFO_ID),
        2,
        vec![
            ValueConstraint::CanonicalMembership {
                members: vec![b"ab".to_vec(), b"xy".to_vec()],
                negated: false,
            },
            ValueConstraint::TextPattern {
                pattern: literal_ab(),
                anchored_start: true,
                anchored_end: true,
                negated: false,
            },
        ],
    )
    .unwrap();
    assert_eq!(contract.validate(b"ab"), Ok(()));
    assert_eq!(
        contract.validate(b"xy"),
        Err(ValueConstraintRefusal::TextPattern)
    );
    assert_eq!(
        contract.validate(b"zz"),
        Err(ValueConstraintRefusal::Membership)
    );
}

#[test]
fn negated_membership_and_pattern_are_exact_checked_truth() {
    let positive_membership = CheckedValueContract::new(
        crate::kind_id(crate::TEXT_INFO_ID),
        5,
        vec![ValueConstraint::CanonicalMembership {
            members: vec![b"admin".to_vec(), b"root".to_vec()],
            negated: false,
        }],
    )
    .unwrap();
    let negative_membership = CheckedValueContract::new(
        crate::kind_id(crate::TEXT_INFO_ID),
        5,
        vec![ValueConstraint::CanonicalMembership {
            members: vec![b"admin".to_vec(), b"root".to_vec()],
            negated: true,
        }],
    )
    .unwrap();
    assert_eq!(negative_membership.validate(b"guest"), Ok(()));
    assert_eq!(
        negative_membership.validate(b"root"),
        Err(ValueConstraintRefusal::Membership)
    );
    assert_ne!(
        positive_membership.identity_bytes(),
        negative_membership.identity_bytes()
    );

    let positive_pattern = CheckedValueContract::new(
        crate::kind_id(crate::TEXT_INFO_ID),
        2,
        vec![ValueConstraint::TextPattern {
            pattern: literal_ab(),
            anchored_start: true,
            anchored_end: true,
            negated: false,
        }],
    )
    .unwrap();
    let negative_pattern = CheckedValueContract::new(
        crate::kind_id(crate::TEXT_INFO_ID),
        2,
        vec![ValueConstraint::TextPattern {
            pattern: literal_ab(),
            anchored_start: true,
            anchored_end: true,
            negated: true,
        }],
    )
    .unwrap();
    assert_eq!(negative_pattern.validate(b"xy"), Ok(()));
    assert_eq!(
        negative_pattern.validate(b"ab"),
        Err(ValueConstraintRefusal::TextPattern)
    );
    assert_ne!(
        positive_pattern.identity_bytes(),
        negative_pattern.identity_bytes()
    );
}

#[test]
fn incompatible_and_malformed_constraints_refuse_before_play() {
    assert_eq!(
        CheckedValueContract::new(
            crate::kind_id(crate::TEXT_INFO_ID),
            8,
            vec![ValueConstraint::UnsignedRange {
                minimum: Some(0),
                maximum: Some(8),
                minimum_endpoint: IntervalEndpoint::Inclusive,
                maximum_endpoint: IntervalEndpoint::Inclusive,
            }],
        ),
        Err(ConstraintDefinitionError::WrongConstraintKind)
    );
    assert_eq!(
        CheckedValueContract::new(
            crate::kind_id(crate::COUNT_INFO_ID),
            crate::COUNT_ENCODED_LEN as u32,
            vec![ValueConstraint::CanonicalMembership {
                members: vec![vec![0]],
                negated: false,
            }],
        ),
        Err(ConstraintDefinitionError::MalformedMembership)
    );
}
