//! Both semantic roles use the same bounded receipt encoding. Difference
//! admission supplies distinct source and result Types and zero offsets.
use super::*;

struct Facts<'a> {
    original: &'a str,
    source: Quantity,
    source_suffix: ResolvedQuantitySuffix<'a>,
    target: ResolvedQuantitySuffix<'a>,
    source_transform: (i128, i128, i128),
    target_transform: (i128, i128, i128),
    result: Result<(i128, i16), QuantityConversionRefusal>,
}

pub(super) fn prepare(
    profile: ConversionProfile,
    source: &ConfigurationValue,
    target: &ConfigurationValue,
) -> Result<StructuredInfoValue, QuantityConversionPreparationRefusal> {
    use QuantityConversionPreparationRefusal as R;
    let ConfigurationValue::Unit(target) = target else {
        return Err(R::Configuration);
    };
    let receipt = match (profile, source) {
        (ConversionProfile::Quantity, ConfigurationValue::Quantity(source)) => {
            let receipt = ExactQuantityConversionReceipt::from_checked(
                source.value(),
                target.value(),
                source.source(),
                target.source(),
            )
            .map_err(R::Request)?;
            Facts {
                original: source.source(),
                source: receipt.source(),
                source_suffix: receipt.source_suffix(),
                target: receipt.target(),
                source_transform: receipt.source_transform(),
                target_transform: receipt.target_transform(),
                result: receipt
                    .result()
                    .map(|value| (value.coefficient(), value.exponent())),
            }
        }
        (
            ConversionProfile::TemperatureDifference,
            ConfigurationValue::TemperatureDifference(source),
        ) => {
            let receipt = ExactTemperatureDifferenceConversionReceipt::from_checked(
                source.value(),
                target.value(),
                source.source(),
                target.source(),
            )
            .map_err(R::Request)?;
            Facts {
                original: source.source(),
                source: receipt.source().storage_coordinate(),
                source_suffix: receipt.source_suffix(),
                target: receipt.target(),
                source_transform: receipt.source_transform(),
                target_transform: receipt.target_transform(),
                result: receipt
                    .result()
                    .map(|value| (value.coefficient(), value.exponent())),
            }
        }
        _ => return Err(R::Configuration),
    };
    let (ss, so, sd) = receipt.source_transform;
    let (ts, to, td) = receipt.target_transform;
    let value = |identity: &str, bytes: Vec<u8>| {
        StructuredInfoValue::leaf(leaf(identity), bytes).map_err(R::Receipt)
    };
    let text = |text: &str| value(TEXT_INFO_ID, text.as_bytes().to_vec());
    let result = match receipt.result {
        Ok(coordinate) => {
            let coordinate = StructuredInfoValue::record(
                coordinate_type(profile),
                vec![
                    StructuredFieldValue::new(
                        "coefficient",
                        value("value/i128", coordinate.0.to_le_bytes().to_vec())?,
                    )
                    .map_err(R::Receipt)?,
                    StructuredFieldValue::new(
                        "exponent",
                        value("value/i16", coordinate.1.to_le_bytes().to_vec())?,
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
            text(match refusal {
                QuantityConversionRefusal::IncompatibleDimensions => "incompatible-dimensions",
                QuantityConversionRefusal::Inexact => "inexact",
                QuantityConversionRefusal::Overflow => "overflow",
            })?,
        )
        .map_err(R::Receipt)?,
    };
    let mut fields = vec![
        ("original", text(receipt.original)?),
        (
            "source",
            profile.source_value(receipt.source).map_err(R::Receipt)?,
        ),
        ("source-suffix", text(receipt.source_suffix.source())?),
        (
            "source-base",
            text(receipt.source_suffix.unit().base_unit().plot_suffix())?,
        ),
        (
            "source-prefix",
            text(
                &receipt
                    .source_suffix
                    .prefix()
                    .map_or("", |prefix| prefix.symbol()),
            )?,
        ),
        (
            "source-prefix-exponent",
            value(
                "value/i16",
                i16::from(
                    receipt
                        .source_suffix
                        .prefix()
                        .map_or(0, |prefix| prefix.exponent()),
                )
                .to_le_bytes()
                .to_vec(),
            )?,
        ),
        (
            "source-dimension",
            text(dimension_name(receipt.source.dimension()))?,
        ),
        (
            "target-dimension",
            text(dimension_name(receipt.target.unit().dimension()))?,
        ),
        ("target", text(receipt.target.source())?),
        ("profile", text(profile.source_id())?),
        ("catalogue", text(QUANTITY_PREFIX_CATALOG_ID)?),
        (
            "target-exponent",
            value(
                "value/i16",
                receipt
                    .target
                    .decimal_exponent()
                    .unwrap_or(0)
                    .to_le_bytes()
                    .to_vec(),
            )?,
        ),
        ("result", result),
    ];
    for (name, number) in [
        ("source-scale", ss),
        ("source-offset", so),
        ("source-denominator", sd),
        ("target-scale", ts),
        ("target-offset", to),
        ("target-denominator", td),
    ] {
        fields.push((name, value("value/i128", number.to_le_bytes().to_vec())?));
    }
    let receipt = StructuredInfoValue::record(
        receipt_type_for(profile),
        fields
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
