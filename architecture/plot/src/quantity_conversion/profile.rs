//! Source-role contracts use the single self-contained Quantity carrier.
use super::*;

#[derive(Clone, Copy)]
pub(super) enum ConversionProfile {
    Quantity,
    TemperatureDifference,
}
impl ConversionProfile {
    pub(super) fn configuration_rule(self) -> KindConfigurationRule {
        match self {
            Self::Quantity => KindConfigurationRule::Quantity,
            Self::TemperatureDifference => KindConfigurationRule::TemperatureDifference,
        }
    }
    pub(super) fn kind(self) -> &'static str {
        match self {
            Self::Quantity => KIND,
            Self::TemperatureDifference => temperature_difference::KIND,
        }
    }
    pub(super) fn revision(self) -> &'static str {
        match self {
            Self::Quantity => REVISION,
            Self::TemperatureDifference => temperature_difference::REVISION,
        }
    }
    pub(super) fn receipt_name(self) -> &'static str {
        match self {
            Self::Quantity => RECEIPT_NAME,
            Self::TemperatureDifference => temperature_difference::RECEIPT_NAME,
        }
    }
    pub(super) fn receipt_id(self) -> &'static str {
        match self {
            Self::Quantity => "quantity/exact-conversion-receipt@1",
            Self::TemperatureDifference => {
                "quantity/exact-temperature-difference-conversion-receipt@1"
            }
        }
    }
    pub(super) fn result_id(self) -> &'static str {
        match self {
            Self::Quantity => "quantity/exact-conversion-result@1",
            Self::TemperatureDifference => {
                "quantity/exact-temperature-difference-conversion-result@1"
            }
        }
    }
    pub(super) fn coordinate_id(self) -> &'static str {
        match self {
            Self::Quantity => "quantity/exact-target-coordinate@1",
            Self::TemperatureDifference => {
                "quantity/exact-temperature-difference-target-coordinate@1"
            }
        }
    }
    pub(super) fn source_id(self) -> String {
        match self {
            Self::Quantity => QUANTITY_INFO_ID.into(),
            Self::TemperatureDifference => temperature_delta_info_id(),
        }
    }
    pub(super) fn source_type(self) -> StructuredInfoType {
        match self {
            Self::Quantity => leaf(QUANTITY_INFO_ID),
            Self::TemperatureDifference => exact_temperature_difference_type(),
        }
    }
    pub(super) fn source_value(
        self,
        coordinate: Quantity,
    ) -> Result<StructuredInfoValue, StructuredInfoRefusal> {
        StructuredInfoValue::leaf(self.source_type(), coordinate.encode().to_vec())
    }
}
