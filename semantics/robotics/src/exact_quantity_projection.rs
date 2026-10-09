//! Checked exact quantities project into the existing bounded observation
//! records. Constructing a value does not assert a physical sensor reading.
use conduit_core::{ExactDecimalQuantity, InfoDecodeError, QuantityUnit};

use crate::{BatteryObservation, RangeObservation};

impl RangeObservation {
    /// Explicit millimetre/millisecond projection; source scale remains exact
    /// until this consumer's reviewed integer representation is selected.
    pub fn from_exact_quantities(
        distance: ExactDecimalQuantity,
        age: ExactDecimalQuantity,
    ) -> Result<Self, InfoDecodeError> {
        Self::from_quantities(
            distance
                .convert_to_legacy(QuantityUnit::Millimeter)
                .map_err(InfoDecodeError::QuantityConversion)?,
            age.convert_to_legacy(QuantityUnit::Millisecond)
                .map_err(InfoDecodeError::QuantityConversion)?,
        )
    }
}

impl BatteryObservation {
    /// Explicit permille/millivolt projection preserves the existing battery
    /// record identity and exact bounds, including refusal of sub-millivolts.
    pub fn from_exact_quantities(
        charge: ExactDecimalQuantity,
        voltage: ExactDecimalQuantity,
    ) -> Result<Self, InfoDecodeError> {
        Self::from_quantities(
            charge
                .convert_to_legacy(QuantityUnit::Permille)
                .map_err(InfoDecodeError::QuantityConversion)?,
            voltage
                .convert_to_legacy(QuantityUnit::Millivolt)
                .map_err(InfoDecodeError::QuantityConversion)?,
        )
    }
}
