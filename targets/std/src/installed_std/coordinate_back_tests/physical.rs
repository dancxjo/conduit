//! Symbolic and affine actual input through two separately checked projections.
use super::*;
fn execute(source: &[u8], unit: &str, limit: &str) -> (StructuredInfoValue, StructuredInfoValue) {
    let (mut host, plan) = prepared_for(unit, limit);
    let mut output = Outputs(Vec::with_capacity(2));
    let run = host
        .run_external_plot_to(
            plan.fragments[0].clone(),
            &[ExternalForeInput {
                front_port_id: "value".into(),
                track: ConnectionTrack::Payload,
                bytes: source.to_vec(),
            }],
            &mut output,
            &mut Vec::new(),
            &mut NoTimer,
        )
        .unwrap();
    let kernel = run.kernel.as_ref().unwrap();
    assert_eq!(kernel.post_play_start_allocations, 0);
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
    assert_eq!(output.0.len(), 2);
    let mut records = output
        .0
        .iter()
        .map(|b| StructuredInfoValue::from_canonical_bytes(b).unwrap())
        .collect::<Vec<_>>();
    let index = records
        .iter()
        .position(|v| v.value_type() == &quantity_coordinate_evidence_type())
        .unwrap();
    let coordinate = records.remove(index);
    let numeric = records.pop().unwrap();
    assert_eq!(numeric.value_type(), &near_quantity_type());
    (coordinate, numeric)
}
fn replay_coordinate(source: &[u8], target: &[u8], unit: Unit, record: &StructuredInfoValue) {
    let head = field(record, "projection-evidence-head");
    let tail = field(record, "projection-evidence-tail");
    assert_eq!(head.len(), (head.len() + tail.len()).min(65536));
    let mut frame = head.to_vec();
    frame.extend_from_slice(tail);
    let inspected = inspect_projection_evidence(&frame).unwrap();
    assert_eq!(&inspected.source_bytes[4..4 + source.len()], source);
    let ids = QuantityCoordinateContentIds::new(source, target, unit);
    let mut verifier = PreparedQuantityCoordinateConversion::new(500000, COORDINATE_WORK).unwrap();
    let mut scratch = vec![0; MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES];
    let guard = crate::allocation_probe::begin();
    verifier
        .verify_evidence(
            source,
            target,
            QuantityCoordinateConversionPolicy { target_unit: unit },
            COORDINATE_WORK,
            ids.basis(),
            &frame,
            &mut scratch,
            &mut work(),
        )
        .unwrap();
    assert_eq!(guard.finish(), 0);
}
#[test]
fn installed_symbolic_coordinates_remain_exact_until_the_consuming_near_back() {
    let value = MathematicalValue::parse_literal("pi", &mut work()).unwrap();
    let mut coordinate = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    let n = value.encode(&mut coordinate).unwrap();
    let view = MathematicalValueView::decode(&coordinate[..n], &mut work()).unwrap();
    let mut source = vec![0; MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES];
    let n = encode_mathematical_quantity_value(
        &view,
        Unit::Meter,
        QuantityRole::Linear,
        &mut source,
        &mut work(),
    )
    .unwrap();
    source.truncate(n);
    let (coordinate, numeric) = execute(&source, "cm", "0.01cm");
    let target = field(&numeric, "source-wire");
    let converted = MathematicalQuantityView::decode(target, &mut work()).unwrap();
    assert_eq!(converted.unit(), Unit::Centimeter);
    assert_eq!(converted.role(), QuantityRole::Linear);
    assert!(converted
        .coordinate()
        .as_rational(&mut work())
        .unwrap()
        .is_none());
    replay_coordinate(&source, target, Unit::Centimeter, &coordinate);
    let allowance = RationalQuantity::from_quantity(
        Quantity::parse_plot_literal("0.01cm").unwrap(),
        &mut work(),
    )
    .unwrap();
    let mut verifier = PreparedMathematicalNearQuantityVerifier::new(
        &near_quantity_type(),
        crate::numerical_host::near_step::mathematical_near_requirements(),
        Binary32Rounding::NearestTiesEven,
        allowance,
        900_000_000,
    )
    .unwrap();
    let bytes = numeric.canonical_bytes().unwrap();
    let guard = crate::allocation_probe::begin();
    let checked = verifier
        .verify(
            &bytes,
            QUANTITY_INFO_ID,
            Some(semantic_digest(QUANTITY_INFO_ID, target)),
            &mut ExactNumericWork::new(900_000_000),
        )
        .unwrap();
    assert_eq!(checked.source().encoded(), target);
    assert_ne!(checked.maximum_error(), ExactRational::ZERO);
    assert!(checked
        .maximum_error()
        .checked_compare(allowance.coordinate(), &mut work())
        .unwrap()
        .is_le());
    assert_eq!(guard.finish(), 0);
}
#[test]
fn installed_affine_point_and_delta_keep_separate_exact_offsets_and_numeric_evidence() {
    for (role, expected) in [
        (QuantityRole::Point, (46067, 180)),
        (QuantityRole::Delta, (5, 9)),
    ] {
        let source = Quantity::from_decimal_role(1, 0, Unit::Fahrenheit, role)
            .unwrap()
            .encode();
        let (coordinate, numeric) = execute(&source, "K", "TemperatureDelta(0.001,K)");
        let target = field(&numeric, "source-wire");
        let converted = QuantityValueView::decode(target, &mut work()).unwrap();
        assert_eq!(
            converted.representation(),
            QuantityCoordinateRepresentation::Rational
        );
        assert_eq!(converted.unit(), Unit::Kelvin);
        assert_eq!(converted.role(), role);
        let exact = converted.rational(&mut work()).unwrap();
        assert_eq!(
            exact.coordinate(),
            ExactRational::ratio(expected.0, expected.1, &mut work()).unwrap()
        );
        replay_coordinate(&source, target, Unit::Kelvin, &coordinate);
        let allowance = RationalQuantity::new(
            ExactRational::ratio(1, 1000, &mut work()).unwrap(),
            Unit::Kelvin,
            QuantityRole::Delta,
        )
        .unwrap();
        let verifier = PreparedNearQuantityVerifier::new(&near_quantity_type()).unwrap();
        let bytes = numeric.canonical_bytes().unwrap();
        let guard = crate::allocation_probe::begin();
        let checked = verifier
            .verify(
                &bytes,
                QUANTITY_INFO_ID,
                allowance,
                Some(semantic_digest(QUANTITY_INFO_ID, target)),
                &mut ExactNumericWork::new(900_000_000),
            )
            .unwrap();
        assert_eq!(checked.source(), exact);
        assert_eq!(guard.finish(), 0);
    }
}

