use conduit_core::{
    Quantity, QuantityConversionRefusal, QuantityDimension, QuantityRefusal, Unit, UnitRefusal,
};
use core::cmp::Ordering;

#[test]
fn reviewed_families_have_exact_distinct_dimensions() {
    assert_eq!(Unit::Millisecond.dimension(), QuantityDimension::Time);
    assert_eq!(Unit::Hertz.dimension(), QuantityDimension::Frequency);
    assert_eq!(Unit::Volt.dimension(), QuantityDimension::Voltage);
    assert_eq!(Unit::Meter.dimension(), QuantityDimension::Length);
    assert_eq!(Unit::Degree.dimension(), QuantityDimension::Angle);
    assert_eq!(Unit::Percent.dimension(), QuantityDimension::Ratio);
    assert_eq!(Unit::Byte.dimension(), QuantityDimension::DataSize);
}

#[test]
fn exact_decimal_and_binary_conversions_are_deterministic() {
    let vectors = [
        (1, Unit::Second, 1_000, Unit::Millisecond),
        (440, Unit::Hertz, 440_000, Unit::Millihertz),
        (3, Unit::Volt, 3_000, Unit::Millivolt),
        (18, Unit::Meter, 1_800, Unit::Centimeter),
        (90, Unit::Degree, 90_000, Unit::Millidegree),
        (72, Unit::Percent, 720_000, Unit::Millionth),
        (75, Unit::Percent, 750, Unit::Permille),
        (2, Unit::Kibibyte, 2_048, Unit::Byte),
    ];
    for (source, source_unit, expected, target_unit) in vectors {
        assert_eq!(
            Quantity::new(source, source_unit).convert(target_unit),
            Ok(Quantity::new(expected, target_unit))
        );
    }
}

#[test]
fn signed_physical_values_remain_exact() {
    assert_eq!(
        Quantity::new(-3, Unit::Millivolt).convert(Unit::Microvolt),
        Ok(Quantity::new(-3_000, Unit::Microvolt))
    );
}

#[test]
fn affine_temperature_conversion_is_explicit_and_exact_only() {
    assert_eq!(
        Quantity::new(0, Unit::Celsius).convert(Unit::Millikelvin),
        Ok(Quantity::new(273_150, Unit::Millikelvin))
    );
    assert_eq!(
        Quantity::new(273_150, Unit::Millikelvin).convert(Unit::Celsius),
        Ok(Quantity::new(0, Unit::Celsius))
    );
    assert_eq!(
        Quantity::new(273_151, Unit::Millikelvin).to_i64(Unit::Celsius),
        Err(QuantityConversionRefusal::Inexact)
    );
    assert_eq!(
        Quantity::new(32, Unit::Fahrenheit).convert(Unit::Celsius),
        Ok(Quantity::new(0, Unit::Celsius))
    );
    assert_eq!(
        Quantity::new(21, Unit::Celsius).convert(Unit::MilliFahrenheit),
        Ok(Quantity::new(69_800, Unit::MilliFahrenheit))
    );
    assert_eq!(
        Quantity::new(1, Unit::Fahrenheit).convert(Unit::Millikelvin),
        Err(QuantityConversionRefusal::Inexact)
    );
}

#[test]
fn current_charge_and_radian_families_are_concrete_and_angle_cross_conversion_refuses() {
    assert_eq!(
        Quantity::new(2, Unit::Ampere).convert(Unit::Milliampere),
        Ok(Quantity::new(2_000, Unit::Milliampere))
    );
    assert_eq!(
        Quantity::new(3, Unit::AmpereHour).convert(Unit::MilliampereHour),
        Ok(Quantity::new(3_000, Unit::MilliampereHour))
    );
    assert_eq!(
        Quantity::new(1, Unit::Radian).convert(Unit::Microdegree),
        Err(QuantityConversionRefusal::Inexact)
    );
}

