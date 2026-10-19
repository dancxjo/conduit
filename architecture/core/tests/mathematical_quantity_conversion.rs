//! Physical coordinate conversion is independent of numerical realization.
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
fn numeric(value: &MathematicalValue) -> Vec<u8> {
    let mut out = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    let n = value.encode(&mut out).unwrap();
    out.truncate(n);
    out
}
fn frame(value: &MathematicalValue, unit: Unit, role: QuantityRole) -> Vec<u8> {
    let bytes = numeric(value);
    let view = MathematicalValueView::decode(&bytes, &mut work()).unwrap();
    let mut out = vec![0; MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES];
    let n = encode_mathematical_quantity_value(&view, unit, role, &mut out, &mut work()).unwrap();
    out.truncate(n);
    out
}
fn expected(scale: ExactRational, offset: ExactRational) -> Vec<u8> {
    let mut b = MathematicalValueBuilder::new();
    let pi = b.insert(MathematicalNode::Pi, &mut work()).unwrap();
    let factor = b
        .insert(MathematicalNode::Rational(scale), &mut work())
        .unwrap();
    let mut root = b
        .insert(MathematicalNode::Multiply(pi, factor), &mut work())
        .unwrap();
    if offset != ExactRational::ZERO {
        let shift = b
            .insert(MathematicalNode::Rational(offset), &mut work())
            .unwrap();
        root = b
            .insert(MathematicalNode::Add(root, shift), &mut work())
            .unwrap();
    }
    numeric(&b.finish(root, &mut work()).unwrap())
}
#[test]
fn symbolic_coordinates_convert_exactly_then_enclose_in_the_selected_unit_without_allocations() {
    let pi = MathematicalValue::parse_literal("pi", &mut work()).unwrap();
    let source = frame(&pi, Unit::Meter, QuantityRole::Linear);
    let expected = expected(q(100, 1), ExactRational::ZERO);
    let mut converter = PreparedMathematicalQuantityConversion::new(200000, 200_000_000).unwrap();
    let retained = converter.retained_payload_bytes();
    let mut output = vec![0; MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES];
    let req = MathematicalEnclosureRequirements {
        maximum_width: q(1, 100_000_000_000_000),
        maximum_leaf_width: q(1, 100_000_000_000_000_000),
        maximum_constant_terms: 32,
        maximum_root_refinements: 64,
        maximum_scratch_payload_bytes: 131072,
        maximum_retained_payload_bytes: 131072,
    };
    let mut numeric = PreparedMathematicalQuantityEnclosure::new(req, 200_000_000).unwrap();
    let (_, allocations) = observe(|| {
        for _ in 0..4 {
            let receipt = converter
                .execute(&source, Unit::Centimeter, &mut output, &mut work())
                .unwrap();
            assert_eq!(receipt.source().encoded(), source);
            assert_eq!(receipt.target_unit(), Unit::Centimeter);
            assert_eq!(receipt.scale(), q(100, 1));
            assert_eq!(receipt.offset(), ExactRational::ZERO);
            let bytes = &output[..receipt.encoded_bytes()];
            let view = MathematicalQuantityView::decode(bytes, &mut work()).unwrap();
            assert_eq!(view.coordinate().encoded(), expected);
            view.validate_kind(DISTANCE_INFO_ID).unwrap();
            assert_eq!(view.role(), QuantityRole::Linear);
            assert_eq!(view.family(), Unit::Meter.family());
            assert_eq!(view.dimension(), Unit::Meter.dimension());
            let result = numeric.execute(bytes, &mut work()).unwrap();
            let lower = result
                .lower()
                .convert_to_unit(Unit::Meter, &mut work())
                .unwrap();
            let upper = result
                .upper()
                .convert_to_unit(Unit::Meter, &mut work())
                .unwrap();
            assert!(lower
                .coordinate()
                .checked_compare(q(314159, 100000), &mut work())
                .unwrap()
                .is_gt());
            assert!(upper
                .coordinate()
                .checked_compare(q(314160, 100000), &mut work())
                .unwrap()
                .is_lt());
            assert_eq!(converter.retained_payload_bytes(), retained);
        }
    });
    assert_eq!(allocations, 0);
}
#[test]
fn point_and_difference_coordinate_changes_share_scale_but_only_points_receive_offset() {
    let pi = MathematicalValue::parse_literal("pi", &mut work()).unwrap();
    let point = frame(&pi, Unit::Fahrenheit, QuantityRole::Point);
    let delta = frame(&pi, Unit::Fahrenheit, QuantityRole::Delta);
    let point_expected = expected(q(5, 9), q(45967, 180));
    let delta_expected = expected(q(5, 9), ExactRational::ZERO);
    let mut converter = PreparedMathematicalQuantityConversion::new(200000, 200_000_000).unwrap();
    let mut output = vec![0; MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES];
    let (_, allocations) = observe(|| {
        for _ in 0..4 {
            for (source, role, expected, offset) in [
                (&point, QuantityRole::Point, &point_expected, q(45967, 180)),
                (
                    &delta,
                    QuantityRole::Delta,
                    &delta_expected,
                    ExactRational::ZERO,
                ),
            ] {
                let receipt = converter
                    .execute(source, Unit::Kelvin, &mut output, &mut work())
                    .unwrap();
                assert_eq!(receipt.scale(), q(5, 9));
                assert_eq!(receipt.offset(), offset);
                let view = MathematicalQuantityView::decode(
                    &output[..receipt.encoded_bytes()],
                    &mut work(),
                )
                .unwrap();
                assert_eq!(view.coordinate().encoded(), expected);
                assert_eq!(view.role(), role);
                assert_eq!(view.unit(), Unit::Kelvin);
                assert_eq!(receipt.source().encoded(), source);
            }
        }
    });
    assert_eq!(allocations, 0);
}
#[test]
fn conversion_refusals_are_atomic_bounded_and_recoverable() {
    let pi = MathematicalValue::parse_literal("pi", &mut work()).unwrap();
    let source = frame(&pi, Unit::Meter, QuantityRole::Linear);
    let turns = frame(&pi, Unit::Turn, QuantityRole::Linear);
    let mut converter = PreparedMathematicalQuantityConversion::new(200000, 200_000_000).unwrap();
    let retained = converter.retained_payload_bytes();
    assert!(
        PreparedMathematicalQuantityConversion::new((retained - 1) as u32, 200_000_000).is_err()
    );
    assert!(PreparedMathematicalQuantityConversion::new(200000, 0).is_err());
    let mut limited = PreparedMathematicalQuantityConversion::new(200000, 1).unwrap();
    let mut output = vec![0xA5; MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES];
    let (_, allocations) = observe(|| {
        assert!(converter
            .execute(&source, Unit::Second, &mut output, &mut work())
            .is_err());
        assert!(converter
            .execute(&turns, Unit::Radian, &mut output, &mut work())
            .is_err());
        assert!(converter
            .execute(&source, Unit::Centimeter, &mut output[..1], &mut work())
            .is_err());
        assert!(converter
            .execute(
                &source,
                Unit::Centimeter,
                &mut output,
                &mut ExactNumericWork::new(0)
            )
            .is_err());
        let mut large = ExactNumericWork::new(u64::MAX);
        assert!(limited
            .execute(&source, Unit::Centimeter, &mut output, &mut large)
            .is_err());
        assert!(large.spent() <= 1);
        assert!(output.iter().all(|b| *b == 0xA5));
        assert!(converter
            .execute(&source, Unit::Centimeter, &mut output, &mut work())
            .is_ok());
        assert_eq!(converter.retained_payload_bytes(), retained);
    });
    assert_eq!(allocations, 0);
}
