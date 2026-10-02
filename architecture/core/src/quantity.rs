//! Typed finite quantities with exact-only conversions.

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
            // but do not yet own public dimension-specific Form aliases.
            Self::Current | Self::Charge | Self::DataSize => QUANTITY_INFO_ID,
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
    NonCanonicalUnit { canonical: &'static str },
    Inexact,
    Overflow,
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
            Self::Pixel => "pixel/count",
        }
    }

    pub const fn form_suffix(self) -> &'static str {
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
            Self::Pixel => "px",
        }
    }

    pub fn from_form_suffix(suffix: &str) -> Result<Self, QuantityLiteralRefusal> {
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
    pub fn parse_form_literal(literal: &str) -> Result<Self, QuantityLiteralRefusal> {
        let value_end = literal
            .char_indices()
            .find_map(|(index, character)| {
                (!(character.is_ascii_digit()
                    || character == '.'
                    || (index == 0 && character == '-')))
                    .then_some(index)
            })
            .unwrap_or(literal.len());
        let (value, suffix) = literal.split_at(value_end);
        if value.is_empty() || value == "-" {
            return Err(QuantityLiteralRefusal::MissingValue);
        }
        let unit = QuantityUnit::from_form_suffix(suffix)?;
        let Some((whole, fraction)) = value.split_once('.') else {
            let value = value
                .parse::<i64>()
                .map_err(|_| QuantityLiteralRefusal::InvalidValue)?;
            return Ok(Self::new(value, unit));
        };
        parse_exact_decimal(whole, fraction, unit)
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
        let (left_numerator, left_denominator) = canonical_fraction(self)?;
        let (right_numerator, right_denominator) = canonical_fraction(other)?;
        let left = left_numerator
            .checked_mul(right_denominator)
            .ok_or(QuantityConversionRefusal::Overflow)?;
        let right = right_numerator
            .checked_mul(left_denominator)
            .ok_or(QuantityConversionRefusal::Overflow)?;
        Ok(left.cmp(&right))
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

fn canonical_fraction(value: Quantity) -> Result<(i128, i128), QuantityConversionRefusal> {
    let (scale, offset, denominator) = value.unit.canonical_transform();
    let numerator = i128::from(value.value)
        .checked_mul(scale)
        .and_then(|value| value.checked_add(offset))
        .ok_or(QuantityConversionRefusal::Overflow)?;
    Ok((numerator, denominator))
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
                return Err(QuantityLiteralRefusal::Overflow)
            }
            Err(QuantityConversionRefusal::IncompatibleDimensions) => unreachable!(),
        }
    }
    Err(QuantityLiteralRefusal::Inexact)
}

const fn temperature_family(unit: QuantityUnit) -> u8 {
    match unit {
        QuantityUnit::Millikelvin | QuantityUnit::Kelvin => 1,
        QuantityUnit::MilliCelsius | QuantityUnit::Celsius => 2,
        QuantityUnit::MilliFahrenheit | QuantityUnit::Fahrenheit => 3,
        _ => 0,
    }
}

const ALL_QUANTITY_UNITS: [QuantityUnit; 82] = [
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
    QuantityUnit::Pixel,
];

const fn is_radian(unit: QuantityUnit) -> bool {
    matches!(
        unit,
        QuantityUnit::Microradian | QuantityUnit::Milliradian | QuantityUnit::Radian
    )
}

fn convert_exact_rational(
    source_numerator: i128,
    source_denominator: i128,
    source: QuantityUnit,
    target: QuantityUnit,
) -> Result<Quantity, QuantityConversionRefusal> {
    let (source_scale, source_offset, source_transform_denominator) = source.canonical_transform();
    let (target_scale, target_offset, target_denominator) = target.canonical_transform();
    let canonical_numerator = source_numerator
        .checked_mul(source_scale)
        .and_then(|value| {
            source_offset
                .checked_mul(source_denominator)?
                .checked_add(value)
        })
        .ok_or(QuantityConversionRefusal::Overflow)?;
    let canonical_denominator = source_denominator
        .checked_mul(source_transform_denominator)
        .ok_or(QuantityConversionRefusal::Overflow)?;
    let target_numerator = canonical_numerator
        .checked_mul(target_denominator)
        .and_then(|value| {
            target_offset
                .checked_mul(canonical_denominator)?
                .checked_neg()?
                .checked_add(value)
        })
        .ok_or(QuantityConversionRefusal::Overflow)?;
    let target_value_denominator = canonical_denominator
        .checked_mul(target_scale)
        .ok_or(QuantityConversionRefusal::Overflow)?;
    if target_numerator % target_value_denominator != 0 {
        return Err(QuantityConversionRefusal::Inexact);
    }
    let value = i64::try_from(target_numerator / target_value_denominator)
        .map_err(|_| QuantityConversionRefusal::Overflow)?;
    Ok(Quantity::new(value, target))
}
