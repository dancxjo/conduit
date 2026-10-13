use conduit_core::{
    CheckedTextPattern, CheckedValueContract, TEXT_INFO_ID, TextPatternState,
    TextPatternTransition, ValueConstraint, ValueConstraintRefusal, kind_id,
};

fn lowercase_z_pattern() -> CheckedTextPattern {
    CheckedTextPattern::new(
        vec![
            TextPatternState {
                accepting: false,
                transitions: vec![TextPatternTransition {
                    first_scalar: 'z' as u32,
                    last_scalar: 'z' as u32,
                    target_state: 1,
                }],
            },
            TextPatternState {
                accepting: true,
                transitions: vec![],
            },
        ],
        0,
        8,
        8,
    )
    .unwrap()
}

fn exact_contract() -> CheckedValueContract {
    CheckedValueContract::new(
        kind_id(TEXT_INFO_ID),
        8,
        vec![
            ValueConstraint::CanonicalMembership {
                members: vec![b"admin".to_vec(), b"root".to_vec()],
                negated: true,
            },
            ValueConstraint::TextPattern {
                pattern: lowercase_z_pattern(),
                anchored_start: false,
                anchored_end: true,
                negated: true,
            },
        ],
    )
    .unwrap()
}

#[test]
fn exact_negated_constraint_profile_round_trips_across_portable_and_json_carriers() {
    let contract = exact_contract();

    let portable = postcard::to_allocvec(&contract).unwrap();
    let portable_round_trip: CheckedValueContract = postcard::from_bytes(&portable).unwrap();
    assert_eq!(portable_round_trip, contract);
    assert_eq!(
        portable_round_trip.identity_bytes(),
        contract.identity_bytes()
    );

    let json = serde_json::to_vec(&contract).unwrap();
    let json_round_trip: CheckedValueContract = serde_json::from_slice(&json).unwrap();
    assert_eq!(json_round_trip, contract);
    assert_eq!(json_round_trip.identity_bytes(), contract.identity_bytes());

    assert_eq!(contract.validate(b"guest"), Ok(()));
    assert_eq!(
        contract.validate(b"root"),
        Err(ValueConstraintRefusal::Membership)
    );
    assert_eq!(
        contract.validate(b"endsz"),
        Err(ValueConstraintRefusal::TextPattern)
    );
}

#[test]
fn every_builtin_physical_family_accepts_its_exact_quantity_range() {
    use conduit_core::*;
    for (kind, unit) in [
        (DISTANCE_INFO_ID, Unit::Meter),
        (DURATION_INFO_ID, Unit::Second),
        (FREQUENCY_INFO_ID, Unit::Hertz),
        (VOLTAGE_INFO_ID, Unit::Volt),
        (TEMPERATURE_INFO_ID, Unit::Kelvin),
        (ANGLE_INFO_ID, Unit::Turn),
        (RATIO_INFO_ID, Unit::One),
        (PIXEL_COUNT_INFO_ID, Unit::Pixel),
    ] {
        let lower = Quantity::new(1, unit.clone());
        let upper = Quantity::new(2, unit.clone());
        assert_eq!(
            validate_primitive_info(kind, lower.encoded()),
            Ok(()),
            "{kind}"
        );
        let contract = CheckedValueContract::new(
            kind_id(kind),
            MAX_QUANTITY_ENCODED_BYTES as u32,
            vec![ValueConstraint::QuantityRange {
                minimum: Some(Box::new(lower)),
                maximum: Some(Box::new(upper)),
                minimum_endpoint: IntervalEndpoint::Inclusive,
                maximum_endpoint: IntervalEndpoint::Inclusive,
            }],
        )
        .unwrap();
        assert_eq!(
            contract.validate(
                Quantity::from_decimal(15, -1, unit.clone())
                    .unwrap()
                    .encoded()
            ),
            Ok(()),
            "{kind}"
        );
        assert_eq!(
            contract.validate(Quantity::new(3, unit).encoded()),
            Err(ValueConstraintRefusal::QuantityRange),
            "{kind}"
        );
    }
}
