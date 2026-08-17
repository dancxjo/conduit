use conduit_core::{
    Quantity, QuantityConversionRefusal, QuantityDimension, QuantityLiteralRefusal, QuantityUnit,
};

#[test]
fn reviewed_families_have_exact_distinct_dimensions() {
    assert_eq!(
        QuantityUnit::Millisecond.dimension(),
        QuantityDimension::Time
    );
    assert_eq!(
        QuantityUnit::Hertz.dimension(),
        QuantityDimension::Frequency
    );
    assert_eq!(QuantityUnit::Volt.dimension(), QuantityDimension::Voltage);
    assert_eq!(QuantityUnit::Meter.dimension(), QuantityDimension::Length);
    assert_eq!(QuantityUnit::Degree.dimension(), QuantityDimension::Angle);
    assert_eq!(QuantityUnit::Percent.dimension(), QuantityDimension::Ratio);
    assert_eq!(QuantityUnit::Byte.dimension(), QuantityDimension::DataSize);
}

#[test]
fn exact_decimal_and_binary_conversions_are_deterministic() {
    let vectors = [
        (1, QuantityUnit::Second, 1_000, QuantityUnit::Millisecond),
        (440, QuantityUnit::Hertz, 440_000, QuantityUnit::Millihertz),
        (3, QuantityUnit::Volt, 3_000, QuantityUnit::Millivolt),
        (18, QuantityUnit::Meter, 1_800, QuantityUnit::Centimeter),
        (90, QuantityUnit::Degree, 90_000, QuantityUnit::Millidegree),
        (72, QuantityUnit::Percent, 720_000, QuantityUnit::Millionth),
        (75, QuantityUnit::Percent, 750, QuantityUnit::Permille),
        (2, QuantityUnit::Kibibyte, 2_048, QuantityUnit::Byte),
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
        Quantity::new(-3, QuantityUnit::Millivolt).convert(QuantityUnit::Microvolt),
        Ok(Quantity::new(-3_000, QuantityUnit::Microvolt))
    );
}

#[test]
fn incompatible_and_inexact_conversions_refuse_without_rounding() {
    assert_eq!(
        Quantity::new(1, QuantityUnit::Second).convert(QuantityUnit::Hertz),
        Err(QuantityConversionRefusal::IncompatibleDimensions)
    );
    assert_eq!(
        Quantity::new(1, QuantityUnit::Millisecond).convert(QuantityUnit::Second),
        Err(QuantityConversionRefusal::Inexact)
    );
    assert_eq!(
        Quantity::new(1, QuantityUnit::Byte).convert(QuantityUnit::Kibibyte),
        Err(QuantityConversionRefusal::Inexact)
    );
}

#[test]
fn percentage_and_dimensionless_one_convert_without_float_truth() {
    assert_eq!(
        Quantity::new(100, QuantityUnit::Percent).convert(QuantityUnit::One),
        Ok(Quantity::new(1, QuantityUnit::One))
    );
    assert_eq!(
        Quantity::new(1, QuantityUnit::One).convert(QuantityUnit::Percent),
        Ok(Quantity::new(100, QuantityUnit::Percent))
    );
}

#[test]
fn canonical_multiplication_overflow_is_explicit() {
    assert_eq!(
        Quantity::new(i64::MAX, QuantityUnit::Second).convert(QuantityUnit::Nanosecond),
        Err(QuantityConversionRefusal::Overflow)
    );
    assert_eq!(
        Quantity::new(i64::MAX, QuantityUnit::Second).convert(QuantityUnit::Second),
        Ok(Quantity::new(i64::MAX, QuantityUnit::Second))
    );
}

#[test]
fn every_reviewed_unit_has_one_round_tripping_form_suffix() {
    let units = [
        QuantityUnit::Nanosecond,
        QuantityUnit::Microsecond,
        QuantityUnit::Millisecond,
        QuantityUnit::Second,
        QuantityUnit::Millihertz,
        QuantityUnit::Hertz,
        QuantityUnit::Microvolt,
        QuantityUnit::Millivolt,
        QuantityUnit::Volt,
        QuantityUnit::Micrometer,
        QuantityUnit::Millimeter,
        QuantityUnit::Centimeter,
        QuantityUnit::Meter,
        QuantityUnit::Microdegree,
        QuantityUnit::Millidegree,
        QuantityUnit::Degree,
        QuantityUnit::Millionth,
        QuantityUnit::Permille,
        QuantityUnit::Percent,
        QuantityUnit::One,
        QuantityUnit::Byte,
        QuantityUnit::Kibibyte,
        QuantityUnit::Mebibyte,
    ];
    for unit in units {
        let literal = format!("-17{}", unit.form_suffix());
        assert_eq!(
            Quantity::parse_form_literal(&literal),
            Ok(Quantity::new(-17, unit))
        );
    }
}

#[test]
fn form_literals_refuse_missing_unknown_fractional_and_overflowing_parts() {
    assert_eq!(
        Quantity::parse_form_literal("ms"),
        Err(QuantityLiteralRefusal::MissingValue)
    );
    assert_eq!(
        Quantity::parse_form_literal("17"),
        Err(QuantityLiteralRefusal::MissingUnit)
    );
    assert_eq!(
        Quantity::parse_form_literal("17fortnight"),
        Err(QuantityLiteralRefusal::UnknownUnit)
    );
    assert_eq!(
        Quantity::parse_form_literal("1.5s"),
        Err(QuantityLiteralRefusal::UnknownUnit)
    );
    assert_eq!(
        Quantity::parse_form_literal("9223372036854775808ms"),
        Err(QuantityLiteralRefusal::InvalidValue)
    );
}
