//! Proposed physical coordinate form; not Source/Plan/installed profile proof.
use conduit_core::*;
#[path = "common/numerical_allocation.rs"]
mod allocation_probe;
use allocation_probe::observe;
type QRefusal = MathematicalQuantityRefusal;
fn work() -> ExactNumericWork {
    ExactNumericWork::new(200_000_000)
}
fn q(n: i128, d: i128) -> ExactRational {
    ExactRational::ratio(n, d, &mut work()).unwrap()
}
fn bytes(value: &MathematicalValue) -> Vec<u8> {
    let mut out = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    let n = value.encode(&mut out).unwrap();
    out.truncate(n);
    out
}
fn frame(value: &[u8], unit: Unit, role: QuantityRole) -> Vec<u8> {
    let view = MathematicalValueView::decode(value, &mut work()).unwrap();
    let mut out = vec![0; MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES];
    let n = encode_mathematical_quantity_value(&view, unit, role, &mut out, &mut work()).unwrap();
    out.truncate(n);
    out
}
fn requirements() -> MathematicalEnclosureRequirements {
    MathematicalEnclosureRequirements {
        maximum_width: q(1, 100_000_000_000_000_000),
        maximum_leaf_width: q(1, 100_000_000_000_000_000_000),
        maximum_constant_terms: 32,
        maximum_root_refinements: 64,
        maximum_scratch_payload_bytes: 131072,
        maximum_retained_payload_bytes: 131072,
    }
}
#[test]
fn checked_angle_coordinate_conversion_then_numeric_enclosure_retains_the_physical_frame() {
    let turn = Quantity::parse_plot_literal("0.25turn").unwrap();
    let degrees = Quantity::parse_plot_literal("90°").unwrap();
    assert_eq!(
        RationalQuantity::from_quantity(turn, &mut work())
            .unwrap()
            .compare(
                RationalQuantity::from_quantity(degrees, &mut work()).unwrap(),
                &mut work()
            )
            .unwrap(),
        core::cmp::Ordering::Equal
    );
    let mut outputs = Vec::new();
    for (source, spelling) in [(turn, "0.25turn"), (degrees, "90°")] {
        let receipt = PreparedAngleReferenceConversion::from_checked(
            source,
            Unit::Radian,
            spelling,
            "rad",
            &mut work(),
        )
        .unwrap();
        assert_eq!(receipt.original(), spelling);
        assert_eq!(receipt.source(), source);
        assert_eq!(receipt.reference_law(), TURN_RADIAN_REFERENCE_LAW);
        assert_eq!(receipt.turns().coordinate(), q(1, 4));
        let coordinate = bytes(receipt.denotation());
        let encoded = frame(&coordinate, Unit::Radian, QuantityRole::Linear);
        let kind = quantity_role_info_id(Unit::Radian.family(), QuantityRole::Linear).unwrap();
        let mut prepared =
            PreparedMathematicalQuantityEnclosure::new(requirements(), 200_000_000).unwrap();
        let retained = prepared.retained_payload_bytes();
        let lo = q(15707963267948966, 10_000_000_000_000_000);
        let hi = q(15707963267948967, 10_000_000_000_000_000);
        let (_, allocations) = observe(|| {
            for _ in 0..4 {
                let view = MathematicalQuantityView::decode(&encoded, &mut work()).unwrap();
                assert_eq!(view.unit(), Unit::Radian);
                assert_eq!(view.role(), QuantityRole::Linear);
                assert_eq!(view.dimension(), source.unit().dimension());
                assert_eq!(view.family(), source.unit().family());
                assert_eq!(view.coordinate().encoded(), coordinate);
                view.validate_kind(&kind).unwrap();
                assert!(matches!(
                    view.rational(&mut work()),
                    Err(QRefusal::NonRationalCoordinate)
                ));
                let result = prepared.execute(&encoded, &mut work()).unwrap();
                assert_eq!(result.source().encoded(), encoded);
                for endpoint in [result.lower(), result.upper()] {
                    assert_eq!(endpoint.unit(), Unit::Radian);
                    assert_eq!(endpoint.role(), QuantityRole::Linear);
                    assert_eq!(endpoint.family(), view.family());
                }
                assert!(!result
                    .lower()
                    .coordinate()
                    .checked_compare(lo, &mut work())
                    .unwrap()
                    .is_lt());
                assert!(!result
                    .upper()
                    .coordinate()
                    .checked_compare(hi, &mut work())
                    .unwrap()
                    .is_gt());
                assert_ne!(result.width_coordinate(), ExactRational::ZERO);
                assert!(!result
                    .width_coordinate()
                    .checked_compare(requirements().maximum_width, &mut work())
                    .unwrap()
                    .is_gt());
                assert_eq!(prepared.retained_payload_bytes(), retained);
            }
        });
        assert_eq!(allocations, 0);
        outputs.push(encoded);
    }
    // Equal physical inputs with distinct retained source spellings produce
    // the same exact radian-coordinate form, never a rational approximation of pi.
    assert_eq!(outputs[0], outputs[1]);
}
#[test]
fn physical_role_guards_and_legacy_port_rejection_are_independent_of_numeric_data() {
    let coordinate = bytes(&MathematicalValue::parse_literal("pi", &mut work()).unwrap());
    let encoded = frame(&coordinate, Unit::Kelvin, QuantityRole::Delta);
    let delta = quantity_role_info_id(Unit::Kelvin.family(), QuantityRole::Delta).unwrap();
    let point = quantity_role_info_id(Unit::Kelvin.family(), QuantityRole::Point).unwrap();
    let length = quantity_role_info_id(Unit::Meter.family(), QuantityRole::Linear).unwrap();
    let (_, allocations) = observe(|| {
        let view = MathematicalQuantityView::decode(&encoded, &mut work()).unwrap();
        view.validate_kind(&delta).unwrap();
        assert_eq!(
            view.validate_kind(&point),
            Err(QuantityValueContractRefusal::WrongRole)
        );
        assert_eq!(
            view.validate_kind(&length),
            Err(QuantityValueContractRefusal::WrongFamily)
        );
        assert_eq!(
            view.validate_kind(MATHEMATICAL_VALUE_ID),
            Err(QuantityValueContractRefusal::NotQuantityKind)
        );
        for profile in [QuantityValueProfile::Decimal, QuantityValueProfile::Exact] {
            let mut zero = ExactNumericWork::new(0);
            assert_eq!(
                profile
                    .decode(QUANTITY_INFO_ID, &encoded, &mut zero)
                    .unwrap_err(),
                QuantityValueContractRefusal::UnsupportedRepresentation {
                    profile,
                    version: 3
                }
            );
            assert_eq!(zero.spent(), 0);
        }
        assert!(QuantityValueView::decode(&encoded, &mut work()).is_err());
        assert!(validate_primitive_info(QUANTITY_INFO_ID, &encoded).is_err());
    });
    assert_eq!(allocations, 0);
    let rational = bytes(&MathematicalValue::rational(q(1, 3)));
    let encoded = frame(&rational, Unit::Meter, QuantityRole::Linear);
    let value = MathematicalQuantityView::decode(&encoded, &mut work())
        .unwrap()
        .rational(&mut work())
        .unwrap();
    assert_eq!(value.coordinate(), q(1, 3));
    assert_eq!(value.unit(), Unit::Meter);
}
#[test]
fn malformed_forms_copy_work_and_enclosure_bounds_refuse_without_output_or_growth() {
    let coordinate = bytes(&MathematicalValue::parse_literal("pi", &mut work()).unwrap());
    let view = MathematicalValueView::decode(&coordinate, &mut work()).unwrap();
    let valid = frame(&coordinate, Unit::Radian, QuantityRole::Linear);
    let mut invalid_role = valid.clone();
    invalid_role[1] = 255;
    let mut bad_version = valid.clone();
    bad_version[0] = 4;
    let oversized = vec![3; MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES + 1];
    let mut output = vec![0xA5; MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES];
    let mut prepared =
        PreparedMathematicalQuantityEnclosure::new(requirements(), 200_000_000).unwrap();
    let retained = prepared.retained_payload_bytes();
    let (_, allocations) = observe(|| {
        assert!(matches!(
            MathematicalQuantityView::decode(&invalid_role, &mut work()),
            Err(QRefusal::InvalidRole)
        ));
        assert!(matches!(
            MathematicalQuantityView::decode(&bad_version, &mut work()),
            Err(QRefusal::UnsupportedVersion(4))
        ));
        assert!(matches!(
            MathematicalQuantityView::decode(&oversized, &mut work()),
            Err(QRefusal::ExtentExceeded)
        ));
        assert!(MathematicalQuantityView::decode(&valid[..valid.len() - 1], &mut work()).is_err());
        assert!(matches!(
            encode_mathematical_quantity_value(
                &view,
                Unit::Radian,
                QuantityRole::Point,
                &mut output,
                &mut work()
            ),
            Err(QRefusal::InvalidRole)
        ));
        assert!(matches!(
            encode_mathematical_quantity_value(
                &view,
                Unit::Radian,
                QuantityRole::Linear,
                &mut output[..3],
                &mut work()
            ),
            Err(QRefusal::Numeric(
                ExactNumericRefusal::EncodingBufferTooSmall
            ))
        ));
        assert!(matches!(
            encode_mathematical_quantity_value(
                &view,
                Unit::Radian,
                QuantityRole::Linear,
                &mut output,
                &mut ExactNumericWork::new(0)
            ),
            Err(QRefusal::Numeric(ExactNumericRefusal::WorkLimit))
        ));
        assert!(output.iter().all(|b| *b == 0xA5));
        assert!(prepared
            .execute(&valid, &mut ExactNumericWork::new(0))
            .is_err());
        let n = encode_mathematical_quantity_value(
            &view,
            Unit::Radian,
            QuantityRole::Linear,
            &mut output,
            &mut work(),
        )
        .unwrap();
        assert_eq!(output[..n], valid);
        assert!(prepared.execute(&output[..n], &mut work()).is_ok());
        assert_eq!(prepared.retained_payload_bytes(), retained);
    });
    assert_eq!(allocations, 0);
    let mut too_small = requirements();
    too_small.maximum_retained_payload_bytes = (retained - 1) as u32;
    assert!(PreparedMathematicalQuantityEnclosure::new(too_small, 200_000_000).is_err());
    let mut bounded = PreparedMathematicalQuantityEnclosure::new(requirements(), 1).unwrap();
    let mut large = ExactNumericWork::new(u64::MAX);
    assert!(bounded.execute(&valid, &mut large).is_err());
    assert!(large.spent() <= 1);
}

