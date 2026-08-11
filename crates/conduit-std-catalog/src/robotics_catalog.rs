use super::{
    configuration_type, robotics_contracts_with_revisions, StandardConfigurationField,
    StandardConfigurationRule,
};
use alloc::format;
use alloc::string::{String, ToString};
use conduit_core::{ConfigurationValue, KindContractRevision};
use conduit_form::{
    ConfigurationField, ConfigurationRule, KindDefinition, KindSignature, StartupParameterSignature,
};

pub fn install_robotics_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    for (contract, revision) in robotics_contracts_with_revisions() {
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
            .map(|field| ConfigurationField {
                key: field.key.clone(),
                default_value: field.default_value.clone(),
                validation: match &field.rule {
                    StandardConfigurationRule::U64Range { minimum, maximum } => {
                        ConfigurationRule::U64Range {
                            minimum: *minimum,
                            maximum: *maximum,
                        }
                    }
                    StandardConfigurationRule::I64Range { minimum, maximum } => {
                        ConfigurationRule::I64Range {
                            minimum: *minimum,
                            maximum: *maximum,
                        }
                    }
                    StandardConfigurationRule::TextOneOf { values } => {
                        ConfigurationRule::TextOneOf {
                            values: values.clone(),
                        }
                    }
                    _ => unreachable!("robotics uses only finite numeric/text rules"),
                },
            })
            .collect();
        profile
            .insert(KindDefinition {
                kind_id: contract.kind_id,
                kind_contract_revision: KindContractRevision::from(revision),
                inputs: contract.inputs,
                outputs: contract.outputs,
                configuration,
            })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn configuration_source(field: &StandardConfigurationField) -> String {
    match &field.default_value {
        ConfigurationValue::Text(value) => format!("\"{value}\""),
        ConfigurationValue::U64(value) => value.to_string(),
        ConfigurationValue::I64(value) => value.to_string(),
        _ => unreachable!("robotics configuration is finite text/integer"),
    }
}
