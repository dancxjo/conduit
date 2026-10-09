//! Explicit ordinary Gear for temperature differences. Its source and output
//! Types cannot connect to the absolute-temperature conversion receipt.
use super::*;

pub const KIND: &str = "units/convert-temperature-difference";
pub const REVISION: &str = "quantity/exact-temperature-difference-conversion@1";
pub const RECEIPT_NAME: &str = "ExactTemperatureDifferenceConversionReceipt";

pub fn receipt_type() -> StructuredInfoType {
    receipt_type_for(ConversionProfile::TemperatureDifference)
}
pub fn source_type() -> StructuredInfoType {
    ConversionProfile::TemperatureDifference.source_type()
}
pub fn contract() -> Kind {
    contract_for(ConversionProfile::TemperatureDifference)
}
pub fn prepare_configuration(
    configuration: &[ConfigurationEntry],
) -> Result<StructuredInfoValue, QuantityConversionPreparationRefusal> {
    prepare_for(ConversionProfile::TemperatureDifference, configuration)
}
pub fn validate_receipt(
    receipt: &StructuredInfoValue,
) -> Result<(), QuantityConversionPreparationRefusal> {
    validate_for(ConversionProfile::TemperatureDifference, receipt)
}

/// Admit the distinct source record before a consumer uses its coordinate.
/// Shape alone cannot authorize a non-temperature coordinate as a difference.
pub fn validate_source_value(
    value: &StructuredInfoValue,
) -> Result<ExactTemperatureDifference, QuantityConversionPreparationRefusal> {
    use QuantityConversionPreparationRefusal as R;
    if value.value_type() != &source_type() {
        return Err(R::ForgedReceipt);
    }
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        return Err(R::ForgedReceipt);
    };
    let Some(field) = fields.iter().find(|field| field.name() == "coordinate") else {
        return Err(R::ForgedReceipt);
    };
    let StructuredInfoValueShape::Leaf(bytes) = field.value().shape() else {
        return Err(R::ForgedReceipt);
    };
    let coordinate = ExactDecimalQuantity::decode(bytes).map_err(|error| {
        R::Request(
            ExactQuantityConversionRequestRefusal::TemperatureDifferenceSource(
                ExactTemperatureDifferenceRefusal::Coordinate(error),
            ),
        )
    })?;
    ExactTemperatureDifference::new(
        coordinate.coefficient(),
        coordinate.exponent(),
        coordinate.unit(),
    )
    .map_err(|error| {
        R::Request(ExactQuantityConversionRequestRefusal::TemperatureDifferenceSource(error))
    })
}
