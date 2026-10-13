use conduit_core::{
    ExactTemperatureDifference as Difference, ExactTemperatureDifferenceConversionReceipt,
    ExactTemperatureDifferenceRefusal, Quantity as Point, QuantityConversionRefusal,
    ResolvedQuantitySuffix,
};
use conduit_core::{MathValue, Unit};
use core::cmp::Ordering;

#[test]
fn point_and_difference_have_distinct_identity_and_affine_laws() {
    for (source, target, coefficient, exponent) in [
        ("9°F", "K", 5, 0),
        ("-9°F", "°C", -5, 0),
        ("1m°C", "K", 1, -3),
        ("9m°F", "mK", 5, 0),
        ("0°C", "K", 0, 0),
        ("1QK", "qK", 1, 60),
    ] {
        let point = Point::parse_plot_literal(source).unwrap();
        let authored = format!(
            "TemperatureDelta({}, {})",
            point
                .canonical_literal()
                .unwrap()
                .strip_suffix(point.unit().symbol())
                .unwrap(),
            point.unit().symbol()
        );
        let receipt =
            ExactTemperatureDifferenceConversionReceipt::check(&authored, target).unwrap();
        let difference = receipt.source();
        let result = receipt.result().unwrap();
        assert_eq!(
            result.coordinate(),
            &MathValue::from_decimal_parts(coefficient, exponent).unwrap()
        );
        assert_eq!(receipt.original(), authored);
        assert_eq!(result.target().source(), target);
        assert!(
            receipt
                .source()
                .storage_coordinate()
                .unit()
                .exact_offset(conduit_core::QuantityRole::Delta)
                .unwrap()
                .is_zero()
        );
        assert!(
            receipt
                .target()
                .unit()
                .exact_offset(conduit_core::QuantityRole::Delta)
                .unwrap()
                .is_zero()
        );
        assert_ne!(
            difference.semantic_digest(),
            Point::parse_plot_literal(source).unwrap().semantic_digest()
        );
    }
    assert_eq!(
        Difference::parse_plot_literal("1Hz"),
        Err(ExactTemperatureDifferenceRefusal::NotTemperature)
    );
    assert_eq!(
        Difference::parse_plot_literal("TemperatureDelta(9, °F)")
            .unwrap()
            .compare(&Difference::parse_plot_literal("TemperatureDelta(5, K)").unwrap()),
        Ok(Some(Ordering::Equal))
    );
    assert_eq!(
        Difference::parse_plot_literal("TemperatureDelta(1, °C)")
            .unwrap()
            .compare(&Difference::parse_plot_literal("TemperatureDelta(0, K)").unwrap()),
        Ok(Some(Ordering::Greater))
    );
    assert_eq!(
        Difference::parse_plot_literal("TemperatureDelta(1, °C)")
            .unwrap()
            .convert_to_target(ResolvedQuantitySuffix::resolve("m").unwrap()),
        Err(QuantityConversionRefusal::IncompatibleDimensions)
    );
    let largest = Difference::new(10_i128.pow(38) - 1, 128, Unit::Kelvin).unwrap();
    assert!(
        largest
            .convert_to_target(ResolvedQuantitySuffix::resolve("qK").unwrap())
            .is_ok()
    );
    let smallest = Difference::new(1, -128, Unit::Kelvin).unwrap();
    assert!(
        smallest
            .convert_to_target(ResolvedQuantitySuffix::resolve("QK").unwrap())
            .is_ok()
    );
    assert_eq!(largest.compare(&smallest), Ok(Some(Ordering::Greater)));
    let point = Point::parse_plot_literal("1m°C")
        .unwrap()
        .convert_to_target(ResolvedQuantitySuffix::resolve("K").unwrap())
        .unwrap();
    assert_eq!(
        point.coordinate(),
        &MathValue::parse_decimal("273.151").unwrap()
    );
}

#[test]
fn both_roles_match_independent_unbounded_fraction_reference() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/quantity_temperature_reference.json")).unwrap();
    let cases = reference["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 972);
    for case in cases {
        let source = case["source"].as_str().unwrap();
        let target = ResolvedQuantitySuffix::resolve(case["target"].as_str().unwrap()).unwrap();
        let point = Point::parse_plot_literal(source).unwrap();
        let result = if case["role"] == "point" {
            point
                .convert_to_target(target)
                .unwrap()
                .coordinate()
                .clone()
        } else {
            let unit = point.unit().to_owned();
            Difference::from_math(point.coordinate().to_owned().unwrap(), &unit)
                .unwrap()
                .convert_to_target(target)
                .unwrap()
                .coordinate()
                .clone()
        };
        let expected = MathValue::from_integer_decimal(case["numerator"].as_str().unwrap())
            .unwrap()
            .checked_div(
                &MathValue::from_integer_decimal(case["denominator"].as_str().unwrap()).unwrap(),
            )
            .unwrap();
        assert_eq!(result, expected, "{case}");
    }
}
