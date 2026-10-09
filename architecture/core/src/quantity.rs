//! Typed finite quantities with exact-only conversions.

mod conversion;
mod conversion_law;
mod exact;
mod literal;
mod literal_eligibility;
mod magnitude;
mod target;
mod wide_conversion;

pub use exact::*;
pub use target::*;

use conversion::{compare_legacy, convert_exact_rational, is_radian};
use core::cmp::Ordering;
use serde::{Deserialize, Serialize};

use crate::semantic_digest;

pub const QUANTITY_INFO_ID: &str = "value/quantity";
/// Exact length-dimension quantity. Uses the canonical Quantity wire encoding.
pub const DISTANCE_INFO_ID: &str = "value/distance";
/// Exact frequency-dimension quantity. Uses the canonical Quantity wire encoding.
pub const FREQUENCY_INFO_ID: &str = "value/frequency";
pub const DURATION_INFO_ID: &str = "value/duration";
pub const VOLTAGE_INFO_ID: &str = "value/voltage";
pub const TEMPERATURE_INFO_ID: &str = "value/temperature";
pub const ANGLE_INFO_ID: &str = "value/angle";
pub const RATIO_INFO_ID: &str = "value/ratio";
/// Pixel count is an image/display dimension, never physical length.
pub const PIXEL_COUNT_INFO_ID: &str = "value/pixel-count";
pub const QUANTITY_ENCODED_LEN: usize = 9;
pub const QUANTITY_UNIT_INFO_ID: &str = "value/quantity-unit";
pub const QUANTITY_UNIT_ENCODED_LEN: usize = 1;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum QuantityDimension {
    Time,
    Frequency,
    Voltage,
    Current,
    Temperature,
    Charge,
    Length,
    Angle,
    Ratio,
    DataSize,
    PixelCount,
    Mass,
    Area,
    Volume,
    Speed,
    Acceleration,
    Force,
    Energy,
    Power,
    Pressure,
}

impl QuantityDimension {
    pub const fn info_id(self) -> &'static str {
        match self {
            Self::Time => DURATION_INFO_ID,
            Self::Frequency => FREQUENCY_INFO_ID,
            Self::Voltage => VOLTAGE_INFO_ID,
            Self::Temperature => TEMPERATURE_INFO_ID,
            Self::Length => DISTANCE_INFO_ID,
            Self::Angle => ANGLE_INFO_ID,
            Self::Ratio => RATIO_INFO_ID,
            Self::PixelCount => PIXEL_COUNT_INFO_ID,
            // These dimensions have reviewed units and exact conversion laws,
            // but do not yet own public dimension-specific Plot aliases.
            Self::Current
            | Self::Charge
            | Self::DataSize
            | Self::Mass
            | Self::Area
            | Self::Volume
            | Self::Speed
            | Self::Acceleration
            | Self::Force
            | Self::Energy
            | Self::Power
            | Self::Pressure => QUANTITY_INFO_ID,
        }
    }
}