#[test]
fn installed_turn_to_radian_conversion_retains_pi_until_near_consumes_it() {
    let source = Quantity::parse_plot_literal("1turn").unwrap().encode();
    let (coordinate, numeric) = execute(&source, "rad", "0.01rad");
    let target = field(&numeric, "source-wire");
    let value = MathematicalQuantityView::decode(target, &mut work()).unwrap();
    assert_eq!(value.unit(), Unit::Radian);
    assert_eq!(
        value
            .coordinate()
            .rational_pi_coefficient(&mut work())
            .unwrap(),
        Some(ExactRational::from_i128(2))
    );
    replay_coordinate(&source, target, Unit::Radian, &coordinate);
    let allowance = RationalQuantity::from_quantity(
        Quantity::parse_plot_literal("0.01rad").unwrap(),
        &mut work(),
    )
    .unwrap();
    let mut verifier = PreparedMathematicalNearQuantityVerifier::new(
        &near_quantity_type(),
        crate::numerical_host::near_step::mathematical_near_requirements(),
        Binary32Rounding::NearestTiesEven,
        allowance,
        900_000_000,
    )
    .unwrap();
    let bytes = numeric.canonical_bytes().unwrap();
    let guard = crate::allocation_probe::begin();
    let checked = verifier
        .verify(
            &bytes,
            QUANTITY_INFO_ID,
            Some(semantic_digest(QUANTITY_INFO_ID, target)),
            &mut ExactNumericWork::new(900_000_000),
        )
        .unwrap();
    assert_eq!(checked.source().encoded(), target);
    assert_eq!(guard.finish(), 0);
}

