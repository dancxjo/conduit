use conduit_core::{Quantity, QuantityRole, Unit};
use conduit_face::{
    rounded_quantity_coordinate, FormattedTemperature, LookTemperatureUnit as T,
    QuantityFormatRefusal as R,
};
fn q(source: &str) -> Quantity {
    Quantity::parse_plot_literal(source).unwrap()
}

#[test]
fn three_unit_looks_retain_one_exact_temperature_point_and_correct_symbols() {
    let source = q("19.8°C");
    let encoded = source.encode();
    for (unit, number, label) in [
        (T::Celsius, "19.800", "°C"),
        (T::Kelvin, "292.950", "K"),
        (T::Fahrenheit, "67.640", "°F"),
    ] {
        let shown = FormattedTemperature::prepare(source, unit, 3).unwrap();
        assert_eq!(shown.source(), source);
        assert_eq!(shown.source().encode(), encoded);
        assert_eq!(shown.unit(), unit);
        assert_eq!(shown.number(), number);
        assert_eq!(shown.unit_label(), label);
        assert_eq!(shown.label(), format!("{number} {label}"));
    }
}
#[test]
fn nonterminating_affine_conversion_is_rounded_once_in_the_selected_unit() {
    let source = q("33°F");
    for (unit, expected) in [
        (T::Celsius, "0.555556 °C"),
        (T::Kelvin, "273.705556 K"),
        (T::Fahrenheit, "33.000000 °F"),
    ] {
        let shown = FormattedTemperature::prepare(source, unit, 6).unwrap();
        assert_eq!(shown.label(), expected);
        assert_eq!(shown.source(), source);
    }
}
#[test]
fn ties_round_away_from_zero_and_zero_has_no_negative_sign() {
    for (source, expected) in [("32.9°F", "1"), ("31.1°F", "-1")] {
        assert_eq!(
            FormattedTemperature::prepare(q(source), T::Celsius, 0)
                .unwrap()
                .number(),
            expected
        );
    }
    assert_eq!(
        FormattedTemperature::prepare(q("-0.0004°C"), T::Celsius, 3)
            .unwrap()
            .number(),
        "0.000"
    );
    assert_eq!(
        FormattedTemperature::prepare(q("-273.15°C"), T::Kelvin, 3)
            .unwrap()
            .label(),
        "0.000 K"
    );
}
#[test]
fn finite_precision_role_family_and_numeric_extent_refuse_explicitly() {
    assert_eq!(
        FormattedTemperature::prepare(q("1°C"), T::Celsius, 10),
        Err(R::DecimalPlaces)
    );
    assert_eq!(
        FormattedTemperature::prepare(q("59%"), T::Celsius, 3),
        Err(R::UnsupportedQuantity)
    );
    let delta = Quantity::from_decimal_role(1, 0, Unit::Kelvin, QuantityRole::Delta).unwrap();
    assert_eq!(
        FormattedTemperature::prepare(delta, T::Kelvin, 3),
        Err(R::UnsupportedQuantity)
    );
    let huge = Quantity::from_decimal(1, 128, Unit::Celsius).unwrap();
    assert!(FormattedTemperature::prepare(huge, T::Celsius, 3).is_err());
    let tiny = Quantity::from_decimal(-1, -128, Unit::Celsius).unwrap();
    assert_eq!(
        FormattedTemperature::prepare(tiny, T::Celsius, 9)
            .unwrap()
            .number(),
        "0.000000000"
    );
    assert_eq!(
        rounded_quantity_coordinate(q("59%"), Unit::Percent, 3),
        Ok(59_000)
    );
}

#[test]
fn tiny_affine_inputs_round_after_wide_exact_conversion_before_integer_narrowing() {
    for coefficient in [-1, 1] {
        let source = Quantity::from_decimal(coefficient, -128, Unit::Celsius).unwrap();
        assert_eq!(
            FormattedTemperature::prepare(source, T::Kelvin, 3)
                .unwrap()
                .label(),
            "273.150 K"
        );
        assert_eq!(
            FormattedTemperature::prepare(source, T::Fahrenheit, 3)
                .unwrap()
                .label(),
            "32.000 °F"
        );
    }
}
