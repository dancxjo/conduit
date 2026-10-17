//! Checked physical values are the authority; source spelling remains evidence.
use super::*;

pub(super) fn prepare(
    profile: ConversionProfile,
    source: &ConfigurationValue,
    target: &ConfigurationValue,
) -> Result<StructuredInfoValue, QuantityConversionPreparationRefusal> {
    use QuantityConversionPreparationRefusal as R;
    let ConfigurationValue::Unit(target) = target else {
        return Err(R::Configuration);
    };
    let (original, coordinate) = match (profile, source) {
        (ConversionProfile::Quantity, ConfigurationValue::Quantity(source)) => {
            (source.source(), source.value())
        }
        (
            ConversionProfile::TemperatureDifference,
            ConfigurationValue::TemperatureDifference(source),
        ) => (source.source(), source.value().storage_coordinate()),
        _ => return Err(R::Configuration),
    };
    if !coordinate.matches_literal_evidence(original)
        || !target.value().matches_source_evidence(target.source())
    {
        return Err(R::SourceCorrelation);
    }
    let value = |identity: &str, bytes: Vec<u8>| {
        StructuredInfoValue::leaf(leaf(identity), bytes).map_err(R::Receipt)
    };
    let text = |text: &str| value(TEXT_INFO_ID, text.as_bytes().to_vec());
    let result = match coordinate.convert_to_unit(target.value()) {
        Ok((coefficient, exponent)) => {
            let coordinate = StructuredInfoValue::record(
                coordinate_type(profile),
                vec![
                    StructuredFieldValue::new(
                        "coefficient",
                        value("value/i128", coefficient.to_le_bytes().to_vec())?,
                    )
                    .map_err(R::Receipt)?,
                    StructuredFieldValue::new(
                        "exponent",
                        value("value/i16", exponent.to_le_bytes().to_vec())?,
                    )
                    .map_err(R::Receipt)?,
                ],
            )
            .map_err(R::Receipt)?;
            StructuredInfoValue::variant(result_type(profile), "converted", coordinate)
                .map_err(R::Receipt)?
        }
        Err(refusal) => StructuredInfoValue::variant(
            result_type(profile),
            "refused",
            text(refusal_reason(refusal))?,
        )
        .map_err(R::Receipt)?,
    };
    let receipt = StructuredInfoValue::record(
        receipt_type_for(profile),
        [
            ("original", text(original)?),
            (
                "source",
                profile.source_value(coordinate).map_err(R::Receipt)?,
            ),
            ("target", text(target.source())?),
            (
                "target-unit",
                value(UNIT_INFO_ID, target.value().encode().to_vec())?,
            ),
            ("profile", text(&profile.source_id())?),
            ("result", result),
        ]
        .into_iter()
        .map(|(name, value)| StructuredFieldValue::new(name, value))
        .collect::<Result<Vec<_>, _>>()
        .map_err(R::Receipt)?,
    )
    .map_err(R::Receipt)?;
    if receipt.canonical_bytes().map_err(R::Receipt)?.len() > MAXIMUM_RECEIPT_BYTES as usize {
        return Err(R::ReceiptTooLarge);
    }
    Ok(receipt)
}
