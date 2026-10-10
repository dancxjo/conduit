use conduit_core::{
    ExactTemperatureDifference as Difference, ExactTemperatureDifferenceConversionReceipt,
    ExactTemperatureDifferenceRefusal, Quantity as Point, QuantityConversionRefusal,
    ResolvedQuantitySuffix,
};
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
        let receipt = ExactTemperatureDifferenceConversionReceipt::check(source, target).unwrap();
        let difference = receipt.source();
        let result = receipt.result().unwrap();
        assert_eq!(
            (result.coefficient(), result.exponent()),
            (coefficient, exponent)
        );
        assert_eq!(receipt.original(), source);
        assert_eq!(result.target().source(), target);
        assert_eq!(receipt.source_transform().1, 0);
        assert_eq!(receipt.target_transform().1, 0);
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
        Difference::parse_plot_literal("9°F")
            .unwrap()
            .compare(Difference::parse_plot_literal("5K").unwrap()),
        Ok(Ordering::Equal)
    );
    assert_eq!(
        Difference::parse_plot_literal("1°C")
            .unwrap()
            .compare(Difference::parse_plot_literal("0K").unwrap()),
        Ok(Ordering::Greater)
    );
    assert_eq!(
        Difference::parse_plot_literal("1°C")
            .unwrap()
            .convert_to_target(ResolvedQuantitySuffix::resolve("m").unwrap()),
        Err(QuantityConversionRefusal::IncompatibleDimensions)
    );
    let largest = Difference::new(10_i128.pow(38) - 1, 128, conduit_core::Unit::Kelvin).unwrap();
    assert_eq!(
        largest.convert_to_target(ResolvedQuantitySuffix::resolve("qK").unwrap()),
        Err(QuantityConversionRefusal::Overflow)
    );
    let smallest = Difference::new(1, -128, conduit_core::Unit::Kelvin).unwrap();
    assert_eq!(
        smallest.convert_to_target(ResolvedQuantitySuffix::resolve("QK").unwrap()),
        Err(QuantityConversionRefusal::Overflow)
    );
    assert_eq!(largest.compare(smallest), Ok(Ordering::Greater));
    // Existing point meanings and the old exceptional milli-Celsius law survive.
    let point = Point::parse_plot_literal("1m°C")
        .unwrap()
        .convert_to_target(ResolvedQuantitySuffix::resolve("K").unwrap())
        .unwrap();
    assert_eq!((point.coefficient(), point.exponent()), (273151, -3));
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
        let result = if case["role"] == "point" {
            Point::parse_plot_literal(source)
                .unwrap()
                .convert_to_target(target)
                .map(|value| (value.coefficient(), value.exponent()))
        } else {
            Difference::parse_plot_literal(source)
                .unwrap()
                .convert_to_target(target)
                .map(|value| (value.coefficient(), value.exponent()))
        };
        let expected = match case["refusal"].as_str() {
            Some("inexact") => Err(QuantityConversionRefusal::Inexact),
            Some("overflow") => Err(QuantityConversionRefusal::Overflow),
            None => Ok((
                case["coefficient"].as_str().unwrap().parse().unwrap(),
                case["exponent"].as_i64().unwrap() as i16,
            )),
            other => panic!("unexpected reference refusal {other:?}"),
        };
        assert_eq!(result, expected, "{case}");
    }
}
