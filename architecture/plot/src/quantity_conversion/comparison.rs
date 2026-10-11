//! Ordinary checked exact comparisons retain both operands and their physical
//! laws. No target projection or rounded numeric value participates in ordering.
mod operand;
use super::*;
use core::cmp::Ordering;

pub const KIND: &str = "units/compare";
pub const DIFFERENCE_KIND: &str = "units/compare-temperature-differences";
pub const RECEIPT_NAME: &str = "ExactQuantityComparisonReceipt";
pub const DIFFERENCE_RECEIPT_NAME: &str = "ExactTemperatureDifferenceComparisonReceipt";

fn identities(
    profile: ConversionProfile,
) -> (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
) {
    match profile {
        ConversionProfile::Quantity => (
            KIND,
            RECEIPT_NAME,
            "quantity/exact-comparison@2",
            "quantity/exact-comparison-receipt@1",
            "quantity/exact-comparison-result@1",
        ),
        ConversionProfile::TemperatureDifference => (
            DIFFERENCE_KIND,
            DIFFERENCE_RECEIPT_NAME,
            "quantity/exact-temperature-difference-comparison@2",
            "quantity/exact-temperature-difference-comparison-receipt@1",
            "quantity/exact-temperature-difference-comparison-result@1",
        ),
    }
}
fn result_type(profile: ConversionProfile) -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id(identities(profile).4),
        vec![
            StructuredVariantCase::new("less", leaf(EMPTY_INFO_ID)).expect("reviewed case"),
            StructuredVariantCase::new("equal", leaf(EMPTY_INFO_ID)).expect("reviewed case"),
            StructuredVariantCase::new("greater", leaf(EMPTY_INFO_ID)).expect("reviewed case"),
            StructuredVariantCase::new("refused", leaf(TEXT_INFO_ID)).expect("reviewed case"),
        ],
    )
    .expect("finite comparison result")
}
fn receipt_for(profile: ConversionProfile) -> StructuredInfoType {
    StructuredInfoType::record(
        kind_id(identities(profile).3),
        vec![
            field("left", operand::value_type(profile)),
            field("right", operand::value_type(profile)),
            field("profile", leaf(TEXT_INFO_ID)),
            field("result", result_type(profile)),
        ],
    )
    .expect("bounded comparison receipt")
}
pub fn receipt_type() -> StructuredInfoType {
    receipt_for(ConversionProfile::Quantity)
}
pub fn difference_receipt_type() -> StructuredInfoType {
    receipt_for(ConversionProfile::TemperatureDifference)
}
pub fn contract() -> Kind {
    contract_for(ConversionProfile::Quantity)
}
pub fn difference_contract() -> Kind {
    contract_for(ConversionProfile::TemperatureDifference)
}
fn contract_for(profile: ConversionProfile) -> Kind {
    let (kind, _, revision, _, _) = identities(profile);
    typed_arguments::contract(
        kind,
        revision,
        ["left", "right"],
        [profile.configuration_rule(), profile.configuration_rule()],
        receipt_for(profile),
    )
}
pub(super) fn install(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    for role in [
        ConversionProfile::Quantity,
        ConversionProfile::TemperatureDifference,
    ] {
        install_receipt(
            identities(role).1,
            receipt_for(role),
            contract_for(role),
            startup,
            profile,
        )?;
    }
    Ok(())
}
pub fn prepare_configuration(
    configuration: &[ConfigurationEntry],
) -> Result<StructuredInfoValue, QuantityConversionPreparationRefusal> {
    prepare_for(ConversionProfile::Quantity, configuration)
}
pub fn prepare_difference_configuration(
    configuration: &[ConfigurationEntry],
) -> Result<StructuredInfoValue, QuantityConversionPreparationRefusal> {
    prepare_for(ConversionProfile::TemperatureDifference, configuration)
}
fn prepare_for(
    profile: ConversionProfile,
    configuration: &[ConfigurationEntry],
) -> Result<StructuredInfoValue, QuantityConversionPreparationRefusal> {
    use QuantityConversionPreparationRefusal as R;
    let [left, right] = typed_arguments::arguments(configuration, ["left", "right"])?;
    let left = operand::Operand::from_checked(profile, left).map_err(R::ComparisonLeft)?;
    let right = operand::Operand::from_checked(profile, right).map_err(R::ComparisonRight)?;
    let (tag, payload) = match left.compare(&right) {
        Ok(ordering) => (
            match ordering {
                Ordering::Less => "less",
                Ordering::Equal => "equal",
                Ordering::Greater => "greater",
            },
            StructuredInfoValue::leaf(leaf(EMPTY_INFO_ID), vec![]).map_err(R::Receipt)?,
        ),
        Err(refusal) => ("refused", text(refusal_reason(refusal))?),
    };
    let result =
        StructuredInfoValue::variant(result_type(profile), tag, payload).map_err(R::Receipt)?;
    let receipt = StructuredInfoValue::record(
        receipt_for(profile),
        [
            ("left", left.encode(profile)?),
            ("right", right.encode(profile)?),
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
fn text(value: &str) -> Result<StructuredInfoValue, QuantityConversionPreparationRefusal> {
    StructuredInfoValue::leaf(leaf(TEXT_INFO_ID), value.as_bytes().to_vec())
        .map_err(QuantityConversionPreparationRefusal::Receipt)
}
pub fn validate_receipt(
    receipt: &StructuredInfoValue,
) -> Result<(), QuantityConversionPreparationRefusal> {
    validate_for(ConversionProfile::Quantity, receipt)
}
pub fn validate_difference_receipt(
    receipt: &StructuredInfoValue,
) -> Result<(), QuantityConversionPreparationRefusal> {
    validate_for(ConversionProfile::TemperatureDifference, receipt)
}
fn validate_for(
    profile: ConversionProfile,
    receipt: &StructuredInfoValue,
) -> Result<(), QuantityConversionPreparationRefusal> {
    use QuantityConversionPreparationRefusal as R;
    if receipt.value_type() != &receipt_for(profile) {
        return Err(R::ForgedReceipt);
    }
    if receipt.canonical_bytes().map_err(R::Receipt)?.len() > MAXIMUM_RECEIPT_BYTES as usize {
        return Err(R::ReceiptTooLarge);
    }
    let StructuredInfoValueShape::Record(fields) = receipt.shape() else {
        return Err(R::ForgedReceipt);
    };
    let original = |name: &str| -> Result<String, R> {
        let operand = fields
            .iter()
            .find(|field| field.name() == name)
            .ok_or(R::ForgedReceipt)?
            .value();
        let StructuredInfoValueShape::Record(fields) = operand.shape() else {
            return Err(R::ForgedReceipt);
        };
        let original = fields
            .iter()
            .find(|field| field.name() == "original")
            .ok_or(R::ForgedReceipt)?
            .value();
        let StructuredInfoValueShape::Leaf(bytes) = original.shape() else {
            return Err(R::ForgedReceipt);
        };
        if bytes.len() > QUANTITY_MAX_LITERAL_BYTES {
            return Err(R::ForgedReceipt);
        }
        core::str::from_utf8(bytes)
            .map(|value| value.into())
            .map_err(|_| R::ForgedReceipt)
    };
    let expected = prepare_for(
        profile,
        &[
            ConfigurationEntry {
                key: "left".into(),
                value: operand::from_receipt(profile, receipt, "left", &original("left")?)?,
            },
            ConfigurationEntry {
                key: "right".into(),
                value: operand::from_receipt(profile, receipt, "right", &original("right")?)?,
            },
        ],
    )?;
    if receipt != &expected {
        return Err(R::ForgedReceipt);
    }
    Ok(())
}
