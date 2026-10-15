//! Configuration projection and diagnostic value rendering.
use crate::prelude::*;
use crate::{
    ConfigurationValue, KindConfigurationField, KindConfigurationRule, StartupParameterSignature,
};
/// Preserve checked configuration defaults in the startup IR, without Source rendering.
pub(crate) fn install_structured_defaults(
    startup: &mut crate::StartupCatalog,
    signature: &crate::KindSignature,
    fields: &[KindConfigurationField],
) -> Result<(), String> {
    for field in fields {
        let ConfigurationValue::Structured(configuration) = &field.default_value else {
            continue;
        };
        // Built-in semantic values such as Dimension have their own checked
        // Source literal owner; their configuration envelope is not a new Type.
        if field.default_value.semantic_kind() != *configuration.profile() {
            continue;
        }
        let value = conduit_core::StructuredInfoValue::from_canonical_bytes(
            configuration.canonical_value(),
        )
        .map_err(|error| {
            format!(
                "invalid canonical structured default '{}': {error:?}",
                field.key
            )
        })?;
        if value
            .value_type()
            .profile()
            .map_err(|error| format!("invalid structured default type: {error:?}"))?
            .value_kind()
            != configuration.profile()
        {
            return Err(format!(
                "structured default '{}' differs from its exact profile",
                field.key
            ));
        }
        let Some(parameter) = signature
            .startup_parameters
            .iter()
            .find(|parameter| parameter.name == field.key)
        else {
            // Internal configuration fields are not callable startup defaults.
            continue;
        };
        startup.ensure_structured_type(parameter.value_type.clone(), value.value_type().clone())?;
        if parameter.default.is_some() {
            let stored = crate::checked_syntax::StoredStructuredDefault::from_checked_value(&value)
                .map_err(|error| {
                    format!(
                        "invalid structured default encoding '{}': {error:?}",
                        field.key
                    )
                })?;
            drop(value);
            startup
                .structured_defaults
                .insert((signature.kind.clone(), parameter.name.clone()), stored);
        }
    }
    Ok(())
}

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
            (_, ConfigurationValue::Structured(_))
                if field.default_value.semantic_kind().as_str()
                    == conduit_core::DIMENSION_INFO_ID =>
            {
                "Dimension"
            }
            (_, ConfigurationValue::Structured(value)) => value.profile().as_str(),
            (_, ConfigurationValue::Quantity(_)) => "Quantity",
            (_, ConfigurationValue::Unit(_)) => "Unit",
            (_, ConfigurationValue::TemperatureDifference(_)) => "TemperatureDifference",
        }
        .into(),
        // Projections without a declared callable Fore use the checked
        // configuration default as their omission contract.
        default: if matches!(field.rule, KindConfigurationRule::DataLabel { .. }) {
            None
        } else {
            Some(render_value(&field.default_value))
        },
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

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{
        KindIdentity, StructuredFieldType, StructuredFieldValue, StructuredInfoType,
        StructuredInfoValue,
    };

    #[test]
    fn profile_derived_structured_default_retains_checked_bytes_without_source_rendering() {
        let text_type =
            StructuredInfoType::leaf(conduit_core::kind_id(conduit_core::BOOL_INFO_ID)).unwrap();
        let value_type = StructuredInfoType::record(
            conduit_core::kind_id("fixture/settings@1"),
            vec![StructuredFieldType::new("label", text_type.clone()).unwrap()],
        )
        .unwrap();
        let value = StructuredInfoValue::record(
            value_type.clone(),
            vec![StructuredFieldValue::new(
                "label",
                StructuredInfoValue::leaf(
                    text_type,
                    conduit_core::InfoBool::TRUE.encode().to_vec(),
                )
                .unwrap(),
            )
            .unwrap()],
        )
        .unwrap();
        let profile = value_type.profile().unwrap().value_kind().clone();
        let configuration = conduit_core::StructuredConfigurationValue::new(
            profile.clone(),
            value.canonical_bytes().unwrap(),
        )
        .unwrap();
        let mut catalog = crate::ProfileCatalog::new();
        catalog
            .insert(crate::KindProjection {
                kind_id: "fixture/structured-default".into(),
                kind_contract_revision: KindIdentity::from("fixture/structured-default@1"),
                inputs: vec![],
                outputs: vec![],
                configuration: vec![KindConfigurationField {
                    key: "settings".into(),
                    default_value: ConfigurationValue::Structured(configuration.clone()),
                    rule: KindConfigurationRule::Structured { profile },
                }],
            })
            .unwrap();
        let checked = crate::parse(
            "plot defaults {\n item: fixture/structured-default\n}",
            &catalog,
        )
        .unwrap();
        assert_eq!(
            checked.gears[0].configuration[0].value,
            ConfigurationValue::Structured(configuration)
        );
        // Source-owned defaults on a same-name Plot must not inherit the
        // installed primitive's prepared default.
        let startup = catalog.startup_catalog().unwrap();
        let syntax = crate::parse_syntax_document(
            "plot fixture/structured-default (\n settings: Count = 9\n) {\n}\nplot caller {\n item: fixture/structured-default\n}",
        );
        let source = crate::check_syntax_document(&syntax, &startup).unwrap();
        assert_eq!(
            source
                .plots
                .iter()
                .find(|plot| plot.name == "caller")
                .unwrap()
                .gears[0]
                .startup_bindings[0]
                .value,
            crate::CanonicalStartupValue::Literal("9".into())
        );
    }
}
