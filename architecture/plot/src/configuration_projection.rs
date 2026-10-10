//! Configuration projection and diagnostic value rendering.
use crate::prelude::*;
use crate::{
    ConfigurationValue, KindConfigurationField, KindConfigurationRule, StartupParameterSignature,
};
pub(crate) fn projected_startup_parameter(
    field: &KindConfigurationField,
) -> StartupParameterSignature {
    StartupParameterSignature {
        name: field.key.clone(),
        value_type: match (&field.rule, &field.default_value) {
            (
                KindConfigurationRule::QuantityRange { canonical_unit, .. },
                ConfigurationValue::Quantity(_),
            ) => canonical_unit.dimension().info_id(),
            (_, ConfigurationValue::Bool(_)) => "Boolean",
            (_, ConfigurationValue::U64(_)) => "Count",
            (_, ConfigurationValue::I64(_)) => "Scalar",
            (_, ConfigurationValue::Text(_)) => "Text",
            (_, ConfigurationValue::Structured(value)) => value.profile().as_str(),
            (_, ConfigurationValue::Quantity(_)) => "Quantity",
            (_, ConfigurationValue::Unit(_)) => "Unit",
            (_, ConfigurationValue::TemperatureDifference(_)) => "TemperatureDifference",
        }
        .into(),
        // Projections without a declared callable Fore use the checked
        // configuration default as their omission contract.
        default: Some(render_value(&field.default_value)),
    }
}

pub(crate) fn render_value(value: &ConfigurationValue) -> String {
    match value {
        ConfigurationValue::Bool(value) => value.to_string(),
        ConfigurationValue::U64(value) => value.to_string(),
        ConfigurationValue::I64(value) => value.to_string(),
        ConfigurationValue::Text(value) => format!("{value:?}"),
        ConfigurationValue::Structured(value) => alloc::format!(
            "<structured:{}:{}-bytes>",
            value.profile().as_str(),
            value.canonical_value().len()
        ),
        ConfigurationValue::Quantity(value) => value.source().into(),
        ConfigurationValue::Unit(value) => value.source().into(),
        ConfigurationValue::TemperatureDifference(value) => value.source().into(),
    }
}