#[test]
fn numeric_projection_keeps_affine_points_and_differences_distinct_without_shifting_coordinates() {
    let coordinate = bytes(&MathematicalValue::parse_literal("pi", &mut work()).unwrap());
    let point = frame(&coordinate, Unit::Fahrenheit, QuantityRole::Point);
    let delta = frame(&coordinate, Unit::Fahrenheit, QuantityRole::Delta);
    let point_view = MathematicalQuantityView::decode(&point, &mut work()).unwrap();
    let delta_view = MathematicalQuantityView::decode(&delta, &mut work()).unwrap();
    assert_ne!(
        point_view.representation_digest(),
        delta_view.representation_digest()
    );
    let mut prepared =
        PreparedMathematicalQuantityEnclosure::new(requirements(), 200_000_000).unwrap();
    let (_, allocations) = observe(|| {
        let a = prepared.execute(&point, &mut work()).unwrap();
        let b = prepared.execute(&delta, &mut work()).unwrap();
        assert_eq!(a.lower().unit(), Unit::Fahrenheit);
        assert_eq!(b.lower().unit(), Unit::Fahrenheit);
        assert_eq!(a.lower().role(), QuantityRole::Point);
        assert_eq!(b.lower().role(), QuantityRole::Delta);
        assert_eq!(a.lower().coordinate(), b.lower().coordinate());
        assert_eq!(a.upper().coordinate(), b.upper().coordinate());
        assert_eq!(a.width_coordinate(), b.width_coordinate());
        // A later physical coordinate conversion applies the existing offset
        // only to the point. Numeric realization itself changed neither frame.
        let point_kelvin = a
            .lower()
            .convert_to_unit(Unit::Kelvin, &mut work())
            .unwrap();
        let delta_kelvin = b
            .lower()
            .convert_to_unit(Unit::Kelvin, &mut work())
            .unwrap();
        assert_ne!(point_kelvin.coordinate(), delta_kelvin.coordinate());
        assert_eq!(point_kelvin.role(), QuantityRole::Point);
        assert_eq!(delta_kelvin.role(), QuantityRole::Delta);
    });
    assert_eq!(allocations, 0);
}

