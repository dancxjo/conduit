//! Legacy authored quantity parsing and exact finite storage selection.

use super::conversion::{convert_exact_rational, is_radian};
use super::{
    Quantity, QuantityConversionRefusal, QuantityDimension, QuantityLiteralRefusal, QuantityUnit,
};

pub(super) fn parse_plot_literal(literal: &str) -> Result<Quantity, QuantityLiteralRefusal> {
    let value_end = literal
        .char_indices()
        .find_map(|(index, character)| {
            (!(character.is_ascii_digit() || character == '.' || (index == 0 && character == '-')))
                .then_some(index)
        })
        .unwrap_or(literal.len());
    let (value, suffix) = literal.split_at(value_end);
    if value.is_empty() || value == "-" {
        return Err(QuantityLiteralRefusal::MissingValue);
    }
    let resolved =
        crate::ResolvedQuantitySuffix::resolve(suffix).map_err(|refusal| match refusal {
            crate::QuantitySuffixRefusal::Literal(refusal) => refusal,
            crate::QuantitySuffixRefusal::Ambiguous => QuantityLiteralRefusal::AmbiguousUnit,
        })?;
    let Some(unit) = resolved.legacy_unit() else {
        return super::literal_eligibility::project_extended_literal(literal);
    };
    let Some((whole, fraction)) = value.split_once('.') else {
        let value = value
            .parse::<i64>()
            .map_err(|_| QuantityLiteralRefusal::InvalidValue)?;
        return Ok(Quantity::new(value, unit));
    };
    parse_exact_decimal(whole, fraction, unit)
}

fn parse_exact_decimal(
    whole: &str,
    fraction: &str,
    unit: QuantityUnit,
) -> Result<Quantity, QuantityLiteralRefusal> {
    if whole.is_empty()
        || whole == "-"
        || fraction.is_empty()
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(QuantityLiteralRefusal::InvalidValue);
    }
    let negative = whole.starts_with('-');
    let whole_magnitude = whole
        .trim_start_matches('-')
        .parse::<u64>()
        .map_err(|_| QuantityLiteralRefusal::InvalidValue)?;
    let exponent = u32::try_from(fraction.len()).map_err(|_| QuantityLiteralRefusal::Overflow)?;
    let denominator = 10_u64
        .checked_pow(exponent)
        .ok_or(QuantityLiteralRefusal::Overflow)?;
    let fraction = fraction
        .parse::<u64>()
        .map_err(|_| QuantityLiteralRefusal::InvalidValue)?;
    let numerator = whole_magnitude
        .checked_mul(denominator)
        .and_then(|value| value.checked_add(fraction))
        .ok_or(QuantityLiteralRefusal::Overflow)?;
    for target in ALL_QUANTITY_UNITS {
        if target.dimension() != unit.dimension()
            || (unit.dimension() == QuantityDimension::Angle
                && is_radian(unit) != is_radian(target))
            || (unit.dimension() == QuantityDimension::Temperature
                && temperature_family(unit) != temperature_family(target))
        {
            continue;
        }
        let signed_numerator = i128::from(numerator)
            .checked_mul(if negative { -1 } else { 1 })
            .ok_or(QuantityLiteralRefusal::Overflow)?;
        match convert_exact_rational(signed_numerator, i128::from(denominator), unit, target) {
            Ok(value) => return Ok(value),
            Err(QuantityConversionRefusal::Inexact) => {}
            Err(QuantityConversionRefusal::Overflow) => {
                return Err(QuantityLiteralRefusal::Overflow);
            }
            Err(QuantityConversionRefusal::IncompatibleDimensions) => unreachable!(),
        }
    }
    Err(QuantityLiteralRefusal::Inexact)
}

pub(super) const fn temperature_family(unit: QuantityUnit) -> u8 {
    match unit {
        QuantityUnit::Millikelvin | QuantityUnit::Kelvin => 1,
        QuantityUnit::MilliCelsius | QuantityUnit::Celsius => 2,
        QuantityUnit::MilliFahrenheit | QuantityUnit::Fahrenheit => 3,
        _ => 0,
    }
}