#[test]
fn installed_inverse_pi_coordinates_satisfy_zero_tolerance_at_the_numeric_boundary() {
    let pi = MathematicalValue::parse_literal("pi", &mut work()).unwrap();
    let mut encoded = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    let n = pi.encode(&mut encoded).unwrap();
    let pi = MathematicalValueView::decode(&encoded[..n], &mut work()).unwrap();
    let mut source = vec![0; MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES];
    let n = encode_mathematical_quantity_value(
        &pi,
        Unit::Radian,
        QuantityRole::Linear,
        &mut source,
        &mut work(),
    )
    .unwrap();
    source.truncate(n);
    let (coordinate, numeric) = execute(&source, "turn", "0turn");
    let target = field(&numeric, "source-wire");
    let value = MathematicalQuantityView::decode(target, &mut work()).unwrap();
    assert_eq!(value.unit(), Unit::Turn);
    assert_eq!(
        value.coordinate().as_rational(&mut work()).unwrap(),
        Some(ExactRational::ratio(1, 2, &mut work()).unwrap())
    );
    replay_coordinate(&source, target, Unit::Turn, &coordinate);
    let allowance =
        RationalQuantity::new(ExactRational::ZERO, Unit::Turn, QuantityRole::Linear).unwrap();
    let mut verifier = PreparedMathematicalNearQuantityVerifier::new(
        &near_quantity_type(),
        crate::numerical_host::near_step::mathematical_near_requirements(),
        Binary32Rounding::NearestTiesEven,
        allowance,
        900_000_000,
    )
    .unwrap();
    let bytes = numeric.canonical_bytes().unwrap();
    let guard = crate::allocation_probe::begin();
    let checked = verifier
        .verify(
            &bytes,
            QUANTITY_INFO_ID,
            Some(semantic_digest(QUANTITY_INFO_ID, target)),
            &mut ExactNumericWork::new(900_000_000),
        )
        .unwrap();
    assert_eq!(checked.source().encoded(), target);
    assert_eq!(checked.target().bits(), 0x3f00_0000);
    assert_eq!(checked.maximum_error(), ExactRational::ZERO);
    assert_eq!(guard.finish(), 0);
}

#[test]
fn installed_reciprocal_pi_coordinates_feed_a_bounded_projection_without_growth() {
    let source = Quantity::parse_plot_literal("1rad").unwrap().encode();
    let (coordinate, numeric) = execute(&source, "turn", "0.001turn");
    let target = field(&numeric, "source-wire");
    let value = MathematicalQuantityView::decode(target, &mut work()).unwrap();
    assert_eq!(value.unit(), Unit::Turn);
    assert!(value
        .coordinate()
        .as_rational(&mut work())
        .unwrap()
        .is_none());
    replay_coordinate(&source, target, Unit::Turn, &coordinate);
    let allowance = RationalQuantity::from_quantity(
        Quantity::parse_plot_literal("0.001turn").unwrap(),
        &mut work(),
    )
    .unwrap();
    let mut verifier = PreparedMathematicalNearQuantityVerifier::new(
        &near_quantity_type(),
        crate::numerical_host::near_step::mathematical_near_requirements(),
        Binary32Rounding::NearestTiesEven,
        allowance,
        900_000_000,
    )
    .unwrap();
    let bytes = numeric.canonical_bytes().unwrap();
    let guard = crate::allocation_probe::begin();
    let checked = verifier
        .verify(
            &bytes,
            QUANTITY_INFO_ID,
            Some(semantic_digest(QUANTITY_INFO_ID, target)),
            &mut ExactNumericWork::new(900_000_000),
        )
        .unwrap();
    assert_eq!(checked.source().encoded(), target);
    assert_eq!(checked.target().bits(), 0x3e22_f983);
    assert_ne!(checked.maximum_error(), ExactRational::ZERO);
    assert!(checked
        .maximum_error()
        .checked_compare(allowance.coordinate(), &mut work())
        .unwrap()
        .is_le());
    assert_eq!(guard.finish(), 0);
}