#[test]
fn proposed_coordinates_do_not_bypass_checked_physical_range_admission() {
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
    // Both an in-range rational and a symbolic/out-of-range coordinate are
    // mathematical data, not a grant to use an unselected representation.
    let rational = bytes(&MathematicalValue::rational(q(1, 3)));
    let symbolic = bytes(&MathematicalValue::parse_literal("pi", &mut work()).unwrap());
    let frames = [
        frame(&rational, Unit::Meter, QuantityRole::Linear),
        frame(&symbolic, Unit::Meter, QuantityRole::Linear),
    ];
    let legacy = Quantity::parse_plot_literal("30cm").unwrap().encode();
    let outside = Quantity::parse_plot_literal("60cm").unwrap().encode();
    let (_, allocations) = observe(|| {
        for profile in [QuantityValueProfile::Decimal, QuantityValueProfile::Exact] {
            assert!(contract
                .validate_with_quantity_profile(&legacy, profile)
                .is_ok());
            assert_eq!(
                contract.validate_with_quantity_profile(&outside, profile),
                Err(QuantityProfileContractRefusal::Contract(
                    ValueConstraintRefusal::QuantityRange
                ))
            );
            for encoded in &frames {
                assert_eq!(
                    contract.validate_with_quantity_profile(encoded, profile),
                    Err(QuantityProfileContractRefusal::Quantity(
                        QuantityValueContractRefusal::UnsupportedRepresentation {
                            profile,
                            version: 3
                        }
                    ))
                );
                assert!(matches!(
                    contract.validate(encoded),
                    Err(ValueConstraintRefusal::MalformedPrimitive(_))
                ));
            }
        }
    });
    assert_eq!(allocations, 0);
}
