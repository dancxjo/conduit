//! Typed finite quantities with exact-only conversions.

use serde::{Deserialize, Serialize};

use crate::semantic_digest;

pub const QUANTITY_INFO_ID: &str = "value/quantity@1";
pub const QUANTITY_ENCODED_LEN: usize = 9;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum QuantityDimension {
    Time,
    Frequency,
    Voltage,
    Length,
    Angle,
    Ratio,
    DataSize,
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
    Micrometer,
    Millimeter,
    Centimeter,
    Meter,
    Microdegree,
    Millidegree,
    Degree,
    Millionth,
    Permille,
    Percent,
    One,
    Byte,
    Kibibyte,
    Mebibyte,
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

impl QuantityUnit {
    pub const fn dimension(self) -> QuantityDimension {
        match self {
            Self::Nanosecond | Self::Microsecond | Self::Millisecond | Self::Second => {
                QuantityDimension::Time
            }
            Self::Millihertz | Self::Hertz => QuantityDimension::Frequency,
            Self::Microvolt | Self::Millivolt | Self::Volt => QuantityDimension::Voltage,
            Self::Micrometer | Self::Millimeter | Self::Centimeter | Self::Meter => {
                QuantityDimension::Length
            }
            Self::Microdegree | Self::Millidegree | Self::Degree => QuantityDimension::Angle,
            Self::Millionth | Self::Permille | Self::Percent | Self::One => {
                QuantityDimension::Ratio
            }
            Self::Byte | Self::Kibibyte | Self::Mebibyte => QuantityDimension::DataSize,
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
            Self::Micrometer => 1,
            Self::Millimeter => 1_000,
            Self::Centimeter => 10_000,
            Self::Meter => 1_000_000,
            Self::Microdegree => 1,
            Self::Millidegree => 1_000,
            Self::Degree => 1_000_000,
            Self::Millionth => 1,
            Self::Permille => 1_000,
            Self::Percent => 10_000,
            Self::One => 1_000_000,
            Self::Byte => 1,
            Self::Kibibyte => 1_024,
            Self::Mebibyte => 1_048_576,
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

    pub fn convert(self, target: QuantityUnit) -> Result<Self, QuantityConversionRefusal> {
        if self.unit.dimension() != target.dimension() {
            return Err(QuantityConversionRefusal::IncompatibleDimensions);
        }
        if self.unit == target {
            return Ok(self);
        }
        let canonical = self
            .value
            .checked_mul(self.unit.canonical_factor())
            .ok_or(QuantityConversionRefusal::Overflow)?;
        let target_factor = target.canonical_factor();
        if canonical % target_factor != 0 {
            return Err(QuantityConversionRefusal::Inexact);
        }
        Ok(Self::new(canonical / target_factor, target))
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
