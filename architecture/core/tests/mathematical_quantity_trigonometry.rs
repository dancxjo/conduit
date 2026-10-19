//! Exact Angle identity survives the Pi-bearing physical coordinate form.
use conduit_core::*;
#[path = "common/numerical_allocation.rs"]
mod allocation_probe;
use allocation_probe::observe;
fn work() -> ExactNumericWork {
    ExactNumericWork::new(200_000_000)
}
fn q(n: i128, d: i128) -> ExactRational {
    ExactRational::ratio(n, d, &mut work()).unwrap()
}
fn frame(value: &MathematicalValue, unit: Unit) -> Vec<u8> {
    let mut bytes = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    let n = value.encode(&mut bytes).unwrap();
    let view = MathematicalValueView::decode(&bytes[..n], &mut work()).unwrap();
    let mut out = vec![0; MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES];
    let n = encode_mathematical_quantity_value(
        &view,
        unit,
        QuantityRole::Linear,
        &mut out,
        &mut work(),
    )
    .unwrap();
    out.truncate(n);
    out
}
#[test]
fn quarter_turn_and_ninety_degrees_remain_exact_angles_through_pi_radians_and_trig() {
    let mut out = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    let mut previous = None;
    for spelling in ["0.25turn", "90°"] {
        let source = Quantity::parse_plot_literal(spelling).unwrap();
        let receipt = PreparedAngleReferenceConversion::from_checked(
            source,
            Unit::Radian,
            spelling,
            "rad",
            &mut work(),
        )
        .unwrap();
        let bytes = frame(receipt.denotation(), Unit::Radian);
        assert_eq!(receipt.original(), spelling);
        assert_eq!(receipt.turns().coordinate(), q(1, 4));
        let (_, allocations) = observe(|| {
            for _ in 0..8 {
                let view = MathematicalQuantityView::decode(&bytes, &mut work()).unwrap();
                assert_eq!(view.family(), Unit::Turn.family());
                assert_eq!(view.dimension(), Unit::Turn.dimension());
                assert_eq!(
                    view.coordinate()
                        .rational_pi_coefficient(&mut work())
                        .unwrap(),
                    Some(q(1, 2))
                );
                for (function, expected) in [
                    (AngleTrigonometricFunction::Sine, ExactRational::ONE),
                    (AngleTrigonometricFunction::Cosine, ExactRational::ZERO),
                ] {
                    let n = encode_mathematical_angle_trigonometric_denotation(
                        function,
                        &view,
                        &mut out,
                        &mut work(),
                    )
                    .unwrap();
                    assert_eq!(
                        MathematicalValueView::decode(&out[..n], &mut work())
                            .unwrap()
                            .as_rational(&mut work())
                            .unwrap(),
                        Some(expected)
                    );
                }
                assert_eq!(
                    encode_mathematical_angle_trigonometric_denotation(
                        AngleTrigonometricFunction::Tangent,
                        &view,
                        &mut out,
                        &mut work()
                    ),
                    Err(AngleTrigonometryRefusal::Denotation(
                        MathematicalValueRefusal::TrigonometricPole
                    ))
                );
                assert_eq!(view.encoded(), bytes);
            }
        });
        assert_eq!(allocations, 0);
        if let Some(old) = previous {
            assert_eq!(bytes, old);
        }
        previous = Some(bytes);
    }
}
#[test]
fn scaled_radian_coordinates_and_rational_turns_reuse_the_existing_exact_trig_writer() {
    let source = Quantity::parse_plot_literal("90°").unwrap();
    let receipt = PreparedAngleReferenceConversion::from_checked(
        source,
        Unit::Radian,
        "90°",
        "rad",
        &mut work(),
    )
    .unwrap();
    let bytes = frame(receipt.denotation(), Unit::Radian);
    let milliradian = Quantity::parse_plot_literal("1mrad").unwrap().unit();
    let mut converted = vec![0; MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES];
    let mut conversion = PreparedMathematicalQuantityConversion::new(200000, 200_000_000).unwrap();
    let n = conversion
        .execute(&bytes, milliradian, &mut converted, &mut work())
        .unwrap()
        .encoded_bytes();
    converted.truncate(n);
    let mut division = MathematicalValueBuilder::new();
    let pi = division.insert(MathematicalNode::Pi, &mut work()).unwrap();
    let two = division
        .insert(MathematicalNode::Rational(q(2, 1)), &mut work())
        .unwrap();
    let root = division
        .insert(MathematicalNode::Divide(pi, two), &mut work())
        .unwrap();
    let divided = frame(&division.finish(root, &mut work()).unwrap(), Unit::Radian);
    let turn = frame(&MathematicalValue::rational(q(1, 6)), Unit::Turn);
    let expected = PreparedAngleTrigonometry::from_checked(
        AngleTrigonometricFunction::Sine,
        Quantity::parse_plot_literal("60°").unwrap(),
        "60°",
        &mut work(),
    )
    .unwrap();
    let mut expected_bytes = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    let expected_n = expected.denotation().encode(&mut expected_bytes).unwrap();
    let mut out = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    let (_, allocations) = observe(|| {
        for _ in 0..8 {
            let radians = MathematicalQuantityView::decode(&converted, &mut work()).unwrap();
            let n = encode_mathematical_angle_trigonometric_denotation(
                AngleTrigonometricFunction::Sine,
                &radians,
                &mut out,
                &mut work(),
            )
            .unwrap();
            assert_eq!(
                MathematicalValueView::decode(&out[..n], &mut work())
                    .unwrap()
                    .as_rational(&mut work())
                    .unwrap(),
                Some(ExactRational::ONE)
            );
            let direct = MathematicalQuantityView::decode(&divided, &mut work()).unwrap();
            let n = encode_mathematical_angle_trigonometric_denotation(
                AngleTrigonometricFunction::Sine,
                &direct,
                &mut out,
                &mut work(),
            )
            .unwrap();
            assert_eq!(
                MathematicalValueView::decode(&out[..n], &mut work())
                    .unwrap()
                    .as_rational(&mut work())
                    .unwrap(),
                Some(ExactRational::ONE)
            );
            let turns = MathematicalQuantityView::decode(&turn, &mut work()).unwrap();
            let n = encode_mathematical_angle_trigonometric_denotation(
                AngleTrigonometricFunction::Sine,
                &turns,
                &mut out,
                &mut work(),
            )
            .unwrap();
            assert_eq!(out[..n], expected_bytes[..expected_n]);
        }
    });
    assert_eq!(allocations, 0);
}
#[test]
fn scalar_family_unknown_symbolic_radians_and_work_refuse_without_inventing_an_angle() {
    let pi = MathematicalValue::parse_literal("pi", &mut work()).unwrap();
    let scalar = frame(&pi, Unit::One);
    let plain = frame(
        &MathematicalValue::rational(ExactRational::ONE),
        Unit::Radian,
    );
    let mut b = MathematicalValueBuilder::new();
    let two = b
        .insert(MathematicalNode::Rational(q(2, 1)), &mut work())
        .unwrap();
    let root = b
        .insert(MathematicalNode::PrincipalSquareRoot(two), &mut work())
        .unwrap();
    let unknown = frame(&b.finish(root, &mut work()).unwrap(), Unit::Radian);
    let valid = frame(&pi, Unit::Radian);
    let mut output = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    let (_, allocations) = observe(|| {
        let scalar = MathematicalQuantityView::decode(&scalar, &mut work()).unwrap();
        assert_eq!(
            encode_mathematical_angle_trigonometric_denotation(
                AngleTrigonometricFunction::Sine,
                &scalar,
                &mut output,
                &mut work()
            ),
            Err(AngleTrigonometryRefusal::NotAngle)
        );
        for bytes in [&plain, &unknown] {
            let view = MathematicalQuantityView::decode(bytes, &mut work()).unwrap();
            assert!(encode_mathematical_angle_trigonometric_denotation(
                AngleTrigonometricFunction::Sine,
                &view,
                &mut output,
                &mut work()
            )
            .is_err());
        }
        let view = MathematicalQuantityView::decode(&valid, &mut work()).unwrap();
        assert!(encode_mathematical_angle_trigonometric_denotation(
            AngleTrigonometricFunction::Sine,
            &view,
            &mut output,
            &mut ExactNumericWork::new(0)
        )
        .is_err());
        let n = encode_mathematical_angle_trigonometric_denotation(
            AngleTrigonometricFunction::Sine,
            &view,
            &mut output,
            &mut work(),
        )
        .unwrap();
        assert_eq!(
            MathematicalValueView::decode(&output[..n], &mut work())
                .unwrap()
                .as_rational(&mut work())
                .unwrap(),
            Some(ExactRational::ZERO)
        );
    });
    assert_eq!(allocations, 0);
}
