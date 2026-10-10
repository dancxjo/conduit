//! Typed finite quantities with exact-only conversions.

mod conversion_law;
mod exact;
mod magnitude;
mod receipt;
mod target;
mod temperature_difference;
mod wide_conversion;

pub use exact::*;
pub use receipt::*;
pub use target::*;
pub use temperature_difference::*;

use crate::Unit;
use serde::{Deserialize, Serialize};
pub const DISTANCE_INFO_ID: &str = crate::BUILTIN_DISTANCE_INFO_ID;
pub const FREQUENCY_INFO_ID: &str = crate::BUILTIN_FREQUENCY_INFO_ID;
pub const DURATION_INFO_ID: &str = crate::BUILTIN_DURATION_INFO_ID;
pub const VOLTAGE_INFO_ID: &str = crate::BUILTIN_VOLTAGE_INFO_ID;
pub const TEMPERATURE_INFO_ID: &str = crate::BUILTIN_TEMPERATURE_INFO_ID;
pub const ANGLE_INFO_ID: &str = crate::BUILTIN_ANGLE_INFO_ID;
pub const RATIO_INFO_ID: &str = crate::BUILTIN_RATIO_INFO_ID;
pub const PIXEL_COUNT_INFO_ID: &str = crate::BUILTIN_PIXEL_COUNT_INFO_ID;
pub type QuantityDimension = crate::DimensionDefinition;
#[allow(non_upper_case_globals)]
impl crate::DimensionDefinition {
    pub const Time: Self = crate::BUILTIN_SECOND.family().dimension();
    pub const Frequency: Self = crate::BUILTIN_HERTZ.family().dimension();
    pub const Voltage: Self = crate::BUILTIN_VOLT.family().dimension();
    pub const Current: Self = crate::BUILTIN_AMPERE.family().dimension();
    pub const Temperature: Self = crate::BUILTIN_KELVIN.family().dimension();
    pub const Charge: Self = crate::BUILTIN_AMPERE_HOUR.family().dimension();
    pub const Length: Self = crate::BUILTIN_METER.family().dimension();
    pub const Angle: Self = crate::BUILTIN_ANGLE_DEFAULT_UNIT.family().dimension();
    pub const Ratio: Self = crate::BUILTIN_ONE.family().dimension();
    pub const DataSize: Self = crate::BUILTIN_BYTE.family().dimension();
    pub const PixelCount: Self = crate::BUILTIN_PIXEL.family().dimension();
    pub const Mass: Self = crate::BUILTIN_GRAM.family().dimension();
    pub const Area: Self = crate::BUILTIN_SQUARE_METER.family().dimension();
    pub const Volume: Self = crate::BUILTIN_CUBIC_METER.family().dimension();
    pub const Speed: Self = crate::BUILTIN_METER_PER_SECOND.family().dimension();
    pub const Acceleration: Self = crate::BUILTIN_METER_PER_SECOND_SQUARED.family().dimension();
    pub const Force: Self = crate::BUILTIN_NEWTON.family().dimension();
    pub const Energy: Self = crate::BUILTIN_JOULE.family().dimension();
    pub const Power: Self = crate::BUILTIN_WATT.family().dimension();
    pub const Pressure: Self = crate::BUILTIN_PASCAL.family().dimension();
    pub fn info_id(self) -> &'static str {
        if self == Self::Time {
            return DURATION_INFO_ID;
        }
        if self == Self::Frequency {
            return FREQUENCY_INFO_ID;
        }
        if self == Self::Voltage {
            return VOLTAGE_INFO_ID;
        }
        if self == Self::Temperature {
            return TEMPERATURE_INFO_ID;
        }
        if self == Self::Length {
            return DISTANCE_INFO_ID;
        }
        if self == Self::Angle {
            return ANGLE_INFO_ID;
        }
        if self == Self::Ratio {
            return RATIO_INFO_ID;
        }
        if self == Self::PixelCount {
            return PIXEL_COUNT_INFO_ID;
        }
        QUANTITY_INFO_ID
    }
}
pub fn quantity_info_dimension(identity: &str) -> Option<QuantityDimension> {
    if identity == DURATION_INFO_ID {
        return Some(QuantityDimension::Time);
    }
    if identity == FREQUENCY_INFO_ID {
        return Some(QuantityDimension::Frequency);
    }
    if identity == VOLTAGE_INFO_ID {
        return Some(QuantityDimension::Voltage);
    }
    if identity == TEMPERATURE_INFO_ID {
        return Some(QuantityDimension::Temperature);
    }
    if identity == DISTANCE_INFO_ID {
        return Some(QuantityDimension::Length);
    }
    if identity == ANGLE_INFO_ID {
        return Some(QuantityDimension::Angle);
    }
    if identity == RATIO_INFO_ID {
        return Some(QuantityDimension::Ratio);
    }
    if identity == PIXEL_COUNT_INFO_ID {
        return Some(QuantityDimension::PixelCount);
    }
    None
}
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum QuantityConversionRefusal {
    IncompatibleDimensions,
    IncompatibleQuantityFamilies,
    IncompatibleQuantityRoles,
    Inexact,
    Overflow,
}
