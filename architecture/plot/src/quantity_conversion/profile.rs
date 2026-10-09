//! The semantic role is selected by the exact Kind, not a mutable flag in a
//! quantity value. Both contracts share finite representation machinery.
use super::*;

#[derive(Clone, Copy)]
pub(super) enum ConversionProfile {
    Quantity,
    TemperatureDifference,
}
impl ConversionProfile {
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
    pub(super) fn source_id(self) -> &'static str {
        match self {
            Self::Quantity => EXACT_DECIMAL_QUANTITY_INFO_ID,
            Self::TemperatureDifference => EXACT_TEMPERATURE_DIFFERENCE_INFO_ID,
        }
    }
    pub(super) fn source_type(self) -> StructuredInfoType {
        match self {
            Self::Quantity => leaf(EXACT_DECIMAL_QUANTITY_INFO_ID),
            Self::TemperatureDifference => StructuredInfoType::record(
                kind_id(EXACT_TEMPERATURE_DIFFERENCE_INFO_ID),
                vec![field("coordinate", leaf(EXACT_DECIMAL_QUANTITY_INFO_ID))],
            )
            .expect("bounded difference"),
        }
    }
    pub(super) fn source_value(
        self,
        coordinate: ExactDecimalQuantity,
    ) -> Result<StructuredInfoValue, StructuredInfoRefusal> {
        let coordinate = StructuredInfoValue::leaf(
            leaf(EXACT_DECIMAL_QUANTITY_INFO_ID),
            coordinate.encode().to_vec(),
        )?;
        match self {
            Self::Quantity => Ok(coordinate),
            Self::TemperatureDifference => StructuredInfoValue::record(
                self.source_type(),
                vec![StructuredFieldValue::new("coordinate", coordinate)?],
            ),
        }
    }
}