#[test]
fn incompatible_and_inexact_conversions_refuse_without_rounding() {
    assert_eq!(
        Quantity::new(1, Unit::Second).convert(Unit::Hertz),
        Err(QuantityConversionRefusal::IncompatibleDimensions)
    );
    assert_eq!(
        Quantity::new(1, Unit::Millisecond).to_i64(Unit::Second),
        Err(QuantityConversionRefusal::Inexact)
    );
    assert_eq!(
        Quantity::new(1, Unit::Byte).to_i64(Unit::Kibibyte),
        Err(QuantityConversionRefusal::Inexact)
    );
}

#[test]
fn percentage_and_dimensionless_one_convert_without_float_truth() {
    assert_eq!(
        Quantity::new(100, Unit::Percent).convert(Unit::One),
        Ok(Quantity::new(1, Unit::One))
    );
    assert_eq!(
        Quantity::new(1, Unit::One).convert(Unit::Percent),
        Ok(Quantity::new(100, Unit::Percent))
    );
}

#[test]
fn civil_and_medieval_time_units_convert_exactly() {
    assert_eq!(
        Quantity::new(1, Unit::Moment).convert(Unit::Second),
        Ok(Quantity::new(90, Unit::Second))
    );
    assert_eq!(
        Quantity::new(40, Unit::Moment).convert(Unit::Hour),
        Ok(Quantity::new(1, Unit::Hour))
    );
    assert_eq!(
        Quantity::new(1, Unit::Fortnight).convert(Unit::Day),
        Ok(Quantity::new(14, Unit::Day))
    );
    assert_eq!(
        Quantity::parse_plot_literal("3moment"),
        Ok(Quantity::new(3, Unit::Moment))
    );
}

#[test]
fn uncommon_engineering_historical_and_scientific_units_are_exact() {
    assert_eq!(
        Quantity::new(1, Unit::Furlong).convert(Unit::Chain),
        Ok(Quantity::new(10, Unit::Chain))
    );
    assert_eq!(
        Quantity::new(1, Unit::League).convert(Unit::Mile),
        Ok(Quantity::new(3, Unit::Mile))
    );
    assert_eq!(
        Quantity::new(1, Unit::Inch).convert(Unit::Micrometer),
        Ok(Quantity::new(25_400, Unit::Micrometer))
    );
    assert_eq!(
        Quantity::new(8, Unit::Bit).convert(Unit::Byte),
        Ok(Quantity::new(1, Unit::Byte))
    );
    assert_eq!(
        Quantity::new(60, Unit::Arcsecond).convert(Unit::Arcminute),
        Ok(Quantity::new(1, Unit::Arcminute))
    );
    assert!(Unit::AstronomicalUnit
        .semantic_id()
        .starts_with("physical/unit/"));
    assert_ne!(
        Unit::AstronomicalUnit.definition_identity(),
        Unit::Meter.definition_identity()
    );
}

#[test]
fn si_and_named_derived_quantities_share_exact_dimension_laws() {
    assert_eq!(
        Quantity::new(1, Unit::Kilogram).convert(Unit::Gram),
        Ok(Quantity::new(1_000, Unit::Gram))
    );
    assert_eq!(
        Quantity::new(8, Unit::Ounce).convert(Unit::Microgram),
        Ok(Quantity::new(226_796_185, Unit::Microgram))
    );
    assert_eq!(
        Quantity::new(5, Unit::Acre).convert(Unit::SquareMillimeter),
        Ok(Quantity::new(20_234_282_112, Unit::SquareMillimeter))
    );
    assert_eq!(
        Quantity::new(1_000, Unit::Liter).convert(Unit::CubicMeter),
        Ok(Quantity::new(1, Unit::CubicMeter))
    );
    assert_eq!(
        Quantity::new(36, Unit::KilometerPerHour).convert(Unit::MeterPerSecond),
        Ok(Quantity::new(10, Unit::MeterPerSecond))
    );
    assert_eq!(
        Quantity::new(1, Unit::KilowattHour).convert(Unit::Kilojoule),
        Ok(Quantity::new(3_600, Unit::Kilojoule))
    );
    assert_eq!(
        Quantity::new(760, Unit::Torr).convert(Unit::Pascal),
        Ok(Quantity::new(101_325, Unit::Pascal))
    );
}

