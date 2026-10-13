//! Typed finite quantities with exact-only conversions.

mod exact;
pub(crate) mod numeric_quantity;
mod receipt;
mod target;
mod temperature_difference;

pub use exact::*;
pub use receipt::*;
pub use target::*;
pub use temperature_difference::*;

use serde::{Deserialize, Serialize};
pub const DISTANCE_INFO_ID: &str = crate::BUILTIN_DISTANCE_INFO_ID;
pub const FREQUENCY_INFO_ID: &str = crate::BUILTIN_FREQUENCY_INFO_ID;
pub const ANGULAR_FREQUENCY_INFO_ID:&str=crate::BUILTIN_ANGULAR_FREQUENCY_INFO_ID;
pub const DURATION_INFO_ID: &str = crate::BUILTIN_DURATION_INFO_ID;
pub const VOLTAGE_INFO_ID: &str = crate::BUILTIN_VOLTAGE_INFO_ID;
pub const TEMPERATURE_INFO_ID: &str = crate::BUILTIN_TEMPERATURE_INFO_ID;
pub const ANGLE_INFO_ID: &str = crate::BUILTIN_ANGLE_INFO_ID;
pub const RATIO_INFO_ID: &str = crate::BUILTIN_RATIO_INFO_ID;
pub const PIXEL_COUNT_INFO_ID: &str = crate::BUILTIN_PIXEL_COUNT_INFO_ID;
pub type QuantityDimension = crate::DimensionDefinition;
#[allow(non_upper_case_globals)]
impl crate::DimensionDefinition {
    pub const Time: Self = crate::BUILTIN_DURATION_FAMILY.dimension();
    pub const Frequency: Self = crate::BUILTIN_FREQUENCY_FAMILY.dimension();
    pub const Voltage: Self = crate::BUILTIN_VOLTAGE_FAMILY.dimension();
    pub const Current: Self = crate::BUILTIN_CURRENT_FAMILY.dimension();
    pub const Temperature: Self = crate::BUILTIN_TEMPERATURE_FAMILY.dimension();
    pub const Charge: Self = crate::BUILTIN_CHARGE_FAMILY.dimension();
    pub const Length: Self = crate::BUILTIN_DISTANCE_FAMILY.dimension();
    pub const Angle: Self = crate::BUILTIN_ANGLE_FAMILY.dimension();
    pub const Ratio: Self = crate::BUILTIN_RATIO_FAMILY.dimension();
    pub const DataSize: Self = crate::BUILTIN_DATA_SIZE_FAMILY.dimension();
    pub const PixelCount: Self = crate::BUILTIN_PIXEL_COUNT_FAMILY.dimension();
    pub const Mass: Self = crate::BUILTIN_MASS_FAMILY.dimension();
    pub const Area: Self = crate::BUILTIN_AREA_FAMILY.dimension();
    pub const Volume: Self = crate::BUILTIN_VOLUME_FAMILY.dimension();
    pub const Speed: Self = crate::BUILTIN_SPEED_FAMILY.dimension();
    pub const Acceleration: Self = crate::BUILTIN_ACCELERATION_FAMILY.dimension();
    pub const Force: Self = crate::BUILTIN_FORCE_FAMILY.dimension();
    pub const Energy: Self = crate::BUILTIN_ENERGY_FAMILY.dimension();
    pub const Power: Self = crate::BUILTIN_POWER_FAMILY.dimension();
    pub const Pressure: Self = crate::BUILTIN_PRESSURE_FAMILY.dimension();
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
    UnlinkedOrigin,
    EqualityUnknown,
    Numeric(crate::NumericRefusal),
}

impl From<numeric_quantity::QuantityNumericRefusal> for QuantityConversionRefusal {
 fn from(value:numeric_quantity::QuantityNumericRefusal)->Self {use numeric_quantity::QuantityNumericRefusal as R;match value {
 R::IncompatibleDimension=>Self::IncompatibleDimensions,R::IncompatibleFamily=>Self::IncompatibleQuantityFamilies,R::IncompatibleRole|R::InvalidRole=>Self::IncompatibleQuantityRoles,R::UnlinkedOrigin=>Self::UnlinkedOrigin,R::EqualityUnknown=>Self::EqualityUnknown,R::Numeric(n)=>Self::Numeric(n),_=>Self::Inexact}}
}
