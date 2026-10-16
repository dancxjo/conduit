use conduit_core::*;
fn work() -> ExactNumericWork {
    ExactNumericWork::new(100_000_000)
}
fn quantity(s: &str) -> Quantity {
    Quantity::parse_plot_literal(s).unwrap()
}
fn encoded(value: &MathematicalValue) -> Vec<u8> {
    let a = MathematicalAngle::from_turn_coordinate(value.clone());
    let mut b = vec![0; MATHEMATICAL_ANGLE_MAX_ENCODED_BYTES];
    let n = a.encode(&mut b).unwrap();
    b.truncate(n);
    b
}
#[test]
fn bounded_atan2_writer_matches_exact_builder_across_units_axes_and_quadrants() {
    for (y, x) in [
        ("3m", "400cm"),
        ("-3m", "400cm"),
        ("3m", "-400cm"),
        ("-3m", "-400cm"),
        ("0m", "-4m"),
        ("0m", "4m"),
        ("1m", "1m"),
        ("1m", "-1m"),
        ("-1m", "-1m"),
        ("-1m", "1m"),
        ("1m", "0cm"),
        ("-1m", "0cm"),
    ] {
        let expected = QuantityAtan2Denotation::from_checked(
            Atan2QuantityInputs {
                y: quantity(y),
                original_y: y,
                x: quantity(x),
                original_x: x,
                common_unit: Unit::resolve("m").unwrap(),
                common_unit_source: "m",
            },
            Atan2Branch::PrincipalSignedHalfTurn,
            &mut work(),
        )
        .unwrap();
        let mut out = vec![0; INVERSE_ANGLE_MAX_ENCODED_BYTES];
        let n = encode_quantity_atan2_denotation(quantity(y), quantity(x), &mut out, &mut work())
            .unwrap();
        out.truncate(n);
        assert_eq!(out, encoded(expected.coordinate()), "{y}, {x}");
        validate_primitive_info(MATHEMATICAL_ANGLE_ID, &out).unwrap();
    }
}
#[test]
fn bounded_unary_writers_share_normalization_and_domain_refusals() {
    for (function, node) in [
        (
            InverseAngleFunction::Arctangent,
            MathematicalNode::ArctangentTurns as fn(_) -> _,
        ),
        (
            InverseAngleFunction::Arcsine,
            MathematicalNode::ArcsineTurns,
        ),
        (
            InverseAngleFunction::Arccosine,
            MathematicalNode::ArccosineTurns,
        ),
    ] {
        for n in -4..=4 {
            let input = ExactRational::ratio(n, 2, &mut work()).unwrap();
            let mut b = MathematicalValueBuilder::new();
            let a = b
                .insert(MathematicalNode::Rational(input), &mut work())
                .unwrap();
            let expected = b.insert(node(a), &mut work());
            let mut out = vec![0; INVERSE_ANGLE_MAX_ENCODED_BYTES];
            let actual = encode_inverse_angle_denotation(function, input, &mut out, &mut work());
            match expected {
                Ok(root) => {
                    let expected = b.finish(root, &mut work()).unwrap();
                    out.truncate(actual.unwrap());
                    assert_eq!(out, encoded(&expected));
                }
                Err(reason) => {
                    assert_eq!(actual, Err(InverseAngleEncodingRefusal::Denotation(reason)))
                }
            }
        }
    }
}
#[test]
fn writer_storage_work_and_quantity_domain_refusals_remain_distinct() {
    assert_eq!(
        encode_quantity_atan2_denotation(
            quantity("0m"),
            quantity("0cm"),
            &mut [0; INVERSE_ANGLE_MAX_ENCODED_BYTES],
            &mut work()
        ),
        Err(InverseAngleEncodingRefusal::Quantity(
            Atan2Refusal::UndefinedZeroPair
        ))
    );
    assert_eq!(
        encode_quantity_atan2_denotation(
            quantity("3m"),
            quantity("4s"),
            &mut [0; INVERSE_ANGLE_MAX_ENCODED_BYTES],
            &mut work()
        ),
        Err(InverseAngleEncodingRefusal::Quantity(
            Atan2Refusal::MismatchedDimensions
        ))
    );
    assert!(encode_inverse_angle_denotation(
        InverseAngleFunction::Arctangent,
        ExactRational::ONE,
        &mut [0; 1],
        &mut work()
    )
    .is_err());
    assert!(matches!(
        encode_inverse_angle_denotation(
            InverseAngleFunction::Arctangent,
            ExactRational::ZERO,
            &mut [0; INVERSE_ANGLE_MAX_ENCODED_BYTES],
            &mut ExactNumericWork::new(0)
        ),
        Err(InverseAngleEncodingRefusal::Denotation(
            MathematicalValueRefusal::Numeric(ExactNumericRefusal::WorkLimit)
        ))
    ));
}