#[test]
fn canonical_multiplication_overflow_is_explicit() {
    assert_eq!(
        Quantity::new(i64::MAX, Unit::Second).to_i64(Unit::Nanosecond),
        Err(QuantityConversionRefusal::Overflow)
    );
    assert_eq!(
        Quantity::new(i64::MAX, Unit::Second).convert(Unit::Second),
        Ok(Quantity::new(i64::MAX, Unit::Second))
    );
}

#[test]
fn every_reviewed_unit_has_one_round_tripping_plot_suffix() {
    let units = [
        Unit::Nanosecond,
        Unit::Microsecond,
        Unit::Millisecond,
        Unit::Second,
        Unit::Millihertz,
        Unit::Hertz,
        Unit::Microvolt,
        Unit::Millivolt,
        Unit::Volt,
        Unit::Micrometer,
        Unit::Millimeter,
        Unit::Centimeter,
        Unit::Meter,
        Unit::Microdegree,
        Unit::Millidegree,
        Unit::Degree,
        Unit::Millionth,
        Unit::Permille,
        Unit::Percent,
        Unit::One,
        Unit::Byte,
        Unit::Kibibyte,
        Unit::Mebibyte,
        Unit::MilliFahrenheit,
        Unit::Fahrenheit,
        Unit::Pixel,
    ];
    for unit in units {
        let literal = format!("-17{}", unit.plot_suffix());
        assert_eq!(
            Quantity::parse_plot_literal(&literal),
            Ok(Quantity::new(-17, unit))
        );
    }
}

#[test]
fn scientific_plot_literals_preserve_reviewed_units_and_exact_decimals() {
    assert_eq!(
        Quantity::parse_plot_literal("21°C"),
        Ok(Quantity::new(21, Unit::Celsius))
    );
    assert_eq!(
        Quantity::parse_plot_literal("3.2m"),
        Quantity::from_decimal(32, -1, Unit::Meter)
    );
    assert_eq!(
        Quantity::parse_plot_literal("90°"),
        Ok(Quantity::new(90, Unit::Degree))
    );
    assert_eq!(
        Quantity::parse_plot_literal("640px"),
        Ok(Quantity::new(640, Unit::Pixel))
    );
    assert_eq!(
        Quantity::parse_plot_literal("69.8°F"),
        Quantity::from_decimal(698, -1, Unit::Fahrenheit)
    );
}

#[test]
fn compatible_units_compare_through_the_shared_exact_dimension_law() {
    assert_eq!(
        Quantity::new(30, Unit::Celsius).compare(Quantity::new(86, Unit::Fahrenheit)),
        Ok(Ordering::Equal)
    );
    assert_eq!(
        Quantity::new(21, Unit::Celsius).compare(Quantity::new(300, Unit::Kelvin)),
        Ok(Ordering::Less)
    );
    assert_eq!(
        Quantity::new(640, Unit::Pixel).compare(Quantity::new(1, Unit::Meter)),
        Err(QuantityConversionRefusal::IncompatibleDimensions)
    );
}

#[test]
fn plot_literals_refuse_unknown_parts_and_admit_exact_decimal_quantities() {
    for source in ["ms", "17", "17parsec", "21C", "1μm"] {
        assert!(Quantity::parse_plot_literal(source).is_err(), "{source}");
    }
    assert_eq!(
        Quantity::parse_plot_literal("ms"),
        Err(QuantityRefusal::InvalidNumber)
    );
    assert_eq!(
        Quantity::parse_plot_literal("21C"),
        Err(QuantityRefusal::Unit(UnitRefusal::UnknownSymbol))
    );
    assert_eq!(
        Quantity::parse_plot_literal("0.1ps")
            .unwrap()
            .to_i64(Unit::Picosecond),
        Err(QuantityConversionRefusal::Inexact)
    );
    assert!(Quantity::parse_plot_literal("9223372036854775808ms").is_ok());
}