pub(super) const ALL_QUANTITY_UNITS: [QuantityUnit; 130] = [
    QuantityUnit::Picosecond,
    QuantityUnit::Nanosecond,
    QuantityUnit::Shake,
    QuantityUnit::Microsecond,
    QuantityUnit::Millisecond,
    QuantityUnit::Second,
    QuantityUnit::Minute,
    QuantityUnit::Moment,
    QuantityUnit::Hour,
    QuantityUnit::Day,
    QuantityUnit::Week,
    QuantityUnit::Fortnight,
    QuantityUnit::JulianYear,
    QuantityUnit::Millihertz,
    QuantityUnit::Hertz,
    QuantityUnit::Kilohertz,
    QuantityUnit::Megahertz,
    QuantityUnit::Gigahertz,
    QuantityUnit::Nanovolt,
    QuantityUnit::Microvolt,
    QuantityUnit::Millivolt,
    QuantityUnit::Volt,
    QuantityUnit::Kilovolt,
    QuantityUnit::Nanoampere,
    QuantityUnit::Microampere,
    QuantityUnit::Milliampere,
    QuantityUnit::Ampere,
    QuantityUnit::Kiloampere,
    QuantityUnit::Millikelvin,
    QuantityUnit::Kelvin,
    QuantityUnit::MilliCelsius,
    QuantityUnit::Celsius,
    QuantityUnit::MilliFahrenheit,
    QuantityUnit::Fahrenheit,
    QuantityUnit::MicroampereHour,
    QuantityUnit::MilliampereHour,
    QuantityUnit::AmpereHour,
    QuantityUnit::Micrometer,
    QuantityUnit::Nanometer,
    QuantityUnit::Millimeter,
    QuantityUnit::Centimeter,
    QuantityUnit::Meter,
    QuantityUnit::Kilometer,
    QuantityUnit::Angstrom,
    QuantityUnit::Inch,
    QuantityUnit::Hand,
    QuantityUnit::Foot,
    QuantityUnit::Yard,
    QuantityUnit::Rod,
    QuantityUnit::Chain,
    QuantityUnit::Furlong,
    QuantityUnit::Mile,
    QuantityUnit::League,
    QuantityUnit::NauticalMile,
    QuantityUnit::AstronomicalUnit,
    QuantityUnit::Microdegree,
    QuantityUnit::Millidegree,
    QuantityUnit::Degree,
    QuantityUnit::Arcsecond,
    QuantityUnit::Arcminute,
    QuantityUnit::Gradian,
    QuantityUnit::Turn,
    QuantityUnit::Microradian,
    QuantityUnit::Milliradian,
    QuantityUnit::Radian,
    QuantityUnit::Millionth,
    QuantityUnit::PartPerBillion,
    QuantityUnit::BasisPoint,
    QuantityUnit::Permille,
    QuantityUnit::Percent,
    QuantityUnit::One,
    QuantityUnit::Bit,
    QuantityUnit::Byte,
    QuantityUnit::Kilobyte,
    QuantityUnit::Megabyte,
    QuantityUnit::Gigabyte,
    QuantityUnit::Kibibyte,
    QuantityUnit::Mebibyte,
    QuantityUnit::Gibibyte,
    QuantityUnit::Terabyte,
    QuantityUnit::Tebibyte,
    QuantityUnit::Microgram,
    QuantityUnit::Milligram,
    QuantityUnit::Gram,
    QuantityUnit::Kilogram,
    QuantityUnit::Tonne,
    QuantityUnit::Ounce,
    QuantityUnit::Pound,
    QuantityUnit::Stone,
    QuantityUnit::SquareMillimeter,
    QuantityUnit::SquareCentimeter,
    QuantityUnit::SquareMeter,
    QuantityUnit::Hectare,
    QuantityUnit::SquareKilometer,
    QuantityUnit::Acre,
    QuantityUnit::Microliter,
    QuantityUnit::Milliliter,
    QuantityUnit::Liter,
    QuantityUnit::CubicMeter,
    QuantityUnit::MillimeterPerSecond,
    QuantityUnit::MeterPerSecond,
    QuantityUnit::KilometerPerHour,
    QuantityUnit::MilePerHour,
    QuantityUnit::Knot,
    QuantityUnit::MeterPerSecondSquared,
    QuantityUnit::StandardGravity,
    QuantityUnit::Gal,
    QuantityUnit::Millinewton,
    QuantityUnit::Newton,
    QuantityUnit::Kilonewton,
    QuantityUnit::Millijoule,
    QuantityUnit::Joule,
    QuantityUnit::Kilojoule,
    QuantityUnit::Megajoule,
    QuantityUnit::WattHour,
    QuantityUnit::KilowattHour,
    QuantityUnit::Calorie,
    QuantityUnit::Kilocalorie,
    QuantityUnit::Milliwatt,
    QuantityUnit::Watt,
    QuantityUnit::Kilowatt,
    QuantityUnit::Megawatt,
    QuantityUnit::Pascal,
    QuantityUnit::Kilopascal,
    QuantityUnit::Megapascal,
    QuantityUnit::Bar,
    QuantityUnit::Millibar,
    QuantityUnit::Atmosphere,
    QuantityUnit::Torr,
    QuantityUnit::Pixel,
];
