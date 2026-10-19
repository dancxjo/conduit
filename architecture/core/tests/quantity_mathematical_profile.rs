//! Explicit form negotiation never grants symbolic data to old rational ports.
use conduit_core::*;
#[path = "common/numerical_allocation.rs"]
mod allocation_probe;
use allocation_probe::observe;
fn work() -> ExactNumericWork {
    ExactNumericWork::new(200_000_000)
}
fn wire(symbolic: bool) -> Vec<u8> {
    let value = if symbolic {
        MathematicalValue::parse_literal("pi", &mut work()).unwrap()
    } else {
        MathematicalValue::rational(ExactRational::ratio(1, 3, &mut work()).unwrap())
    };
    let mut coordinate = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    let n = value.encode(&mut coordinate).unwrap();
    let view = MathematicalValueView::decode(&coordinate[..n], &mut work()).unwrap();
    let mut bytes = vec![0; MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES];
    let n = encode_mathematical_quantity_value(
        &view,
        Unit::Meter,
        QuantityRole::Linear,
        &mut bytes,
        &mut work(),
    )
    .unwrap();
    bytes.truncate(n);
    bytes
}
#[test]
fn compatibility_matrix_preserves_old_grants_and_canonical_wire_property() {
    use QuantityValueProfile::{Decimal as D, Exact as E, Mathematical as M};
    for (consumer, row) in [
        (D, [true, false, false]),
        (E, [true, true, false]),
        (M, [true, true, true]),
    ] {
        for (producer, accept) in [D, E, M].into_iter().zip(row) {
            assert_eq!(consumer.accepts(producer), accept);
        }
    }
    assert_eq!(
        M.maximum_encoded_bytes(),
        MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES
    );
    for (profile, versions) in [
        (D, [true, false, false, false]),
        (E, [true, true, false, false]),
        (M, [true, true, true, false]),
    ] {
        for (version, allowed) in (1..=4).zip(versions) {
            assert_eq!(profile.admits_version(version), allowed);
        }
        assert!(!profile.admits_version(0));
    }
    let ports =
        QuantityWireContract::new(vec![("source".into(), M)], vec![("result".into(), E)]).unwrap();
    let property = ports.to_property().unwrap();
    let decoded = QuantityWireContract::from_property(&property).unwrap();
    assert_eq!(decoded, ports);
    assert_eq!(decoded.to_property().unwrap(), property);
    assert_eq!(decoded.input(&"source".into()), M);
    assert_eq!(decoded.input(&"omitted".into()), D);
    assert_eq!(decoded.output(&"result".into()), E);
    assert!(!decoded
        .output(&"result".into())
        .accepts(decoded.input(&"source".into())));
    // The optional view stores a borrowed slice, not an owning symbolic DAG.
    assert!(core::mem::size_of::<QuantityValueView<'_>>() <= core::mem::size_of::<Unit>() + 64);
}
#[test]
fn selected_mathematical_form_retains_typing_and_refuses_unsupported_rational_materialization() {
    let rational = wire(false);
    let symbolic = wire(true);
    let contract = CheckedValueContract::new(
        kind_id(DISTANCE_INFO_ID),
        MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES as u32,
        vec![],
    )
    .unwrap();
    let bounded = CheckedValueContract::new(
        kind_id(DISTANCE_INFO_ID),
        MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES as u32,
        vec![ValueConstraint::QuantityRange {
            minimum: Some(Box::new(Quantity::parse_plot_literal("0m").unwrap())),
            maximum: None,
            minimum_endpoint: IntervalEndpoint::Inclusive,
            maximum_endpoint: IntervalEndpoint::Inclusive,
        }],
    )
    .unwrap();
    let (_, allocations) = observe(|| {
        for (bytes, is_symbolic) in [(&rational, false), (&symbolic, true)] {
            let value = QuantityValueProfile::Mathematical
                .decode(DISTANCE_INFO_ID, bytes, &mut work())
                .unwrap();
            assert_eq!(value.encoded(), bytes);
            assert_eq!(value.unit(), Unit::Meter);
            assert_eq!(value.role(), QuantityRole::Linear);
            assert_eq!(
                value.representation(),
                QuantityCoordinateRepresentation::Mathematical
            );
            assert!(value.decimal().is_none());
            if is_symbolic {
                assert!(matches!(
                    value.rational(&mut work()),
                    Err(QuantityValueRefusal::Mathematical(
                        MathematicalQuantityRefusal::NonRationalCoordinate
                    ))
                ));
            } else {
                assert_eq!(
                    value.rational(&mut work()).unwrap().coordinate(),
                    ExactRational::ratio(1, 3, &mut work()).unwrap()
                );
            }
            assert!(contract
                .validate_with_quantity_profile(bytes, QuantityValueProfile::Mathematical)
                .is_ok());
            // Exact rational comparison is reused; unresolved symbolic bounds
            // remain a truthful refusal rather than a numeric projection.
            let admission =
                bounded.validate_with_quantity_profile(bytes, QuantityValueProfile::Mathematical);
            if is_symbolic {
                assert_eq!(
                    admission,
                    Err(QuantityProfileContractRefusal::Contract(
                        ValueConstraintRefusal::QuantityRange
                    ))
                );
            } else {
                assert!(admission.is_ok());
            }
            assert!(contract.validate(bytes).is_err());
            assert!(QuantityValueView::decode(bytes, &mut work()).is_err());
            assert_eq!(
                QuantityValueProfile::Mathematical
                    .decode(FREQUENCY_INFO_ID, bytes, &mut work())
                    .unwrap_err(),
                QuantityValueContractRefusal::WrongFamily
            );
            for profile in [QuantityValueProfile::Decimal, QuantityValueProfile::Exact] {
                assert!(matches!(
                    profile.decode(DISTANCE_INFO_ID, bytes, &mut work()),
                    Err(QuantityValueContractRefusal::UnsupportedRepresentation { version: 3, .. })
                ));
            }
        }
    });
    assert_eq!(allocations, 0);
}

#[test]
fn mathematical_profile_accepts_legacy_forms_and_refuses_unknown_or_unfunded_data() {
    let native = Quantity::parse_plot_literal("30cm").unwrap();
    let decimal = native.encode();
    let rational = RationalQuantity::from_quantity(native, &mut work()).unwrap();
    let mut exact = vec![0; QUANTITY_VALUE_MAX_ENCODED_BYTES];
    let n = encode_rational_quantity_value(rational, &mut exact).unwrap();
    exact.truncate(n);
    let mut unknown = wire(true);
    unknown[0] = 4;
    let unfunded = wire(true);
    let mut oversized = vec![0; MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES + 1];
    oversized[0] = 3;
    let (_, allocations) = observe(|| {
        for bytes in [decimal.as_slice(), exact.as_slice()] {
            let view = QuantityValueProfile::Mathematical
                .decode(DISTANCE_INFO_ID, bytes, &mut work())
                .unwrap();
            assert_eq!(view.rational(&mut work()).unwrap(), rational);
            assert_eq!(view.project_decimal(&mut work()).unwrap(), native);
        }
        let mut zero = ExactNumericWork::new(0);
        assert_eq!(
            QuantityValueProfile::Mathematical
                .decode(DISTANCE_INFO_ID, &unknown, &mut zero)
                .unwrap_err(),
            QuantityValueContractRefusal::UnsupportedRepresentation {
                profile: QuantityValueProfile::Mathematical,
                version: 4
            }
        );
        assert_eq!(zero.spent(), 0);
        assert_eq!(
            QuantityValueProfile::Mathematical
                .decode(DISTANCE_INFO_ID, &oversized, &mut zero)
                .unwrap_err(),
            QuantityValueContractRefusal::ExtentExceeded
        );
        assert!(QuantityValueProfile::Mathematical
            .decode(DISTANCE_INFO_ID, &unfunded, &mut zero)
            .is_err());
    });
    assert_eq!(allocations, 0);
}

fn rational_wire(coordinate: ExactRational, unit: Unit, role: QuantityRole) -> Vec<u8> {
    let value = MathematicalValue::rational(coordinate);
    let mut numeric = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    let n = value.encode(&mut numeric).unwrap();
    let view = MathematicalValueView::decode(&numeric[..n], &mut work()).unwrap();
    let mut encoded = vec![0; MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES];
    let n =
        encode_mathematical_quantity_value(&view, unit, role, &mut encoded, &mut work()).unwrap();
    encoded.truncate(n);
    encoded
}
#[test]
fn rational_mathematical_ranges_match_existing_physical_comparisons_and_endpoints() {
    let contract = CheckedValueContract::new(
        kind_id(DISTANCE_INFO_ID),
        MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES as u32,
        vec![ValueConstraint::QuantityRange {
            minimum: Some(Box::new(Quantity::parse_plot_literal("25cm").unwrap())),
            maximum: Some(Box::new(Quantity::parse_plot_literal("50cm").unwrap())),
            minimum_endpoint: IntervalEndpoint::Inclusive,
            maximum_endpoint: IntervalEndpoint::Exclusive,
        }],
    )
    .unwrap();
    let mut cases = Vec::new();
    for (n, d, unit, accept) in [
        (1, 3, Unit::Meter, true),
        (1, 4, Unit::Meter, true),
        (1, 2, Unit::Meter, false),
        (1, 6, Unit::Meter, false),
        (25, 1, Unit::Centimeter, true),
        (50, 1, Unit::Centimeter, false),
        (30, 1, Unit::Centimeter, true),
    ] {
        let coordinate = ExactRational::ratio(n, d, &mut work()).unwrap();
        let v3 = rational_wire(coordinate, unit, QuantityRole::Linear);
        let value = RationalQuantity::new(coordinate, unit, QuantityRole::Linear).unwrap();
        let mut v2 = vec![0; QUANTITY_VALUE_MAX_ENCODED_BYTES];
        let n = encode_rational_quantity_value(value, &mut v2).unwrap();
        v2.truncate(n);
        cases.push((v3, v2, accept));
    }
    let symbolic = wire(true);
    let (_, allocations) = observe(|| {
        for (v3, v2, accept) in &cases {
            let actual =
                contract.validate_with_quantity_profile(v3, QuantityValueProfile::Mathematical);
            let reference =
                contract.validate_with_quantity_profile(v2, QuantityValueProfile::Exact);
            assert_eq!(actual, reference);
            assert_eq!(actual.is_ok(), *accept);
            assert!(contract.validate(v3).is_err());
        }
        assert_eq!(
            contract.validate_with_quantity_profile(&symbolic, QuantityValueProfile::Mathematical),
            Err(QuantityProfileContractRefusal::Contract(
                ValueConstraintRefusal::QuantityRange
            ))
        );
        // A prior refusal does not change later constraint outcomes.
        assert!(contract
            .validate_with_quantity_profile(&cases[0].0, QuantityValueProfile::Mathematical)
            .is_ok());
    });
    assert_eq!(allocations, 0);
}
