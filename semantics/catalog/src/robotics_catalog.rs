use super::{
    configuration_type, robotics_contracts_with_revisions,
    robotics_hazard_contracts_with_revisions, robotics_input_contracts_with_revisions,
    KindConfigurationField, KindConfigurationRule,
};
use alloc::format;
use alloc::string::{String, ToString};
use conduit_core::{ConfigurationValue, KindIdentity};
use conduit_form::{KindProjection, KindSignature, StartupParameterSignature};

pub fn install_robotics_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    for (contract, revision) in robotics_contracts_with_revisions()
        .into_iter()
        .chain(robotics_hazard_contracts_with_revisions())
        .chain(robotics_input_contracts_with_revisions())
    {
        startup.insert(KindSignature {
            kind: contract.kind_id.as_str().to_string(),
            startup_parameters: contract
                .configuration
                .iter()
                .map(|field| StartupParameterSignature {
                    name: field.key.clone(),
                    value_type: configuration_type(field).to_string(),
                    default: Some(configuration_source(field)),
                })
                .collect(),
        })?;
        let configuration = contract
            .configuration
            .iter()
            .map(|field| KindConfigurationField {
                key: field.key.clone(),
                default_value: field.default_value.clone(),
                rule: match &field.rule {
                    KindConfigurationRule::U64Range { minimum, maximum } => {
                        KindConfigurationRule::U64Range {
                            minimum: *minimum,
                            maximum: *maximum,
                        }
                    }
                    KindConfigurationRule::I64Range { minimum, maximum } => {
                        KindConfigurationRule::I64Range {
                            minimum: *minimum,
                            maximum: *maximum,
                        }
                    }
                    KindConfigurationRule::TextOneOf { values } => {
                        KindConfigurationRule::TextOneOf {
                            values: values.clone(),
                        }
                    }
                    KindConfigurationRule::QuantityRange {
                        minimum,
                        maximum,
                        canonical_unit,
                    } => KindConfigurationRule::QuantityRange {
                        minimum: *minimum,
                        maximum: *maximum,
                        canonical_unit: *canonical_unit,
                    },
                    _ => unreachable!("robotics uses only finite numeric/text rules"),
                },
            })
            .collect();
        profile
            .insert(KindProjection {
                kind_id: contract.kind_id,
                kind_contract_revision: KindIdentity::from(revision),
                inputs: contract.inputs,
                outputs: contract.outputs,
                configuration,
            })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn configuration_source(field: &KindConfigurationField) -> String {
    match &field.default_value {
        ConfigurationValue::Text(value) => format!("\"{value}\""),
        ConfigurationValue::U64(value) => value.to_string(),
        ConfigurationValue::I64(value) => value.to_string(),
        ConfigurationValue::Quantity(value) => {
            format!("{}{}", value.value(), value.unit().form_suffix())
        }
        _ => unreachable!("robotics configuration is finite text/integer"),
    }
}
