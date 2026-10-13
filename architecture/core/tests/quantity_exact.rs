use conduit_core::*;
use core::cmp::Ordering;

#[test]
fn reviewed_units_and_prefixes_retain_coordinates_and_physical_equality() {
    for source in [
        "1Qm", "1qm", "1Qm³", "1qm³", "3.2m", "0.001km", "1000mm", "-1.50km", "440Hz", "250ms",
        "21°C", "1m°C", "1MiB", "1MB", "1um²",
    ] {
        let quantity = Quantity::parse_plot_literal(source).unwrap();
        assert_eq!(Quantity::decode(&quantity.encode()), Ok(quantity.clone()));
        assert_eq!(
            Quantity::parse_plot_literal(&quantity.canonical_literal().unwrap()),
            Ok(quantity)
        );
    }
    let first = Quantity::parse_plot_literal("1000mm").unwrap();
    let second = Quantity::parse_plot_literal("1m").unwrap();
    assert_ne!(first.encode(), second.encode());
    assert_eq!(first.compare(&second), Ok(Some(Ordering::Equal)));
    assert_eq!(
        second.compare(&Quantity::parse_plot_literal("0.001km").unwrap()),
        Ok(Some(Ordering::Equal))
    );
}

#[test]
fn all_reviewed_scales_match_independent_arbitrary_precision_corpus() {
    let corpus: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/quantity_prefix_scales.json")).unwrap();
    assert_eq!(corpus["cases"].as_array().unwrap().len(), 456);
    for case in corpus["cases"].as_array().unwrap() {
        let source = Quantity::parse_plot_literal(case["source"].as_str().unwrap()).unwrap();
        let target = Unit::resolve(case["base"].as_str().unwrap()).unwrap();
        let expected =
            MathValue::from_integer_decimal(case["relative_numerator"].as_str().unwrap())
                .unwrap()
                .checked_div(
                    &MathValue::from_integer_decimal(
                        case["relative_denominator"].as_str().unwrap(),
                    )
                    .unwrap(),
                )
                .unwrap();
        assert_eq!(source.convert_to_unit(&target).unwrap(), expected, "{case}");
    }
}

#[test]
fn mathematical_coordinates_exceed_the_old_decimal_exponent_and_coefficient_bounds() {
    let huge = MathValue::integer(10)
        .checked_pow(1000)
        .unwrap()
        .checked_add(&MathValue::integer(1))
        .unwrap();
    let q = Quantity::from_math(huge.clone(), &Unit::Meter).unwrap();
    assert_eq!(q.convert_to_unit(&Unit::Meter), Ok(huge));
    assert_eq!(Quantity::decode(q.encoded()), Ok(q));
    assert!(Quantity::from_decimal(1, 500, Unit::Meter).is_ok());
    assert!(Quantity::from_decimal(1, -500, Unit::Meter).is_ok());
}

#[test]
fn native_literal_evidence_bounds_are_separate_from_mathematical_coordinates() {
    assert!(Quantity::parse_plot_literal(&format!("{}m", "1".repeat(100))).is_err());
    assert!(Quantity::parse_plot_literal("NaNm").is_err());
    let third = MathValue::integer(1)
        .checked_div(&MathValue::integer(3))
        .unwrap();
    let q = Quantity::from_math(third, &Unit::Meter).unwrap();
    assert!(q.canonical_literal().is_err());
    assert!(Quantity::decode(q.encoded()).is_ok());
    assert!(Quantity::parse_plot_literal("1/3m").is_err());
}

#[test]
fn canonical_codec_rejects_short_old_and_forged_encodings() {
    for wire in [vec![], vec![0; 9], vec![0; 22], vec![0; 788]] {
        assert!(Quantity::decode(&wire).is_err());
    }
    let q = Quantity::parse_plot_literal("3.2m").unwrap();
    let mut wire = q.encode();
    wire[0] ^= 1;
    assert!(Quantity::decode(&wire).is_err());
    let mut wire = q.encode();
    *wire.last_mut().unwrap() ^= 1;
    assert!(Quantity::decode(&wire).is_err());
    let mut wire = q.encode();
    wire.push(0);
    assert!(Quantity::decode(&wire).is_err());
}

#[test]
fn exact_rational_affine_and_symbolic_relations_do_not_round() {
    let inch = Quantity::parse_plot_literal("1m")
        .unwrap()
        .convert_to_unit(&Unit::Inch)
        .unwrap();
    assert_eq!(
        inch,
        MathValue::integer(5000)
            .checked_div(&MathValue::integer(127))
            .unwrap()
    );
    let celsius = Quantity::parse_plot_literal("1°F")
        .unwrap()
        .convert_to_unit(&Unit::Celsius)
        .unwrap();
    assert_eq!(
        celsius,
        MathValue::integer(-155)
            .checked_div(&MathValue::integer(9))
            .unwrap()
    );
    let turn = Quantity::from_math(
        MathValue::pi().checked_div(&MathValue::integer(2)).unwrap(),
        &Unit::Radian,
    )
    .unwrap();
    assert_eq!(
        turn.convert_to_unit(&Unit::Turn),
        Ok(MathValue::parse_decimal("0.25").unwrap())
    );
}

#[test]
fn integer_projection_is_explicit_and_checked() {
    let q = Quantity::parse_plot_literal("1.5m").unwrap();
    assert_eq!(
        q.to_i64(&Unit::Meter),
        Err(QuantityConversionRefusal::Inexact)
    );
    assert_eq!(q.to_i64(&Unit::Millimeter), Ok(1500));
    let negative = Quantity::parse_plot_literal("-1m").unwrap();
    assert!(negative.to_u64(&Unit::Meter).is_err());
    assert_eq!(
        q.convert_to_unit(&Unit::Hertz),
        Err(QuantityConversionRefusal::IncompatibleDimensions)
    );
}
