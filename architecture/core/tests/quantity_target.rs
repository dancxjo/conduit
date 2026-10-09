use conduit_core::{
    ExactDecimalQuantity as Exact, QuantityConversionRefusal as Refusal, QuantityUnit,
    ResolvedQuantitySuffix,
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
        let result = source.convert_to_target(target).unwrap();
        let numerator = case["relative_numerator"].as_str().unwrap();
        let denominator = case["relative_denominator"].as_str().unwrap();
        let inverse_exponent = denominator.len() as i16 - numerator.len() as i16;
        assert_eq!(
            (result.coefficient(), result.exponent()),
            (1, inverse_exponent),
            "{suffix}"
        );
        assert_eq!(result.target(), target);
        let same = Exact::parse_plot_literal(case["source"].as_str().unwrap())
            .unwrap()
            .convert_to_target(target)
            .unwrap();
        assert_eq!((same.coefficient(), same.exponent()), (1, 0), "{suffix}");
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
        if exponent > 128 {
            assert_eq!(result, Err(Refusal::Overflow));
        } else {
            let result = result.unwrap();
            assert_eq!(
                (result.coefficient(), result.exponent()),
                (coefficient, exponent)
            );
        }
    }
    for (source, target, refusal) in [
        ("1°F", "°C", Refusal::Inexact),
        ("1m", "in", Refusal::Inexact),
        ("1Hz", "m", Refusal::IncompatibleDimensions),
    ] {
        assert_eq!(
            Exact::parse_plot_literal(source)
                .unwrap()
                .convert_to_target(ResolvedQuantitySuffix::resolve(target).unwrap()),
            Err(refusal)
        );
    }
    let high = Exact::new(1, 128, QuantityUnit::Meter).unwrap();
    assert!(high
        .convert_to_target(ResolvedQuantitySuffix::resolve("Qm").unwrap())
        .is_ok());
    let padded = high
        .convert_to_target(ResolvedQuantitySuffix::resolve("qm").unwrap())
        .unwrap();
    assert_eq!(
        (padded.coefficient(), padded.exponent()),
        (10_i128.pow(30), 128)
    );
    let largest = Exact::new(10_i128.pow(38) - 1, 128, QuantityUnit::Meter).unwrap();
    assert_eq!(
        largest.convert_to_target(ResolvedQuantitySuffix::resolve("qm").unwrap()),
        Err(Refusal::Overflow)
    );
}
