//! Small typed value entrances and result manifestations for finite examples.

use super::{
    StandardConfigurationField, StandardConfigurationRule, StandardKindContract, TerminalBehavior,
};
use alloc::{string::ToString, vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, ConfigurationValue, KindContractRevision, PortDescriptor,
    PortDirection, PortTemporal, BOOL_INFO_ID, SCALAR_INFO_ID,
};

pub const SCALAR_LITERAL_KIND: &str = "scalar/literal";
pub const BOOL_LITERAL_KIND: &str = "boolean/literal";
pub const SCALAR_VALUE_PRESENTATION_KIND: &str = "presentation/scalar";
pub const BOOL_VALUE_PRESENTATION_KIND: &str = "presentation/bool-value";
pub const VALUE_PRIMITIVE_CONTRACT_REVISION: &str = "conduit.std/value-primitives@1";

pub fn scalar_literal_contract() -> StandardKindContract {
    source(
        SCALAR_LITERAL_KIND,
        "Scalar literal",
        SCALAR_INFO_ID,
        StandardConfigurationField {
            key: "value".into(),
            default_value: ConfigurationValue::I64(0),
            rule: StandardConfigurationRule::I64Range {
                minimum: i64::MIN,
                maximum: i64::MAX,
            },
        },
        conduit_core::SCALAR_ENCODED_LEN as u32,
    )
}

pub fn bool_literal_contract() -> StandardKindContract {
    source(
        BOOL_LITERAL_KIND,
        "Boolean literal",
        BOOL_INFO_ID,
        StandardConfigurationField {
            key: "value".into(),
            default_value: ConfigurationValue::Bool(false),
            rule: StandardConfigurationRule::Any,
        },
        conduit_core::BOOL_ENCODED_LEN as u32,
    )
}

pub fn scalar_value_presentation_contract() -> StandardKindContract {
    presentation(
        SCALAR_VALUE_PRESENTATION_KIND,
        "Present scalar value",
        SCALAR_INFO_ID,
        conduit_core::SCALAR_ENCODED_LEN as u32,
    )
}

pub fn bool_value_presentation_contract() -> StandardKindContract {
    presentation(
        BOOL_VALUE_PRESENTATION_KIND,
        "Present Boolean value",
        BOOL_INFO_ID,
        conduit_core::BOOL_ENCODED_LEN as u32,
    )
}

fn source(
    kind: &str,
    name: &str,
    value_kind: &str,
    configuration: StandardConfigurationField,
    bytes: u32,
) -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(kind),
        plain_name: name.to_string(),
        summary: "Emit one exact immutable typed value.".to_string(),
        inputs: Vec::new(),
        outputs: vec![port("value", value_kind, PortDirection::Output)],
        configuration: vec![configuration],
        limits: limits(bytes),
        terminal_behavior: TerminalBehavior::EmitsOnce,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: alloc::format!("source: {kind}"),
    }
}

fn presentation(kind: &str, name: &str, value_kind: &str, bytes: u32) -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(kind),
        plain_name: name.to_string(),
        summary: "Manifest one exact typed terminal value through an admitted presenter."
            .to_string(),
        inputs: vec![port("value", value_kind, PortDirection::Input)],
        outputs: Vec::new(),
        configuration: Vec::new(),
        limits: limits(bytes),
        terminal_behavior: TerminalBehavior::CompletesWhenInputsClose,
        hosted_implementation_required: true,
        browser_manifestation_honest: true,
        pico_manifestation_honest: false,
        example: alloc::format!("show: {kind}"),
    }
}

fn port(name: &str, value_kind: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value_kind),
        direction,
        temporal: PortTemporal::Value,
    }
}

fn limits(bytes: u32) -> CapabilityLimits {
    CapabilityLimits {
        max_active_instances: 16,
        max_queue_items: 1,
        max_queue_bytes: bytes,
    }
}

#[cfg(feature = "form-catalog")]
pub fn install_value_primitive_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use conduit_form::{
        ConfigurationField, ConfigurationRule, KindDefinition, KindSignature,
        StartupParameterSignature,
    };
    for contract in [
        scalar_literal_contract(),
        bool_literal_contract(),
        scalar_value_presentation_contract(),
        bool_value_presentation_contract(),
    ] {
        startup.insert(KindSignature {
            kind: contract.kind_id.as_str().to_string(),
            startup_parameters: contract
                .configuration
                .iter()
                .map(|field| StartupParameterSignature {
                    name: field.key.clone(),
                    value_type: match field.default_value {
                        ConfigurationValue::I64(_) => "Scalar",
                        ConfigurationValue::Bool(_) => "Boolean",
                        _ => unreachable!(),
                    }
                    .to_string(),
                    default: Some(match field.default_value {
                        ConfigurationValue::I64(value) => value.to_string(),
                        ConfigurationValue::Bool(value) => value.to_string(),
                        _ => unreachable!(),
                    }),
                })
                .collect(),
        })?;
        profile
            .insert(KindDefinition {
                kind_id: contract.kind_id,
                kind_contract_revision: KindContractRevision::from(
                    VALUE_PRIMITIVE_CONTRACT_REVISION,
                ),
                inputs: contract.inputs,
                outputs: contract.outputs,
                configuration: contract
                    .configuration
                    .into_iter()
                    .map(|field| ConfigurationField {
                        key: field.key,
                        default_value: field.default_value,
                        validation: match field.rule {
                            StandardConfigurationRule::I64Range { minimum, maximum } => {
                                ConfigurationRule::I64Range { minimum, maximum }
                            }
                            StandardConfigurationRule::Any => ConfigurationRule::Any,
                            _ => unreachable!(),
                        },
                    })
                    .collect(),
            })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitive_entrances_preserve_exact_value_kinds_and_temporal_semantics() {
        let scalar = scalar_literal_contract();
        assert_eq!(scalar.outputs[0].value_kind.as_str(), SCALAR_INFO_ID);
        assert_eq!(scalar.outputs[0].temporal, PortTemporal::Value);
        let boolean = bool_value_presentation_contract();
        assert_eq!(boolean.inputs[0].value_kind.as_str(), BOOL_INFO_ID);
        assert_eq!(boolean.inputs[0].temporal, PortTemporal::Value);
    }
}
