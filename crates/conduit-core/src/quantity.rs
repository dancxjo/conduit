//! Typed finite quantities with exact-only conversions.

use serde::{Deserialize, Serialize};

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
}