pub fn quantity_info_dimension(identity: &str) -> Option<QuantityDimension> {
    match identity {
        DURATION_INFO_ID => Some(QuantityDimension::Time),
        FREQUENCY_INFO_ID => Some(QuantityDimension::Frequency),
        VOLTAGE_INFO_ID => Some(QuantityDimension::Voltage),
        TEMPERATURE_INFO_ID => Some(QuantityDimension::Temperature),
        DISTANCE_INFO_ID => Some(QuantityDimension::Length),
        ANGLE_INFO_ID => Some(QuantityDimension::Angle),
        RATIO_INFO_ID => Some(QuantityDimension::Ratio),
        PIXEL_COUNT_INFO_ID => Some(QuantityDimension::PixelCount),
        _ => None,
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum QuantityUnit {
    Picosecond,
    Nanosecond,
    Shake,
    Microsecond,
    Millisecond,
    Second,
    Minute,
    Moment,
    Hour,
    Day,
    Week,
    Fortnight,
    JulianYear,
    Millihertz,
    Hertz,
    Kilohertz,
    Megahertz,
    Gigahertz,
    Nanovolt,
    Microvolt,
    Millivolt,
    Volt,
    Kilovolt,
    Nanoampere,
    Microampere,
    Milliampere,
    Ampere,
    Kiloampere,
    Millikelvin,
    Kelvin,
    MilliCelsius,
    Celsius,
    MilliFahrenheit,
    Fahrenheit,
    MicroampereHour,
    MilliampereHour,
    AmpereHour,
    Micrometer,
    Nanometer,
    Millimeter,
    Centimeter,
    Meter,
    Kilometer,
    Angstrom,
    Inch,
    Hand,
    Foot,
    Yard,
    Rod,
    Chain,
    Furlong,
    Mile,
    League,
    NauticalMile,
    AstronomicalUnit,
    Microdegree,
    Millidegree,
    Degree,
    Arcsecond,
    Arcminute,
    Gradian,
    Turn,
    Microradian,
    Milliradian,
    Radian,
    Millionth,
    PartPerBillion,
    BasisPoint,
    Permille,
    Percent,
    One,
    Bit,
    Byte,
    Kilobyte,
    Megabyte,
    Gigabyte,
    Kibibyte,
    Gibibyte,
    Terabyte,
    Tebibyte,
    Mebibyte,
    Microgram,
    Milligram,
    Gram,
    Kilogram,
    Tonne,
    Ounce,
    Pound,
    Stone,
    SquareMillimeter,
    SquareCentimeter,
    SquareMeter,
    Hectare,
    SquareKilometer,
    Acre,
    Microliter,
    Milliliter,
    Liter,
    CubicMeter,
    MillimeterPerSecond,
    MeterPerSecond,
    KilometerPerHour,
    MilePerHour,
    Knot,
    MeterPerSecondSquared,
    StandardGravity,
    Gal,
    Millinewton,
    Newton,
    Kilonewton,
    Millijoule,
    Joule,
    Kilojoule,
    Megajoule,
    WattHour,
    KilowattHour,
    Calorie,
    Kilocalorie,
    Milliwatt,
    Watt,
    Kilowatt,
    Megawatt,
    Pascal,
    Kilopascal,
    Megapascal,
    Bar,
    Millibar,
    Atmosphere,
    Torr,
    Pixel,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Quantity {
    value: i64,
    unit: QuantityUnit,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum QuantityConversionRefusal {
    IncompatibleDimensions,
    Inexact,
    Overflow,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum QuantityDecodeRefusal {
    WrongLength { expected: usize, actual: usize },
    UnknownUnitTag(u8),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum QuantityLiteralRefusal {
    MissingValue,
    InvalidValue,
    MissingUnit,
    UnknownUnit,
    AmbiguousUnit,
    RepresentationIneligible {
        profile: &'static str,
        reason: QuantityRepresentationRefusal,
    },
    NonCanonicalUnit {
        canonical: &'static str,
    },
    Inexact,
    Overflow,
}

/// Numeric eligibility is separate from suffix recognition. These refusals
/// identify the selected finite profile without classifying a known SI unit
/// as unknown or silently changing persisted quantity identities.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum QuantityRepresentationRefusal {
    NoExactLegacyUnit,
    LiteralTooLong,
    NumberTooLong,
    SignificantDigitsExceeded,
    ExponentOutOfRange,
}

impl QuantityUnit {
    pub const fn encode(self) -> [u8; QUANTITY_UNIT_ENCODED_LEN] {
        [self.tag()]
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, QuantityDecodeRefusal> {
        if encoded.len() != QUANTITY_UNIT_ENCODED_LEN {
            return Err(QuantityDecodeRefusal::WrongLength {
                expected: QUANTITY_UNIT_ENCODED_LEN,
                actual: encoded.len(),
            });
        }
        Self::from_tag(encoded[0])
    }

    pub const fn semantic_id(self) -> &'static str {
        match self {
            Self::Picosecond => "time/picosecond",
            Self::Nanosecond => "time/nanosecond",
            Self::Shake => "time/shake",
            Self::Microsecond => "time/microsecond",
            Self::Millisecond => "time/millisecond",
            Self::Second => "time/second",
            Self::Minute => "time/minute",
            Self::Moment => "time/medieval-moment",
            Self::Hour => "time/hour",
            Self::Day => "time/day",
            Self::Week => "time/week",
            Self::Fortnight => "time/fortnight",
            Self::JulianYear => "time/julian-year",
            Self::Millihertz => "frequency/millihertz",
            Self::Hertz => "frequency/hertz",
            Self::Kilohertz => "frequency/kilohertz",
            Self::Megahertz => "frequency/megahertz",
            Self::Gigahertz => "frequency/gigahertz",
            Self::Nanovolt => "voltage/nanovolt",
            Self::Microvolt => "voltage/microvolt",
            Self::Millivolt => "voltage/millivolt",
            Self::Volt => "voltage/volt",
            Self::Kilovolt => "voltage/kilovolt",
            Self::Nanoampere => "current/nanoampere",
            Self::Microampere => "current/microampere",
            Self::Milliampere => "current/milliampere",
            Self::Ampere => "current/ampere",
            Self::Kiloampere => "current/kiloampere",
            Self::Millikelvin => "temperature/millikelvin",
            Self::Kelvin => "temperature/kelvin",
            Self::MilliCelsius => "temperature/millicelsius",
            Self::Celsius => "temperature/celsius",
            Self::MilliFahrenheit => "temperature/millifahrenheit",
            Self::Fahrenheit => "temperature/fahrenheit",
            Self::MicroampereHour => "charge/microampere-hour",
            Self::MilliampereHour => "charge/milliampere-hour",
            Self::AmpereHour => "charge/ampere-hour",
            Self::Micrometer => "length/micrometer",
            Self::Nanometer => "length/nanometer",
            Self::Millimeter => "length/millimeter",
            Self::Centimeter => "length/centimeter",
            Self::Meter => "length/meter",
            Self::Kilometer => "length/kilometer",
            Self::Angstrom => "length/angstrom",
            Self::Inch => "length/inch",
            Self::Hand => "length/hand",
            Self::Foot => "length/foot",
            Self::Yard => "length/yard",
            Self::Rod => "length/rod",
            Self::Chain => "length/chain",
            Self::Furlong => "length/furlong",
            Self::Mile => "length/mile",
            Self::League => "length/league",
            Self::NauticalMile => "length/nautical-mile",
            Self::AstronomicalUnit => "length/astronomical-unit",
            Self::Microdegree => "angle/microdegree",
            Self::Millidegree => "angle/millidegree",
            Self::Degree => "angle/degree",
            Self::Arcsecond => "angle/arcsecond",
            Self::Arcminute => "angle/arcminute",
            Self::Gradian => "angle/gradian",
            Self::Turn => "angle/turn",
            Self::Microradian => "angle/microradian",
            Self::Milliradian => "angle/milliradian",
            Self::Radian => "angle/radian",
            Self::Millionth => "ratio/millionth",
            Self::PartPerBillion => "ratio/part-per-billion",
            Self::BasisPoint => "ratio/basis-point",
            Self::Permille => "ratio/permille",
            Self::Percent => "ratio/percent",
            Self::One => "ratio/one",
            Self::Bit => "data-size/bit",
            Self::Byte => "data-size/byte",
            Self::Kilobyte => "data-size/kilobyte",
            Self::Megabyte => "data-size/megabyte",
            Self::Gigabyte => "data-size/gigabyte",
            Self::Kibibyte => "data-size/kibibyte",
            Self::Mebibyte => "data-size/mebibyte",
            Self::Gibibyte => "data-size/gibibyte",
            Self::Terabyte => "data-size/terabyte",
            Self::Tebibyte => "data-size/tebibyte",
            Self::Microgram => "mass/microgram",
            Self::Milligram => "mass/milligram",
            Self::Gram => "mass/gram",
            Self::Kilogram => "mass/kilogram",
            Self::Tonne => "mass/tonne",
            Self::Ounce => "mass/avoirdupois-ounce",
            Self::Pound => "mass/avoirdupois-pound",
            Self::Stone => "mass/stone",
            Self::SquareMillimeter => "area/square-millimeter",
            Self::SquareCentimeter => "area/square-centimeter",
            Self::SquareMeter => "area/square-meter",
            Self::Hectare => "area/hectare",
            Self::SquareKilometer => "area/square-kilometer",
            Self::Acre => "area/international-acre",
            Self::Microliter => "volume/microliter",
            Self::Milliliter => "volume/milliliter",
            Self::Liter => "volume/liter",
            Self::CubicMeter => "volume/cubic-meter",
            Self::MillimeterPerSecond => "speed/millimeter-per-second",
            Self::MeterPerSecond => "speed/meter-per-second",
            Self::KilometerPerHour => "speed/kilometer-per-hour",
            Self::MilePerHour => "speed/mile-per-hour",
            Self::Knot => "speed/knot",
            Self::MeterPerSecondSquared => "acceleration/meter-per-second-squared",
            Self::StandardGravity => "acceleration/standard-gravity",
            Self::Gal => "acceleration/gal",
            Self::Millinewton => "force/millinewton",
            Self::Newton => "force/newton",
            Self::Kilonewton => "force/kilonewton",
            Self::Millijoule => "energy/millijoule",
            Self::Joule => "energy/joule",
            Self::Kilojoule => "energy/kilojoule",
            Self::Megajoule => "energy/megajoule",
            Self::WattHour => "energy/watt-hour",
            Self::KilowattHour => "energy/kilowatt-hour",
            Self::Calorie => "energy/thermochemical-calorie",
            Self::Kilocalorie => "energy/thermochemical-kilocalorie",
            Self::Milliwatt => "power/milliwatt",
            Self::Watt => "power/watt",
            Self::Kilowatt => "power/kilowatt",
            Self::Megawatt => "power/megawatt",
            Self::Pascal => "pressure/pascal",
            Self::Kilopascal => "pressure/kilopascal",
            Self::Megapascal => "pressure/megapascal",
            Self::Bar => "pressure/bar",
            Self::Millibar => "pressure/millibar",
            Self::Atmosphere => "pressure/standard-atmosphere",
            Self::Torr => "pressure/torr",
            Self::Pixel => "pixel/count",
        }
    }

    pub const fn plot_suffix(self) -> &'static str {
        match self {
            Self::Picosecond => "ps",
            Self::Nanosecond => "ns",
            Self::Shake => "shake",
            Self::Microsecond => "µs",
            Self::Millisecond => "ms",
            Self::Second => "s",
            Self::Minute => "min",
            Self::Moment => "moment",
            Self::Hour => "h",
            Self::Day => "d",
            Self::Week => "wk",
            Self::Fortnight => "fortnight",
            Self::JulianYear => "a_j",
            Self::Millihertz => "mHz",
            Self::Hertz => "Hz",
            Self::Kilohertz => "kHz",
            Self::Megahertz => "MHz",
            Self::Gigahertz => "GHz",
            Self::Nanovolt => "nV",
            Self::Microvolt => "µV",
            Self::Millivolt => "mV",
            Self::Volt => "V",
            Self::Kilovolt => "kV",
            Self::Nanoampere => "nA",
            Self::Microampere => "µA",
            Self::Milliampere => "mA",
            Self::Ampere => "A",
            Self::Kiloampere => "kA",
            Self::Millikelvin => "mK",
            Self::Kelvin => "K",
            Self::MilliCelsius => "m°C",
            Self::Celsius => "°C",
            Self::MilliFahrenheit => "m°F",
            Self::Fahrenheit => "°F",
            Self::MicroampereHour => "µAh",
            Self::MilliampereHour => "mAh",
            Self::AmpereHour => "Ah",
            Self::Micrometer => "µm",
            Self::Nanometer => "nm",
            Self::Millimeter => "mm",
            Self::Centimeter => "cm",
            Self::Meter => "m",
            Self::Kilometer => "km",
            Self::Angstrom => "Å",
            Self::Inch => "in",
            Self::Hand => "hand",
            Self::Foot => "ft",
            Self::Yard => "yd",
            Self::Rod => "rod",
            Self::Chain => "chain",
            Self::Furlong => "furlong",
            Self::Mile => "mi",
            Self::League => "league",
            Self::NauticalMile => "nmi",
            Self::AstronomicalUnit => "au",
            Self::Microdegree => "µ°",
            Self::Millidegree => "m°",
            Self::Degree => "°",
            Self::Arcsecond => "arcsec",
            Self::Arcminute => "arcmin",
            Self::Gradian => "gon",
            Self::Turn => "turn",
            Self::Microradian => "µrad",
            Self::Milliradian => "mrad",
            Self::Radian => "rad",
            Self::Millionth => "ppm",
            Self::PartPerBillion => "ppb",
            Self::BasisPoint => "bp",
            Self::Permille => "‰",
            Self::Percent => "%",
            Self::One => "one",
            Self::Bit => "bit",
            Self::Byte => "B",
            Self::Kilobyte => "kB",
            Self::Megabyte => "MB",
            Self::Gigabyte => "GB",
            Self::Kibibyte => "KiB",
            Self::Mebibyte => "MiB",
            Self::Gibibyte => "GiB",
            Self::Terabyte => "TB",
            Self::Tebibyte => "TiB",
            Self::Microgram => "µg",
            Self::Milligram => "mg",
            Self::Gram => "g",
            Self::Kilogram => "kg",
            Self::Tonne => "t",
            Self::Ounce => "oz_av",
            Self::Pound => "lb_av",
            Self::Stone => "st",
            Self::SquareMillimeter => "mm²",
            Self::SquareCentimeter => "cm²",
            Self::SquareMeter => "m²",
            Self::Hectare => "ha",
            Self::SquareKilometer => "km²",
            Self::Acre => "acre_int",
            Self::Microliter => "µL",
            Self::Milliliter => "mL",
            Self::Liter => "L",
            Self::CubicMeter => "m³",
            Self::MillimeterPerSecond => "mm/s",
            Self::MeterPerSecond => "m/s",
            Self::KilometerPerHour => "km/h",
            Self::MilePerHour => "mph",
            Self::Knot => "kn",
            Self::MeterPerSecondSquared => "m/s²",
            Self::StandardGravity => "g₀",
            Self::Gal => "Gal",
            Self::Millinewton => "mN",
            Self::Newton => "N",
            Self::Kilonewton => "kN",
            Self::Millijoule => "mJ",
            Self::Joule => "J",
            Self::Kilojoule => "kJ",
            Self::Megajoule => "MJ",
            Self::WattHour => "Wh",
            Self::KilowattHour => "kWh",
            Self::Calorie => "cal_th",
            Self::Kilocalorie => "kcal_th",
            Self::Milliwatt => "mW",
            Self::Watt => "W",
            Self::Kilowatt => "kW",
            Self::Megawatt => "MW",
            Self::Pascal => "Pa",
            Self::Kilopascal => "kPa",
            Self::Megapascal => "MPa",
            Self::Bar => "bar",
            Self::Millibar => "mbar",
            Self::Atmosphere => "atm",
            Self::Torr => "Torr",
            Self::Pixel => "px",
        }
    }

    pub fn from_plot_suffix(suffix: &str) -> Result<Self, QuantityLiteralRefusal> {
        match suffix {
            "ps" => Ok(Self::Picosecond),
            "ns" => Ok(Self::Nanosecond),
            "shake" => Ok(Self::Shake),
            "µs" | "us" => Ok(Self::Microsecond),
            "ms" => Ok(Self::Millisecond),
            "s" => Ok(Self::Second),
            "min" => Ok(Self::Minute),
            "moment" => Ok(Self::Moment),
            "h" => Ok(Self::Hour),
            "d" => Ok(Self::Day),
            "wk" => Ok(Self::Week),
            "fortnight" => Ok(Self::Fortnight),
            "a_j" => Ok(Self::JulianYear),
            "mHz" => Ok(Self::Millihertz),
            "Hz" => Ok(Self::Hertz),
            "kHz" => Ok(Self::Kilohertz),
            "MHz" => Ok(Self::Megahertz),
            "GHz" => Ok(Self::Gigahertz),
            "nV" => Ok(Self::Nanovolt),
            "µV" | "uV" => Ok(Self::Microvolt),
            "mV" => Ok(Self::Millivolt),
            "V" => Ok(Self::Volt),
            "kV" => Ok(Self::Kilovolt),
            "nA" => Ok(Self::Nanoampere),
            "µA" | "uA" => Ok(Self::Microampere),
            "mA" => Ok(Self::Milliampere),
            "A" => Ok(Self::Ampere),
            "kA" => Ok(Self::Kiloampere),
            "mK" => Ok(Self::Millikelvin),
            "K" => Ok(Self::Kelvin),
            "m°C" => Ok(Self::MilliCelsius),
            "°C" => Ok(Self::Celsius),
            "C" => Err(QuantityLiteralRefusal::NonCanonicalUnit { canonical: "°C" }),
            "m°F" => Ok(Self::MilliFahrenheit),
            "°F" => Ok(Self::Fahrenheit),
            "F" => Err(QuantityLiteralRefusal::NonCanonicalUnit { canonical: "°F" }),
            "µAh" | "uAh" => Ok(Self::MicroampereHour),
            "mAh" => Ok(Self::MilliampereHour),
            "Ah" => Ok(Self::AmpereHour),
            "µm" | "um" => Ok(Self::Micrometer),
            "nm" => Ok(Self::Nanometer),
            "mm" => Ok(Self::Millimeter),
            "cm" => Ok(Self::Centimeter),
            "m" => Ok(Self::Meter),
            "km" => Ok(Self::Kilometer),
            "Å" | "angstrom" => Ok(Self::Angstrom),
            "in" => Ok(Self::Inch),
            "hand" => Ok(Self::Hand),
            "ft" => Ok(Self::Foot),
            "yd" => Ok(Self::Yard),
            "rod" => Ok(Self::Rod),
            "chain" => Ok(Self::Chain),
            "furlong" => Ok(Self::Furlong),
            "mi" => Ok(Self::Mile),
            "league" => Ok(Self::League),
            "nmi" => Ok(Self::NauticalMile),
            "au" => Ok(Self::AstronomicalUnit),
            "µ°" | "udeg" => Ok(Self::Microdegree),
            "m°" | "mdeg" => Ok(Self::Millidegree),
            "°" | "deg" => Ok(Self::Degree),
            "arcsec" => Ok(Self::Arcsecond),
            "arcmin" => Ok(Self::Arcminute),
            "gon" => Ok(Self::Gradian),
            "turn" => Ok(Self::Turn),
            "µrad" | "urad" => Ok(Self::Microradian),
            "mrad" => Ok(Self::Milliradian),
            "rad" => Ok(Self::Radian),
            "ppm" => Ok(Self::Millionth),
            "ppb" => Ok(Self::PartPerBillion),
            "bp" => Ok(Self::BasisPoint),
            "‰" | "permille" => Ok(Self::Permille),
            "%" => Ok(Self::Percent),
            "one" => Ok(Self::One),
            "bit" => Ok(Self::Bit),
            "B" => Ok(Self::Byte),
            "kB" => Ok(Self::Kilobyte),
            "MB" => Ok(Self::Megabyte),
            "GB" => Ok(Self::Gigabyte),
            "KiB" => Ok(Self::Kibibyte),
            "MiB" => Ok(Self::Mebibyte),
            "GiB" => Ok(Self::Gibibyte),
            "TB" => Ok(Self::Terabyte),
            "TiB" => Ok(Self::Tebibyte),
            "µg" | "ug" => Ok(Self::Microgram),
            "mg" => Ok(Self::Milligram),
            "g" => Ok(Self::Gram),
            "kg" => Ok(Self::Kilogram),
            "t" => Ok(Self::Tonne),
            "oz_av" => Ok(Self::Ounce),
            "lb_av" => Ok(Self::Pound),
            "st" => Ok(Self::Stone),
            "mm²" | "mm2" => Ok(Self::SquareMillimeter),
            "cm²" | "cm2" => Ok(Self::SquareCentimeter),
            "m²" | "m2" => Ok(Self::SquareMeter),
            "ha" => Ok(Self::Hectare),
            "km²" | "km2" => Ok(Self::SquareKilometer),
            "acre_int" => Ok(Self::Acre),
            "µL" | "uL" => Ok(Self::Microliter),
            "mL" => Ok(Self::Milliliter),
            "L" => Ok(Self::Liter),
            "m³" | "m3" => Ok(Self::CubicMeter),
            "mm/s" => Ok(Self::MillimeterPerSecond),
            "m/s" => Ok(Self::MeterPerSecond),
            "km/h" => Ok(Self::KilometerPerHour),
            "mph" => Ok(Self::MilePerHour),
            "kn" => Ok(Self::Knot),
            "m/s²" | "m/s2" => Ok(Self::MeterPerSecondSquared),
            "g₀" | "g0" => Ok(Self::StandardGravity),
            "Gal" => Ok(Self::Gal),
            "mN" => Ok(Self::Millinewton),
            "N" => Ok(Self::Newton),
            "kN" => Ok(Self::Kilonewton),
            "mJ" => Ok(Self::Millijoule),
            "J" => Ok(Self::Joule),
            "kJ" => Ok(Self::Kilojoule),
            "MJ" => Ok(Self::Megajoule),
            "Wh" => Ok(Self::WattHour),
            "kWh" => Ok(Self::KilowattHour),
            "cal_th" => Ok(Self::Calorie),
            "kcal_th" => Ok(Self::Kilocalorie),
            "mW" => Ok(Self::Milliwatt),
            "W" => Ok(Self::Watt),
            "kW" => Ok(Self::Kilowatt),
            "MW" => Ok(Self::Megawatt),
            "Pa" => Ok(Self::Pascal),
            "kPa" => Ok(Self::Kilopascal),
            "MPa" => Ok(Self::Megapascal),
            "bar" => Ok(Self::Bar),
            "mbar" => Ok(Self::Millibar),
            "atm" => Ok(Self::Atmosphere),
            "Torr" => Ok(Self::Torr),
            "px" => Ok(Self::Pixel),
            "" => Err(QuantityLiteralRefusal::MissingUnit),
            _ => Err(QuantityLiteralRefusal::UnknownUnit),
        }
    }

    pub const fn dimension(self) -> QuantityDimension {
        match self {
            Self::Picosecond
            | Self::Nanosecond
            | Self::Shake
            | Self::Microsecond
            | Self::Millisecond
            | Self::Second
            | Self::Minute
            | Self::Moment
            | Self::Hour
            | Self::Day
            | Self::Week
            | Self::Fortnight
            | Self::JulianYear => QuantityDimension::Time,
            Self::Millihertz
            | Self::Hertz
            | Self::Kilohertz
            | Self::Megahertz
            | Self::Gigahertz => QuantityDimension::Frequency,
            Self::Nanovolt | Self::Microvolt | Self::Millivolt | Self::Volt | Self::Kilovolt => {
                QuantityDimension::Voltage
            }
            Self::Nanoampere
            | Self::Microampere
            | Self::Milliampere
            | Self::Ampere
            | Self::Kiloampere => QuantityDimension::Current,
            Self::Millikelvin
            | Self::Kelvin
            | Self::MilliCelsius
            | Self::Celsius
            | Self::MilliFahrenheit
            | Self::Fahrenheit => QuantityDimension::Temperature,
            Self::MicroampereHour | Self::MilliampereHour | Self::AmpereHour => {
                QuantityDimension::Charge
            }
            Self::Nanometer
            | Self::Micrometer
            | Self::Millimeter
            | Self::Centimeter
            | Self::Meter
            | Self::Kilometer
            | Self::Angstrom
            | Self::Inch
            | Self::Hand
            | Self::Foot
            | Self::Yard
            | Self::Rod
            | Self::Chain
            | Self::Furlong
            | Self::Mile
            | Self::League
            | Self::NauticalMile
            | Self::AstronomicalUnit => QuantityDimension::Length,
            Self::Microdegree
            | Self::Millidegree
            | Self::Degree
            | Self::Microradian
            | Self::Milliradian
            | Self::Radian
            | Self::Arcsecond
            | Self::Arcminute
            | Self::Gradian
            | Self::Turn => QuantityDimension::Angle,
            Self::PartPerBillion
            | Self::Millionth
            | Self::BasisPoint
            | Self::Permille
            | Self::Percent
            | Self::One => QuantityDimension::Ratio,
            Self::Bit
            | Self::Byte
            | Self::Kilobyte
            | Self::Megabyte
            | Self::Gigabyte
            | Self::Kibibyte
            | Self::Mebibyte
            | Self::Gibibyte
            | Self::Terabyte
            | Self::Tebibyte => QuantityDimension::DataSize,
            Self::Microgram
            | Self::Milligram
            | Self::Gram
            | Self::Kilogram
            | Self::Tonne
            | Self::Ounce
            | Self::Pound
            | Self::Stone => QuantityDimension::Mass,
            Self::SquareMillimeter
            | Self::SquareCentimeter
            | Self::SquareMeter
            | Self::Hectare
            | Self::SquareKilometer
            | Self::Acre => QuantityDimension::Area,
            Self::Microliter | Self::Milliliter | Self::Liter | Self::CubicMeter => {
                QuantityDimension::Volume
            }
            Self::MillimeterPerSecond
            | Self::MeterPerSecond
            | Self::KilometerPerHour
            | Self::MilePerHour
            | Self::Knot => QuantityDimension::Speed,
            Self::MeterPerSecondSquared | Self::StandardGravity | Self::Gal => {
                QuantityDimension::Acceleration
            }
            Self::Millinewton | Self::Newton | Self::Kilonewton => QuantityDimension::Force,
            Self::Millijoule
            | Self::Joule
            | Self::Kilojoule
            | Self::Megajoule
            | Self::WattHour
            | Self::KilowattHour
            | Self::Calorie
            | Self::Kilocalorie => QuantityDimension::Energy,
            Self::Milliwatt | Self::Watt | Self::Kilowatt | Self::Megawatt => {
                QuantityDimension::Power
            }
            Self::Pascal
            | Self::Kilopascal
            | Self::Megapascal
            | Self::Bar
            | Self::Millibar
            | Self::Atmosphere
            | Self::Torr => QuantityDimension::Pressure,
            Self::Pixel => QuantityDimension::PixelCount,
        }
    }

    const fn canonical_factor(self) -> i64 {
        match self {
            Self::Picosecond => 1,
            Self::Nanosecond => 1,
            Self::Shake => 10,
            Self::Microsecond => 1_000,
            Self::Millisecond => 1_000_000,
            Self::Second => 1_000_000_000,
            Self::Minute => 60_000_000_000,
            Self::Moment => 90_000_000_000,
            Self::Hour => 3_600_000_000_000,
            Self::Day => 86_400_000_000_000,
            Self::Week => 604_800_000_000_000,
            Self::Fortnight => 1_209_600_000_000_000,
            Self::JulianYear => 31_557_600_000_000_000,
            Self::Millihertz => 1,
            Self::Hertz => 1_000,
            Self::Kilohertz => 1_000_000,
            Self::Megahertz => 1_000_000_000,
            Self::Gigahertz => 1_000_000_000_000,
            Self::Nanovolt => 1,
            Self::Microvolt => 1,
            Self::Millivolt => 1_000,
            Self::Volt => 1_000_000,
            Self::Kilovolt => 1_000_000_000,
            Self::Nanoampere => 1,
            Self::Microampere => 1,
            Self::Milliampere => 1_000,
            Self::Ampere => 1_000_000,
            Self::Kiloampere => 1_000_000_000,
            Self::Millikelvin | Self::MilliCelsius | Self::MilliFahrenheit => 1,
            Self::Kelvin | Self::Celsius | Self::Fahrenheit => 1_000,
            Self::MicroampereHour => 1,
            Self::MilliampereHour => 1_000,
            Self::AmpereHour => 1_000_000,
            Self::Micrometer => 1,
            Self::Nanometer => 1,
            Self::Millimeter => 1_000,
            Self::Centimeter => 10_000,
            Self::Meter => 1_000_000,
            Self::Kilometer => 1_000_000_000,
            Self::Angstrom => 1,
            Self::Inch => 25_400,
            Self::Hand => 101_600,
            Self::Foot => 304_800,
            Self::Yard => 914_400,
            Self::Rod => 5_029_200,
            Self::Chain => 20_116_800,
            Self::Furlong => 201_168_000,
            Self::Mile => 1_609_344_000,
            Self::League => 4_828_032_000,
            Self::NauticalMile => 1_852_000_000,
            Self::AstronomicalUnit => 149_597_870_700_000_000,
            Self::Microdegree => 1,
            Self::Millidegree => 1_000,
            Self::Degree => 1_000_000,
            Self::Arcsecond => 1,
            Self::Arcminute => 1,
            Self::Gradian => 900_000,
            Self::Turn => 360_000_000,
            Self::Microradian => 1,
            Self::Milliradian => 1_000,
            Self::Radian => 1_000_000,
            Self::Millionth => 1,
            Self::PartPerBillion => 1,
            Self::BasisPoint => 100,
            Self::Permille => 1_000,
            Self::Percent => 10_000,
            Self::One => 1_000_000,
            Self::Bit => 1,
            Self::Byte => 1,
            Self::Kilobyte => 1_000,
            Self::Megabyte => 1_000_000,
            Self::Gigabyte => 1_000_000_000,
            Self::Kibibyte => 1_024,
            Self::Mebibyte => 1_048_576,
            Self::Gibibyte => 1_073_741_824,
            Self::Terabyte => 1_000_000_000_000,
            Self::Tebibyte => 1_099_511_627_776,
            Self::Microgram => 1,
            Self::Milligram => 1_000,
            Self::Gram => 1_000_000,
            Self::Kilogram => 1_000_000_000,
            Self::Tonne => 1_000_000_000_000,
            Self::Ounce => 1,
            Self::Pound => 453_592_370,
            Self::Stone => 6_350_293_180,
            Self::SquareMillimeter => 1,
            Self::SquareCentimeter => 100,
            Self::SquareMeter => 1_000_000,
            Self::Hectare => 10_000_000_000,
            Self::SquareKilometer => 1_000_000_000_000,
            Self::Acre => 1,
            Self::Microliter => 1,
            Self::Milliliter => 1_000,
            Self::Liter => 1_000_000,
            Self::CubicMeter => 1_000_000_000,
            Self::MillimeterPerSecond => 1_000,
            Self::MeterPerSecond => 1_000_000,
            Self::KilometerPerHour | Self::MilePerHour | Self::Knot => 1,
            Self::MeterPerSecondSquared => 1_000_000,
            Self::StandardGravity => 9_806_650,
            Self::Gal => 10_000,
            Self::Millinewton => 1,
            Self::Newton => 1_000,
            Self::Kilonewton => 1_000_000,
            Self::Millijoule => 1,
            Self::Joule => 1_000,
            Self::Kilojoule => 1_000_000,
            Self::Megajoule => 1_000_000_000,
            Self::WattHour => 3_600_000,
            Self::KilowattHour => 3_600_000_000,
            Self::Calorie => 4_184,
            Self::Kilocalorie => 4_184_000,
            Self::Milliwatt => 1,
            Self::Watt => 1_000,
            Self::Kilowatt => 1_000_000,
            Self::Megawatt => 1_000_000_000,
            Self::Pascal => 1,
            Self::Kilopascal => 1_000,
            Self::Megapascal => 1_000_000,
            Self::Bar => 100_000,
            Self::Millibar => 100,
            Self::Atmosphere => 101_325,
            Self::Torr => 1,
            Self::Pixel => 1,
        }
    }

    /// Exact affine transform into the dimension's canonical reference scale:
    /// `(value * scale_numerator + offset_numerator) / denominator`.
    /// Keeping this metadata on every reviewed unit lets one conversion engine
    /// serve linear and offset units alike.
    const fn canonical_transform(self) -> (i128, i128, i128) {
        match self {
            Self::Picosecond => (1, 0, 1_000),
            Self::Nanometer => (1, 0, 1_000),
            Self::Angstrom => (1, 0, 10_000),
            Self::Arcsecond => (2_500, 0, 9),
            Self::Arcminute => (50_000, 0, 3),
            Self::PartPerBillion => (1, 0, 1_000),
            Self::Bit => (1, 0, 8),
            Self::Ounce => (226_796_185, 0, 8),
            Self::Acre => (20_234_282_112, 0, 5),
            Self::KilometerPerHour => (2_500_000, 0, 9),
            Self::MilePerHour => (20_116_800, 0, 45),
            Self::Knot => (4_630_000, 0, 9),
            Self::Torr => (20_265, 0, 152),
            Self::MilliCelsius => (1, 273_150, 1),
            Self::Celsius => (1_000, 273_150, 1),
            Self::MilliFahrenheit => (5, 2_298_350, 9),
            Self::Fahrenheit => (5_000, 2_298_350, 9),
            _ => (self.canonical_factor() as i128, 0, 1),
        }
    }

    const fn tag(self) -> u8 {
        match self {
            Self::Nanosecond => 0,
            Self::Microsecond => 1,
            Self::Millisecond => 2,
            Self::Second => 3,
            Self::Millihertz => 4,
            Self::Hertz => 5,
            Self::Microvolt => 6,
            Self::Millivolt => 7,
            Self::Volt => 8,
            Self::Micrometer => 9,
            Self::Millimeter => 10,
            Self::Centimeter => 11,
            Self::Meter => 12,
            Self::Microdegree => 13,
            Self::Millidegree => 14,
            Self::Degree => 15,
            Self::Millionth => 16,
            Self::Permille => 17,
            Self::Percent => 18,
            Self::One => 19,
            Self::Byte => 20,
            Self::Kibibyte => 21,
            Self::Mebibyte => 22,
            Self::Microampere => 23,
            Self::Milliampere => 24,
            Self::Ampere => 25,
            Self::Millikelvin => 26,
            Self::Kelvin => 27,
            Self::MilliCelsius => 28,
            Self::Celsius => 29,
            Self::MicroampereHour => 30,
            Self::MilliampereHour => 31,
            Self::AmpereHour => 32,
            Self::Microradian => 33,
            Self::Milliradian => 34,
            Self::Radian => 35,
            Self::Pixel => 36,
            Self::MilliFahrenheit => 37,
            Self::Fahrenheit => 38,
            Self::Minute => 39,
            Self::Moment => 40,
            Self::Hour => 41,
            Self::Day => 42,
            Self::Week => 43,
            Self::Fortnight => 44,
            Self::Picosecond => 45,
            Self::Shake => 46,
            Self::JulianYear => 47,
            Self::Kilohertz => 48,
            Self::Megahertz => 49,
            Self::Gigahertz => 50,
            Self::Nanovolt => 51,
            Self::Kilovolt => 52,
            Self::Nanoampere => 53,
            Self::Kiloampere => 54,
            Self::Nanometer => 55,
            Self::Kilometer => 56,
            Self::Angstrom => 57,
            Self::Inch => 58,
            Self::Hand => 59,
            Self::Foot => 60,
            Self::Yard => 61,
            Self::Rod => 62,
            Self::Chain => 63,
            Self::Furlong => 64,
            Self::Mile => 65,
            Self::League => 66,
            Self::NauticalMile => 67,
            Self::AstronomicalUnit => 68,
            Self::Arcsecond => 69,
            Self::Arcminute => 70,
            Self::Gradian => 71,
            Self::Turn => 72,
            Self::PartPerBillion => 73,
            Self::BasisPoint => 74,
            Self::Bit => 75,
            Self::Kilobyte => 76,
            Self::Megabyte => 77,
            Self::Gigabyte => 78,
            Self::Gibibyte => 79,
            Self::Terabyte => 80,
            Self::Tebibyte => 81,
            Self::Microgram => 82,
            Self::Milligram => 83,
            Self::Gram => 84,
            Self::Kilogram => 85,
            Self::Tonne => 86,
            Self::Ounce => 87,
            Self::Pound => 88,
            Self::Stone => 89,
            Self::SquareMillimeter => 90,
            Self::SquareCentimeter => 91,
            Self::SquareMeter => 92,
            Self::Hectare => 93,
            Self::SquareKilometer => 94,
            Self::Acre => 95,
            Self::Microliter => 96,
            Self::Milliliter => 97,
            Self::Liter => 98,
            Self::CubicMeter => 99,
            Self::MillimeterPerSecond => 100,
            Self::MeterPerSecond => 101,
            Self::KilometerPerHour => 102,
            Self::MilePerHour => 103,
            Self::Knot => 104,
            Self::MeterPerSecondSquared => 105,
            Self::StandardGravity => 106,
            Self::Gal => 107,
            Self::Millinewton => 108,
            Self::Newton => 109,
            Self::Kilonewton => 110,
            Self::Millijoule => 111,
            Self::Joule => 112,
            Self::Kilojoule => 113,
            Self::Megajoule => 114,
            Self::WattHour => 115,
            Self::KilowattHour => 116,
            Self::Calorie => 117,
            Self::Kilocalorie => 118,
            Self::Milliwatt => 119,
            Self::Watt => 120,
            Self::Kilowatt => 121,
            Self::Megawatt => 122,
            Self::Pascal => 123,
            Self::Kilopascal => 124,
            Self::Megapascal => 125,
            Self::Bar => 126,
            Self::Millibar => 127,
            Self::Atmosphere => 128,
            Self::Torr => 129,
        }
    }

    fn from_tag(tag: u8) -> Result<Self, QuantityDecodeRefusal> {
        match tag {
            0 => Ok(Self::Nanosecond),
            1 => Ok(Self::Microsecond),
            2 => Ok(Self::Millisecond),
            3 => Ok(Self::Second),
            4 => Ok(Self::Millihertz),
            5 => Ok(Self::Hertz),
            6 => Ok(Self::Microvolt),
            7 => Ok(Self::Millivolt),
            8 => Ok(Self::Volt),
            9 => Ok(Self::Micrometer),
            10 => Ok(Self::Millimeter),
            11 => Ok(Self::Centimeter),
            12 => Ok(Self::Meter),
            13 => Ok(Self::Microdegree),
            14 => Ok(Self::Millidegree),
            15 => Ok(Self::Degree),
            16 => Ok(Self::Millionth),
            17 => Ok(Self::Permille),
            18 => Ok(Self::Percent),
            19 => Ok(Self::One),
            20 => Ok(Self::Byte),
            21 => Ok(Self::Kibibyte),
            22 => Ok(Self::Mebibyte),
            23 => Ok(Self::Microampere),
            24 => Ok(Self::Milliampere),
            25 => Ok(Self::Ampere),
            26 => Ok(Self::Millikelvin),
            27 => Ok(Self::Kelvin),
            28 => Ok(Self::MilliCelsius),
            29 => Ok(Self::Celsius),
            30 => Ok(Self::MicroampereHour),
            31 => Ok(Self::MilliampereHour),
            32 => Ok(Self::AmpereHour),
            33 => Ok(Self::Microradian),
            34 => Ok(Self::Milliradian),
            35 => Ok(Self::Radian),
            36 => Ok(Self::Pixel),
            37 => Ok(Self::MilliFahrenheit),
            38 => Ok(Self::Fahrenheit),
            39 => Ok(Self::Minute),
            40 => Ok(Self::Moment),
            41 => Ok(Self::Hour),
            42 => Ok(Self::Day),
            43 => Ok(Self::Week),
            44 => Ok(Self::Fortnight),
            45 => Ok(Self::Picosecond),
            46 => Ok(Self::Shake),
            47 => Ok(Self::JulianYear),
            48 => Ok(Self::Kilohertz),
            49 => Ok(Self::Megahertz),
            50 => Ok(Self::Gigahertz),
            51 => Ok(Self::Nanovolt),
            52 => Ok(Self::Kilovolt),
            53 => Ok(Self::Nanoampere),
            54 => Ok(Self::Kiloampere),
            55 => Ok(Self::Nanometer),
            56 => Ok(Self::Kilometer),
            57 => Ok(Self::Angstrom),
            58 => Ok(Self::Inch),
            59 => Ok(Self::Hand),
            60 => Ok(Self::Foot),
            61 => Ok(Self::Yard),
            62 => Ok(Self::Rod),
            63 => Ok(Self::Chain),
            64 => Ok(Self::Furlong),
            65 => Ok(Self::Mile),
            66 => Ok(Self::League),
            67 => Ok(Self::NauticalMile),
            68 => Ok(Self::AstronomicalUnit),
            69 => Ok(Self::Arcsecond),
            70 => Ok(Self::Arcminute),
            71 => Ok(Self::Gradian),
            72 => Ok(Self::Turn),
            73 => Ok(Self::PartPerBillion),
            74 => Ok(Self::BasisPoint),
            75 => Ok(Self::Bit),
            76 => Ok(Self::Kilobyte),
            77 => Ok(Self::Megabyte),
            78 => Ok(Self::Gigabyte),
            79 => Ok(Self::Gibibyte),
            80 => Ok(Self::Terabyte),
            81 => Ok(Self::Tebibyte),
            82 => Ok(Self::Microgram),
            83 => Ok(Self::Milligram),
            84 => Ok(Self::Gram),
            85 => Ok(Self::Kilogram),
            86 => Ok(Self::Tonne),
            87 => Ok(Self::Ounce),
            88 => Ok(Self::Pound),
            89 => Ok(Self::Stone),
            90 => Ok(Self::SquareMillimeter),
            91 => Ok(Self::SquareCentimeter),
            92 => Ok(Self::SquareMeter),
            93 => Ok(Self::Hectare),
            94 => Ok(Self::SquareKilometer),
            95 => Ok(Self::Acre),
            96 => Ok(Self::Microliter),
            97 => Ok(Self::Milliliter),
            98 => Ok(Self::Liter),
            99 => Ok(Self::CubicMeter),
            100 => Ok(Self::MillimeterPerSecond),
            101 => Ok(Self::MeterPerSecond),
            102 => Ok(Self::KilometerPerHour),
            103 => Ok(Self::MilePerHour),
            104 => Ok(Self::Knot),
            105 => Ok(Self::MeterPerSecondSquared),
            106 => Ok(Self::StandardGravity),
            107 => Ok(Self::Gal),
            108 => Ok(Self::Millinewton),
            109 => Ok(Self::Newton),
            110 => Ok(Self::Kilonewton),
            111 => Ok(Self::Millijoule),
            112 => Ok(Self::Joule),
            113 => Ok(Self::Kilojoule),
            114 => Ok(Self::Megajoule),
            115 => Ok(Self::WattHour),
            116 => Ok(Self::KilowattHour),
            117 => Ok(Self::Calorie),
            118 => Ok(Self::Kilocalorie),
            119 => Ok(Self::Milliwatt),
            120 => Ok(Self::Watt),
            121 => Ok(Self::Kilowatt),
            122 => Ok(Self::Megawatt),
            123 => Ok(Self::Pascal),
            124 => Ok(Self::Kilopascal),
            125 => Ok(Self::Megapascal),
            126 => Ok(Self::Bar),
            127 => Ok(Self::Millibar),
            128 => Ok(Self::Atmosphere),
            129 => Ok(Self::Torr),
            other => Err(QuantityDecodeRefusal::UnknownUnitTag(other)),
        }
    }
}

impl Quantity {
    pub const fn new(value: i64, unit: QuantityUnit) -> Self {
        Self { value, unit }
    }

    pub const fn value(self) -> i64 {
        self.value
    }

    pub const fn unit(self) -> QuantityUnit {
        self.unit
    }

    pub const fn dimension(self) -> QuantityDimension {
        self.unit.dimension()
    }

    /// Parses a reviewed scientific quantity literal exactly. Decimal source
    /// is admitted only when a reviewed unit in the same dimension can retain
    /// the value without rounding (for example, `3.2m` becomes `320cm`).
    pub fn parse_plot_literal(literal: &str) -> Result<Self, QuantityLiteralRefusal> {
        literal::parse_plot_literal(literal)
    }

    pub fn convert(self, target: QuantityUnit) -> Result<Self, QuantityConversionRefusal> {
        if self.unit.dimension() != target.dimension() {
            return Err(QuantityConversionRefusal::IncompatibleDimensions);
        }
        if self.unit == target {
            return Ok(self);
        }
        if self.dimension() == QuantityDimension::Angle && is_radian(self.unit) != is_radian(target)
        {
            return Err(QuantityConversionRefusal::Inexact);
        }
        convert_exact_rational(i128::from(self.value), 1, self.unit, target)
    }

    /// Compares compatible quantities in their shared canonical reference
    /// scale without requiring either operand to be representable in the
    /// other's unit.
    pub fn compare(self, other: Self) -> Result<Ordering, QuantityConversionRefusal> {
        if self.dimension() != other.dimension() {
            return Err(QuantityConversionRefusal::IncompatibleDimensions);
        }
        if self.dimension() == QuantityDimension::Angle
            && is_radian(self.unit) != is_radian(other.unit)
        {
            return Err(QuantityConversionRefusal::Inexact);
        }
        compare_legacy(self, other)
    }

    pub const fn encode(self) -> [u8; QUANTITY_ENCODED_LEN] {
        let value = self.value.to_le_bytes();
        [
            self.unit.tag(),
            value[0],
            value[1],
            value[2],
            value[3],
            value[4],
            value[5],
            value[6],
            value[7],
        ]
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, QuantityDecodeRefusal> {
        if encoded.len() != QUANTITY_ENCODED_LEN {
            return Err(QuantityDecodeRefusal::WrongLength {
                expected: QUANTITY_ENCODED_LEN,
                actual: encoded.len(),
            });
        }
        let unit = QuantityUnit::from_tag(encoded[0])?;
        let value = i64::from_le_bytes(
            encoded[1..]
                .try_into()
                .expect("quantity length checked before value decode"),
        );
        Ok(Self::new(value, unit))
    }

    pub fn semantic_digest(self) -> [u8; 32] {
        semantic_digest(QUANTITY_INFO_ID, &self.encode())
    }
}
