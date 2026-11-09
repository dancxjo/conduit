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

// Preserve the exact pre-staging algorithm as an independent regression oracle:
// all admitted wide extents, operation order, work budget and refusal mappings.
use conduit_core::{
    ExactNumericRefusal, ExactNumericWork, ExactRational, IntegerEnclosureRefusal,
    PreparedIntegerEnclosure, PrimitiveInfoKind, QuantityConversionRefusal, RationalQuantity,
};
fn signed(magnitude: u128, negative: bool) -> Result<i128, R> {
    if negative && magnitude == 1u128 << 127 {
        return Ok(i128::MIN);
    }
    let magnitude = i128::try_from(magnitude).map_err(|_| R::Overflow)?;
    Ok(if negative { -magnitude } else { magnitude })
}
fn reference_rational(value: Quantity, unit: Unit, places: u8) -> Result<i128, R> {
    let mut work = ExactNumericWork::new(3 * conduit_core::MATHEMATICAL_VALUE_VALIDATION_WORK);
    let coordinate = RationalQuantity::from_quantity(value, &mut work)
        .and_then(|value| value.convert_to_unit(unit, &mut work))
        .map_err(|_| R::NumericExtent)?
        .coordinate();
    let coordinate = coordinate
        .checked_multiply(
            ExactRational::from_u128(10_u128.pow(u32::from(places))),
            &mut work,
        )
        .map_err(|_| R::NumericExtent)?;
    let magnitude = if coordinate.is_negative() {
        coordinate.negated()
    } else {
        coordinate
    };
    // floor(abs(coordinate) + 1/2) is nearest, ties away from zero.
    // Divide in Core's wide exact profile before narrowing the integer; large
    // numerator/denominator components do not imply a large display result.
    let half = ExactRational::ratio(1, 2, &mut work).map_err(|_| R::NumericExtent)?;
    let shifted = magnitude
        .checked_add(half, &mut work)
        .map_err(|_| R::NumericExtent)?;
    let enclosure = PreparedIntegerEnclosure::prepare(
        shifted,
        PrimitiveInfoKind::U128,
        ExactRational::ONE,
        &mut work,
    )
    .map_err(|why| match why {
        IntegerEnclosureRefusal::Numeric(ExactNumericRefusal::OutOfRange)
        | IntegerEnclosureRefusal::Target(_) => R::Overflow,
        _ => R::NumericExtent,
    })?;
    let rounded = enclosure
        .target()
        .lower()
        .unsigned()
        .map_err(|_| R::Overflow)?;
    signed(rounded, coordinate.is_negative())
}

#[test]
fn staged_wide_rounding_matches_original_full_extent_algorithm() {
    let units = [Unit::Celsius, Unit::Fahrenheit, Unit::Kelvin];
    let mut fallback_cases = 0;
    for coefficient in [i64::MIN as i128, -1, 1, i64::MAX as i128] {
        for exponent in [-128, -38, -1, 0, 37, 128] {
            for source_unit in units {
                let source = Quantity::from_decimal(coefficient, exponent, source_unit).unwrap();
                let original = source.encode();
                for target in units {
                    if !matches!(
                        source.convert_to_decimal(target),
                        Err(QuantityConversionRefusal::Inexact
                            | QuantityConversionRefusal::Overflow)
                    ) {
                        continue;
                    }
                    for places in [0, 3, 9] {
                        fallback_cases += 1;
                        assert_eq!(rounded(source, target, places), reference_rational(source, target, places),
                            "coefficient={coefficient} exponent={exponent} {source_unit:?}->{target:?} places={places}");
                        assert_eq!(
                            source.encode(),
                            original,
                            "formatting cannot mutate provenance"
                        );
                    }
                }
            }
        }
    }
    assert!(
        fallback_cases >= 100,
        "exercise broad exact fallback, not only decimal fast paths"
    );
}
