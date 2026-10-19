//! All admitted Angle wire forms share the explicit exact radian conversion.
use conduit_core::*;
#[path = "common/numerical_allocation.rs"]
mod allocation_probe;
fn work() -> ExactNumericWork {
    ExactNumericWork::new(200_000_000)
}
fn rational(n: i128, d: i128) -> ExactRational {
    ExactRational::ratio(n, d, &mut work()).unwrap()
}
fn frame(value: &MathematicalValue, unit: Unit) -> Vec<u8> {
    let mut coordinate = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    let n = value.encode(&mut coordinate).unwrap();
    let view = MathematicalValueView::decode(&coordinate[..n], &mut work()).unwrap();
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
fn native_rational_and_symbolic_turns_convert_then_radians_remain_identical() {
    let native = Quantity::parse_plot_literal("90°").unwrap().encode();
    let mut exact = vec![0; 4096];
    let n = encode_rational_quantity_value(
        RationalQuantity::new(rational(1, 4), Unit::Turn, QuantityRole::Linear).unwrap(),
        &mut exact,
    )
    .unwrap();
    exact.truncate(n);
    let symbolic = frame(&MathematicalValue::rational(rational(1, 4)), Unit::Turn);
    let mut converter = PreparedAngleRadianQuantityConversion::new(262144, 200_000_000).unwrap();
    let retained = converter.retained_payload_bytes();
    let mut output = vec![0; MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES];
    let mut identity = output.clone();
    let (_, allocations) = allocation_probe::observe(|| {
        for _ in 0..8 {
            for source in [native.as_slice(), exact.as_slice(), symbolic.as_slice()] {
                let receipt = converter.execute(source, &mut output, &mut work()).unwrap();
                assert_eq!(receipt.reference_law(), Some(TURN_RADIAN_REFERENCE_LAW));
                let n = receipt.encoded_bytes();
                let view = MathematicalQuantityView::decode(&output[..n], &mut work()).unwrap();
                assert_eq!(view.unit(), Unit::Radian);
                assert_eq!(
                    view.coordinate()
                        .rational_pi_coefficient(&mut work())
                        .unwrap(),
                    Some(rational(1, 2))
                );
                let receipt = converter
                    .execute(&output[..n], &mut identity, &mut work())
                    .unwrap();
                assert_eq!(receipt.reference_law(), None);
                assert_eq!(&identity[..receipt.encoded_bytes()], &output[..n]);
            }
        }
    });
    assert_eq!(allocations, 0);
    assert_eq!(converter.retained_payload_bytes(), retained);
}
#[test]
fn radian_coordinates_and_refusals_preserve_exact_values_and_output() {
    let pi = MathematicalValue::parse_literal("pi", &mut work()).unwrap();
    let symbolic = frame(&pi, Unit::Radian);
    let scalar = frame(&pi, Unit::One);
    let native = Quantity::parse_plot_literal("1rad").unwrap().encode();
    let mut converter = PreparedAngleRadianQuantityConversion::new(262144, 200_000_000).unwrap();
    let mut output = vec![0xA5; MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES];
    let (_, allocations) = allocation_probe::observe(|| {
        assert!(converter
            .execute(&scalar, &mut output, &mut work())
            .is_err());
        assert!(converter
            .execute(&symbolic, &mut output[..1], &mut work())
            .is_err());
        assert!(converter
            .execute(&symbolic, &mut output, &mut ExactNumericWork::new(0))
            .is_err());
        assert!(output.iter().all(|b| *b == 0xA5));
        let receipt = converter
            .execute(&native, &mut output, &mut work())
            .unwrap();
        assert_eq!(receipt.reference_law(), None);
        let view =
            MathematicalQuantityView::decode(&output[..receipt.encoded_bytes()], &mut work())
                .unwrap();
        assert_eq!(
            view.coordinate().as_rational(&mut work()).unwrap(),
            Some(ExactRational::ONE)
        );
        let receipt = converter
            .execute(&symbolic, &mut output, &mut work())
            .unwrap();
        assert_eq!(&output[..receipt.encoded_bytes()], symbolic);
    });
    assert_eq!(allocations, 0);
}

#[test]
fn scaled_symbolic_radians_use_only_the_exact_same_origin_law() {
    let pi = MathematicalValue::parse_literal("pi", &mut work()).unwrap();
    let milliradian = Quantity::parse_plot_literal("1mrad").unwrap().unit();
    let source = frame(&pi, milliradian);
    let mut output = vec![0; MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES];
    let mut converter = PreparedAngleRadianQuantityConversion::new(262144, 200_000_000).unwrap();
    let (_, allocations) = allocation_probe::observe(|| {
        let receipt = converter
            .execute(&source, &mut output, &mut work())
            .unwrap();
        assert_eq!(receipt.reference_law(), None);
        assert_eq!(receipt.scale(), rational(1, 1000));
        let result =
            MathematicalQuantityView::decode(&output[..receipt.encoded_bytes()], &mut work())
                .unwrap();
        assert_eq!(result.unit(), Unit::Radian);
        assert_eq!(
            result
                .coordinate()
                .rational_pi_coefficient(&mut work())
                .unwrap(),
            Some(rational(1, 1000))
        );
    });
    assert_eq!(allocations, 0);
}
