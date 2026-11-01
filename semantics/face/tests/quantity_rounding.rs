use conduit_core::{Quantity, Unit};
use conduit_face::{rounded_quantity_coordinate as rounded, QuantityFormatRefusal as R};
fn q(source: &str) -> Quantity {
    Quantity::parse_plot_literal(source).unwrap()
}

#[test]
fn exact_affine_temperature_coordinates_preserve_the_source() {
    let source = q("19.8°C");
    let encoded = source.encode();
    for (unit, expected) in [
        (Unit::Celsius, 19_800),
        (Unit::Kelvin, 292_950),
        (Unit::Fahrenheit, 67_640),
    ] {
        assert_eq!(rounded(source, unit, 3), Ok(expected));
        assert_eq!(source.encode(), encoded);
    }
    let nonterminating = q("33°F");
    assert_eq!(rounded(nonterminating, Unit::Celsius, 6), Ok(555_556));
    assert_eq!(rounded(nonterminating, Unit::Kelvin, 6), Ok(273_705_556));
    // Rounding 0.45 Celsius to tenths first would incorrectly produce 1.
    assert_eq!(rounded(q("32.81°F"), Unit::Celsius, 0), Ok(0));
}

#[test]
fn nearest_ties_are_away_from_zero_and_tiny_affine_values_stay_exact_until_rounding() {
    assert_eq!(rounded(q("32.9°F"), Unit::Celsius, 0), Ok(1));
    assert_eq!(rounded(q("31.1°F"), Unit::Celsius, 0), Ok(-1));
    assert_eq!(rounded(q("-0.0004°C"), Unit::Celsius, 3), Ok(0));
    assert_eq!(rounded(q("-273.15°C"), Unit::Kelvin, 3), Ok(0));
    for coefficient in [-1, 1] {
        let tiny = Quantity::from_decimal(coefficient, -128, Unit::Celsius).unwrap();
        assert_eq!(rounded(tiny, Unit::Celsius, 9), Ok(0));
        assert_eq!(rounded(tiny, Unit::Kelvin, 3), Ok(273_150));
        assert_eq!(rounded(tiny, Unit::Fahrenheit, 3), Ok(32_000));
    }
}

#[test]
fn dimension_precision_and_output_extent_refuse_explicitly() {
    assert_eq!(
        rounded(q("59%"), Unit::Celsius, 3),
        Err(R::UnsupportedQuantity)
    );
    assert_eq!(rounded(q("1°C"), Unit::Celsius, 10), Err(R::DecimalPlaces));
    assert_eq!(
        rounded(
            Quantity::from_decimal(1, 128, Unit::Celsius).unwrap(),
            Unit::Celsius,
            0
        ),
        Err(R::Overflow)
    );
    assert_eq!(rounded(q("59%"), Unit::Percent, 3), Ok(59_000));
}
