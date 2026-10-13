use conduit_core::{
    MathValue, Quantity as Exact, QuantityConversionRefusal as Refusal, ResolvedQuantitySuffix,
    Unit,
};

#[test]
fn complete_target_prefix_matrix_preserves_target_and_exact_inverse_scale() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/quantity_prefix_scales.json")).unwrap();
    for case in reference["cases"].as_array().unwrap() {
        let suffix = &case["source"].as_str().unwrap()[1..];
        let target = ResolvedQuantitySuffix::resolve(suffix).unwrap();
        let source =
            Exact::parse_plot_literal(&format!("1{}", case["base"].as_str().unwrap())).unwrap();
        let result = source.convert_to_target(target.clone()).unwrap();
        let numerator = case["relative_numerator"].as_str().unwrap();
        let denominator = case["relative_denominator"].as_str().unwrap();
        let inverse = MathValue::from_integer_decimal(denominator)
            .unwrap()
            .checked_div(&MathValue::from_integer_decimal(numerator).unwrap())
            .unwrap();
        assert_eq!(result.coordinate(), &inverse, "{suffix}");
        assert_eq!(result.target(), &target);
        let same = Exact::parse_plot_literal(case["source"].as_str().unwrap())
            .unwrap()
            .convert_to_target(target.clone())
            .unwrap();
        assert_eq!(same.coordinate(), &MathValue::integer(1), "{suffix}");
    }
}

#[test]
fn target_projection_checks_affine_law_precision_range_and_dimension() {
    for (source, target, coefficient, exponent) in [
        ("0°C", "K", 27315, -2),
        ("30°C", "°F", 86, 0),
        ("0m°C", "K", 27315, -2),
        ("1kHz", "Hz", 1, 3),
        ("1µs", "ns", 1, 3),
        ("1cm²", "mm²", 1, 2),
        ("-1Qm³", "qm³", -1, 180),
    ] {
        let result = Exact::parse_plot_literal(source)
            .unwrap()
            .convert_to_target(ResolvedQuantitySuffix::resolve(target).unwrap());
        assert_eq!(
            result.unwrap().coordinate(),
            &MathValue::from_decimal_parts(coefficient, exponent).unwrap()
        );
    }
    assert_eq!(
        Exact::parse_plot_literal("1Hz")
            .unwrap()
            .convert_to_target(ResolvedQuantitySuffix::resolve("m").unwrap()),
        Err(Refusal::IncompatibleDimensions)
    );
    assert_eq!(
        Exact::parse_plot_literal("1m")
            .unwrap()
            .convert_to_target(ResolvedQuantitySuffix::resolve("in").unwrap())
            .unwrap()
            .coordinate(),
        &MathValue::integer(5000)
            .checked_div(&MathValue::integer(127))
            .unwrap()
    );

    let high = Exact::from_decimal(1, 128, Unit::Meter).unwrap();
    assert!(
        high.convert_to_target(ResolvedQuantitySuffix::resolve("Qm").unwrap())
            .is_ok()
    );
    let padded = high
        .convert_to_target(ResolvedQuantitySuffix::resolve("qm").unwrap())
        .unwrap();
    assert_eq!(
        padded.coordinate(),
        &MathValue::from_decimal_parts(1, 158).unwrap()
    );
    let largest = Exact::from_decimal(10_i128.pow(38) - 1, 128, Unit::Meter).unwrap();
    assert!(
        largest
            .convert_to_target(ResolvedQuantitySuffix::resolve("qm").unwrap())
            .is_ok()
    );
}
