//! Typed Fores and configuration extraction; spelling remains evidence.
use super::*;

pub(super) fn arguments<'a>(
    configuration: &'a [ConfigurationEntry],
    names: [&str; 2],
) -> Result<[&'a ConfigurationValue; 2], QuantityConversionPreparationRefusal> {
    use QuantityConversionPreparationRefusal as R;
    if configuration.len() != 2 {
        return Err(R::Configuration);
    }
    let get = |name| {
        let mut entries = configuration.iter().filter(|v| v.key == name);
        let value = entries.next().ok_or(R::Configuration)?;
        if entries.next().is_some() {
            return Err(R::Configuration);
        }
        Ok(&value.value)
    };
    Ok([get(names[0])?, get(names[1])?])
}
pub(super) fn contract(
    kind: &str,
    revision: &str,
    names: [&str; 2],
    rules: [KindConfigurationRule; 2],
    receipt: StructuredInfoType,
) -> Kind {
    typed_receipt_contract(kind, revision, names, rules, receipt)
}
pub(super) fn default_value(rule: &KindConfigurationRule) -> ConfigurationValue {
    match rule {
        KindConfigurationRule::Quantity => ConfigurationValue::Quantity(
            QuantityConfigurationValue::parse("0Hz").expect("reviewed default"),
        ),
        KindConfigurationRule::Unit => {
            ConfigurationValue::Unit(UnitConfigurationValue::parse("Hz").expect("reviewed default"))
        }
        KindConfigurationRule::TemperatureDifference => ConfigurationValue::TemperatureDifference(
            ExactTemperatureDifferenceConfigurationValue::parse("0K").expect("reviewed default"),
        ),
        _ => unreachable!("physical rule"),
    }
}
pub(super) fn rule_kind(rule: &KindConfigurationRule) -> KindId {
    match rule {
        KindConfigurationRule::Quantity => kind_id(QUANTITY_INFO_ID),
        KindConfigurationRule::Unit => kind_id(UNIT_INFO_ID),
        KindConfigurationRule::TemperatureDifference => temperature_difference::source_type()
            .profile()
            .expect("difference")
            .value_kind()
            .clone(),
        _ => unreachable!("physical rule"),
    }
}
pub(super) fn source_from_receipt(
    profile: ConversionProfile,
    receipt: &StructuredInfoValue,
    original: &str,
) -> Result<ConfigurationValue, QuantityConversionPreparationRefusal> {
    use QuantityConversionPreparationRefusal as R;
    let StructuredInfoValueShape::Record(fields) = receipt.shape() else {
        return Err(R::ForgedReceipt);
    };
    let source = fields
        .iter()
        .find(|f| f.name() == "source")
        .ok_or(R::ForgedReceipt)?
        .value();
    match profile {
        ConversionProfile::Quantity => {
            let StructuredInfoValueShape::Leaf(bytes) = source.shape() else {
                return Err(R::ForgedReceipt);
            };
            let source = Quantity::decode(bytes).map_err(|_| R::ForgedReceipt)?;
            QuantityConfigurationValue::new(source, original.into())
                .map(ConfigurationValue::Quantity)
                .ok_or(R::ForgedReceipt)
        }
        ConversionProfile::TemperatureDifference => {
            let source = temperature_difference::validate_source_value(source)?;
            ExactTemperatureDifferenceConfigurationValue::new(source, original.into())
                .map(ConfigurationValue::TemperatureDifference)
                .ok_or(R::ForgedReceipt)
        }
    }
}
