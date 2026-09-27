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

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum QuantityUnit {
    Nanosecond,
    Microsecond,
    Millisecond,
    Second,
    Millihertz,
    Hertz,
    Microvolt,
    Millivolt,
    Volt,
    Microampere,
    Milliampere,
    Ampere,
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
    Millimeter,
    Centimeter,
    Meter,
    Microdegree,
    Millidegree,
    Degree,
    Microradian,
    Milliradian,
    Radian,
    Millionth,
    Permille,
    Percent,
    One,
    Byte,
    Kibibyte,
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
    pub const fn semantic_id(self) -> &'static str {
        match self {
            Self::Nanosecond => "time/nanosecond",
            Self::Microsecond => "time/microsecond",
            Self::Millisecond => "time/millisecond",
            Self::Second => "time/second",
            Self::Millihertz => "frequency/millihertz",
            Self::Hertz => "frequency/hertz",
            Self::Microvolt => "voltage/microvolt",
            Self::Millivolt => "voltage/millivolt",
            Self::Volt => "voltage/volt",
            Self::Microampere => "current/microampere",
            Self::Milliampere => "current/milliampere",
            Self::Ampere => "current/ampere",
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
            Self::Millimeter => "length/millimeter",
            Self::Centimeter => "length/centimeter",
            Self::Meter => "length/meter",
            Self::Microdegree => "angle/microdegree",
            Self::Millidegree => "angle/millidegree",
            Self::Degree => "angle/degree",
            Self::Microradian => "angle/microradian",
            Self::Milliradian => "angle/milliradian",
            Self::Radian => "angle/radian",
            Self::Millionth => "ratio/millionth",
            Self::Permille => "ratio/permille",
            Self::Percent => "ratio/percent",
            Self::One => "ratio/one",
            Self::Byte => "data-size/byte",
            Self::Kibibyte => "data-size/kibibyte",
            Self::Mebibyte => "data-size/mebibyte",
            Self::Pixel => "pixel/count",
        }
    }

    pub const fn form_suffix(self) -> &'static str {
        match self {
            Self::Nanosecond => "ns",
            Self::Microsecond => "µs",
            Self::Millisecond => "ms",
            Self::Second => "s",
            Self::Millihertz => "mHz",
            Self::Hertz => "Hz",
            Self::Microvolt => "µV",
            Self::Millivolt => "mV",
            Self::Volt => "V",
            Self::Microampere => "µA",
            Self::Milliampere => "mA",
            Self::Ampere => "A",
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
            Self::Millimeter => "mm",
            Self::Centimeter => "cm",
            Self::Meter => "m",
            Self::Microdegree => "µ°",
            Self::Millidegree => "m°",
            Self::Degree => "°",
            Self::Microradian => "µrad",
            Self::Milliradian => "mrad",
            Self::Radian => "rad",
            Self::Millionth => "ppm",
            Self::Permille => "‰",
            Self::Percent => "%",
            Self::One => "one",
            Self::Byte => "B",
            Self::Kibibyte => "KiB",
            Self::Mebibyte => "MiB",
            Self::Pixel => "px",
        }
    }

    pub fn from_form_suffix(suffix: &str) -> Result<Self, QuantityLiteralRefusal> {
        match suffix {
            "ns" => Ok(Self::Nanosecond),
            "µs" | "us" => Ok(Self::Microsecond),
            "ms" => Ok(Self::Millisecond),
            "s" => Ok(Self::Second),
            "mHz" => Ok(Self::Millihertz),
            "Hz" => Ok(Self::Hertz),
            "µV" | "uV" => Ok(Self::Microvolt),
            "mV" => Ok(Self::Millivolt),
            "V" => Ok(Self::Volt),
            "µA" | "uA" => Ok(Self::Microampere),
            "mA" => Ok(Self::Milliampere),
            "A" => Ok(Self::Ampere),
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
            "mm" => Ok(Self::Millimeter),
            "cm" => Ok(Self::Centimeter),
            "m" => Ok(Self::Meter),
            "µ°" | "udeg" => Ok(Self::Microdegree),
            "m°" | "mdeg" => Ok(Self::Millidegree),
            "°" | "deg" => Ok(Self::Degree),
            "µrad" | "urad" => Ok(Self::Microradian),
            "mrad" => Ok(Self::Milliradian),
            "rad" => Ok(Self::Radian),
            "ppm" => Ok(Self::Millionth),
            "‰" | "permille" => Ok(Self::Permille),
            "%" => Ok(Self::Percent),
            "one" => Ok(Self::One),
            "B" => Ok(Self::Byte),
            "KiB" => Ok(Self::Kibibyte),
            "MiB" => Ok(Self::Mebibyte),
            "px" => Ok(Self::Pixel),
            "" => Err(QuantityLiteralRefusal::MissingUnit),
            _ => Err(QuantityLiteralRefusal::UnknownUnit),
        }
    }

    pub const fn dimension(self) -> QuantityDimension {
        match self {
            Self::Nanosecond | Self::Microsecond | Self::Millisecond | Self::Second => {
                QuantityDimension::Time
            }
            Self::Millihertz | Self::Hertz => QuantityDimension::Frequency,
            Self::Microvolt | Self::Millivolt | Self::Volt => QuantityDimension::Voltage,
            Self::Microampere | Self::Milliampere | Self::Ampere => QuantityDimension::Current,
            Self::Millikelvin
            | Self::Kelvin
            | Self::MilliCelsius
            | Self::Celsius
            | Self::MilliFahrenheit
            | Self::Fahrenheit => QuantityDimension::Temperature,
            Self::MicroampereHour | Self::MilliampereHour | Self::AmpereHour => {
                QuantityDimension::Charge
            }
            Self::Micrometer | Self::Millimeter | Self::Centimeter | Self::Meter => {
                QuantityDimension::Length
            }
            Self::Microdegree
            | Self::Millidegree
            | Self::Degree
            | Self::Microradian
            | Self::Milliradian
            | Self::Radian => QuantityDimension::Angle,
            Self::Millionth | Self::Permille | Self::Percent | Self::One => {
                QuantityDimension::Ratio
            }
            Self::Byte | Self::Kibibyte | Self::Mebibyte => QuantityDimension::DataSize,
            Self::Pixel => QuantityDimension::PixelCount,
        }
    }

    const fn canonical_factor(self) -> i64 {
        match self {
            Self::Nanosecond => 1,
            Self::Microsecond => 1_000,
            Self::Millisecond => 1_000_000,
            Self::Second => 1_000_000_000,
            Self::Millihertz => 1,
            Self::Hertz => 1_000,
            Self::Microvolt => 1,
            Self::Millivolt => 1_000,
            Self::Volt => 1_000_000,
            Self::Microampere => 1,
            Self::Milliampere => 1_000,
            Self::Ampere => 1_000_000,
            Self::Millikelvin | Self::MilliCelsius | Self::MilliFahrenheit => 1,
            Self::Kelvin | Self::Celsius | Self::Fahrenheit => 1_000,
            Self::MicroampereHour => 1,
            Self::MilliampereHour => 1_000,
            Self::AmpereHour => 1_000_000,
            Self::Micrometer => 1,
            Self::Millimeter => 1_000,
            Self::Centimeter => 10_000,
            Self::Meter => 1_000_000,
            Self::Microdegree => 1,
            Self::Millidegree => 1_000,
            Self::Degree => 1_000_000,
            Self::Microradian => 1,
            Self::Milliradian => 1_000,
            Self::Radian => 1_000_000,
            Self::Millionth => 1,
            Self::Permille => 1_000,
            Self::Percent => 10_000,
            Self::One => 1_000_000,
            Self::Byte => 1,
            Self::Kibibyte => 1_024,
            Self::Mebibyte => 1_048_576,
            Self::Pixel => 1,
        }
    }

    /// Exact affine transform into the dimension's canonical reference scale:
    /// `(value * scale_numerator + offset_numerator) / denominator`.
    /// Keeping this metadata on every reviewed unit lets one conversion engine
    /// serve linear and offset units alike.
    const fn canonical_transform(self) -> (i128, i128, i128) {
        match self {
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

const ALL_QUANTITY_UNITS: [QuantityUnit; 39] = [
    QuantityUnit::Nanosecond,
    QuantityUnit::Microsecond,
    QuantityUnit::Millisecond,
    QuantityUnit::Second,
    QuantityUnit::Millihertz,
    QuantityUnit::Hertz,
    QuantityUnit::Microvolt,
    QuantityUnit::Millivolt,
    QuantityUnit::Volt,
    QuantityUnit::Microampere,
    QuantityUnit::Milliampere,
    QuantityUnit::Ampere,
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
    QuantityUnit::Millimeter,
    QuantityUnit::Centimeter,
    QuantityUnit::Meter,
    QuantityUnit::Microdegree,
    QuantityUnit::Millidegree,
    QuantityUnit::Degree,
    QuantityUnit::Microradian,
    QuantityUnit::Milliradian,
    QuantityUnit::Radian,
    QuantityUnit::Millionth,
    QuantityUnit::Permille,
    QuantityUnit::Percent,
    QuantityUnit::One,
    QuantityUnit::Byte,
    QuantityUnit::Kibibyte,
    QuantityUnit::Mebibyte,
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
