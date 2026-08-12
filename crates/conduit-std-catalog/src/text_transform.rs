use super::{
    StandardConfigurationField, StandardConfigurationRule, StandardKindContract, TerminalBehavior,
    MAX_TEXT_BYTES, TEXT_PRESENTATION_VALUE_KIND,
};
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ConfigurationValue, ExecutionProfileId, FaceStartupParameter, ImplementationId,
    KindContractRevision, PortDescriptor, PortDirection,
};

pub const TEXT_LITERAL_KIND: &str = "text/literal";
pub const TEXT_LITERAL_CONTRACT_REVISION: &str = "conduit.std/text-literal@1";
pub const TEXT_LITERAL_EXECUTION_PROFILE: &str = "conduit.std/text-literal-kernel-hosted@1";
pub const TEXT_LITERAL_IMPLEMENTATION: &str = "std/kernel-text-literal@1";
pub const TEXT_LITERAL_ARTIFACT: &str = "conduit-std-host/text-literal@1";
pub const TEXT_LITERAL_CAPABILITY: &str = "text-literal-v1";

pub const TEXT_UPPER_KIND: &str = "text/upper";
pub const TEXT_UPPER_CONTRACT_REVISION: &str = "conduit.std/text-upper@1";
pub const TEXT_UPPER_EXECUTION_PROFILE: &str = "conduit.std/text-upper-kernel-hosted@1";
pub const TEXT_UPPER_IMPLEMENTATION: &str = "std/kernel-text-upper@1";
pub const TEXT_UPPER_ARTIFACT: &str = "conduit-std-host/text-upper@1";
pub const TEXT_UPPER_CAPABILITY: &str = "text-upper-v1";
pub const TEXT_UPPER_HOST_OPERATION_CONTRACT: &str = "conduit.host/text-upper@1";
pub const TEXT_UPPER_HOST_OPERATION_TARGET: &str = "text/uppercase-utf8";

pub const TEXT_JOIN_KIND: &str = "text/join";
pub const TEXT_JOIN_CONTRACT_REVISION: &str = "conduit.std/text-join@1";
pub const TEXT_JOIN_EXECUTION_PROFILE: &str = "conduit.std/text-join-kernel-hosted@1";
pub const TEXT_JOIN_IMPLEMENTATION: &str = "std/kernel-text-join@1";
pub const TEXT_JOIN_ARTIFACT: &str = "conduit-std-host/text-join@1";
pub const TEXT_JOIN_CAPABILITY: &str = "text-join-v1";
pub const TEXT_JOIN_HOST_OPERATION_CONTRACT: &str = "conduit.host/text-join@1";
pub const TEXT_JOIN_HOST_OPERATION_TARGET: &str = "text/prefix-concat-utf8";
pub const CONDUITOS_BOUNDED_HOST_OP_PROFILE: &str = "conduitos/bounded-host-operations@1";
pub const CONDUITOS_BOUNDED_HOST_OP_ARTIFACT: &str = "conduitos/bounded-host-operations@1";
pub const CONDUITOS_TEXT_JOIN_CAPABILITY: &str = "conduitos-text-join-v1";
pub const CONDUITOS_TEXT_JOIN_IMPLEMENTATION: &str = "conduitos/kernel-text-join@1";

pub fn text_literal_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(TEXT_LITERAL_KIND),
        plain_name: "Text literal".to_string(),
        summary: "Emit one bounded immutable UTF-8 startup value.".to_string(),
        inputs: Vec::new(),
        outputs: vec![text_port("text", PortDirection::Output)],
        configuration: vec![StandardConfigurationField {
            key: "value".to_string(),
            default_value: ConfigurationValue::Text(String::new()),
            rule: StandardConfigurationRule::TextBytes {
                maximum: MAX_TEXT_BYTES,
            },
        }],
        limits: text_limits(),
        terminal_behavior: TerminalBehavior::EmitsOnce,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "\"Hello\" > presentation/text".to_string(),
    }
}

pub fn text_upper_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(TEXT_UPPER_KIND),
        plain_name: "Uppercase text".to_string(),
        summary: "Uppercase one bounded stream of UTF-8 text values.".to_string(),
        inputs: vec![text_port("text", PortDirection::Input)],
        outputs: vec![text_port("text", PortDirection::Output)],
        configuration: Vec::new(),
        limits: text_limits(),
        terminal_behavior: TerminalBehavior::MirrorsInputTerminal,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "upper: text/upper".to_string(),
    }
}

pub fn text_join_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(TEXT_JOIN_KIND),
        plain_name: "Prefix text".to_string(),
        summary: "Prepend one immutable bounded UTF-8 prefix without an implicit separator."
            .to_string(),
        inputs: vec![text_port("text", PortDirection::Input)],
        outputs: vec![text_port("text", PortDirection::Output)],
        configuration: vec![StandardConfigurationField {
            key: "prefix".to_string(),
            default_value: ConfigurationValue::Text(String::new()),
            rule: StandardConfigurationRule::TextBytes {
                maximum: MAX_TEXT_BYTES,
            },
        }],
        limits: text_limits(),
        terminal_behavior: TerminalBehavior::MirrorsInputTerminal,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "join: text/join(\"Hello\")".to_string(),
    }
}

pub fn text_literal_offer() -> CapabilityOffer {
    offer(
        &text_literal_contract(),
        TEXT_LITERAL_CAPABILITY,
        TEXT_LITERAL_CONTRACT_REVISION,
        TEXT_LITERAL_EXECUTION_PROFILE,
        TEXT_LITERAL_IMPLEMENTATION,
        TEXT_LITERAL_ARTIFACT,
        vec![FaceStartupParameter {
            name: "value".to_string(),
            value_type: "Text".to_string(),
            has_default: false,
        }],
        None,
    )
}

pub fn text_upper_offer() -> CapabilityOffer {
    let mut offer = offer(
        &text_upper_contract(),
        TEXT_UPPER_CAPABILITY,
        TEXT_UPPER_CONTRACT_REVISION,
        TEXT_UPPER_EXECUTION_PROFILE,
        TEXT_UPPER_IMPLEMENTATION,
        TEXT_UPPER_ARTIFACT,
        Vec::new(),
        Some((port_id("text"), port_id("text"))),
    );
    offer
        .host_operations
        .push(conduit_core::HostOperationRequirement {
            contract_id: conduit_core::HostOperationContractId::from(
                TEXT_UPPER_HOST_OPERATION_CONTRACT,
            ),
            target_kind: Some(kind_id(TEXT_UPPER_HOST_OPERATION_TARGET)),
            maximum_in_flight: 1,
            maximum_input_bytes: MAX_TEXT_BYTES,
            maximum_output_bytes: MAX_TEXT_BYTES,
        });
    offer
}

pub fn text_join_offer() -> CapabilityOffer {
    let mut offer = offer(
        &text_join_contract(),
        TEXT_JOIN_CAPABILITY,
        TEXT_JOIN_CONTRACT_REVISION,
        TEXT_JOIN_EXECUTION_PROFILE,
        TEXT_JOIN_IMPLEMENTATION,
        TEXT_JOIN_ARTIFACT,
        vec![FaceStartupParameter {
            name: "prefix".to_string(),
            value_type: "Text".to_string(),
            has_default: false,
        }],
        Some((port_id("text"), port_id("text"))),
    );
    offer
        .host_operations
        .push(conduit_core::HostOperationRequirement {
            contract_id: conduit_core::HostOperationContractId::from(
                TEXT_JOIN_HOST_OPERATION_CONTRACT,
            ),
            target_kind: Some(kind_id(TEXT_JOIN_HOST_OPERATION_TARGET)),
            maximum_in_flight: 1,
            maximum_input_bytes: MAX_TEXT_BYTES,
            maximum_output_bytes: MAX_TEXT_BYTES,
        });
    offer
}

pub fn conduitos_text_join_offer() -> CapabilityOffer {
    conduitos_bounded_host_operation_offer(
        text_join_offer(),
        CONDUITOS_TEXT_JOIN_CAPABILITY,
        CONDUITOS_TEXT_JOIN_IMPLEMENTATION,
    )
}

pub(crate) fn conduitos_bounded_host_operation_offer(
    mut offer: CapabilityOffer,
    capability: &str,
    implementation: &str,
) -> CapabilityOffer {
    offer.capability_id = CapabilityId::from(capability);
    offer.implementation.execution_profile_id =
        ExecutionProfileId::from(CONDUITOS_BOUNDED_HOST_OP_PROFILE);
    offer.implementation.implementation_id = ImplementationId::from(implementation);
    offer.implementation.artifact_id = ArtifactId::from(CONDUITOS_BOUNDED_HOST_OP_ARTIFACT);
    offer
}

#[cfg(feature = "form-catalog")]
pub fn install_text_pipeline_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    use conduit_form::{
        ConfigurationField, ConfigurationRule, KindDefinition, KindSignature,
        StartupParameterSignature,
    };
    startup.insert(KindSignature {
        kind: TEXT_LITERAL_KIND.to_string(),
        startup_parameters: vec![StartupParameterSignature {
            name: "value".to_string(),
            value_type: "Text".to_string(),
            default: None,
        }],
    })?;
    startup.insert(KindSignature {
        kind: TEXT_UPPER_KIND.to_string(),
        startup_parameters: Vec::new(),
    })?;
    startup.insert(KindSignature {
        kind: TEXT_JOIN_KIND.to_string(),
        startup_parameters: vec![StartupParameterSignature {
            name: "prefix".to_string(),
            value_type: "Text".to_string(),
            default: None,
        }],
    })?;
    startup.insert(KindSignature {
        kind: super::TEXT_PRESENTATION_KIND.to_string(),
        startup_parameters: vec![StartupParameterSignature {
            name: "maximum-values".to_string(),
            value_type: "Count".to_string(),
            default: Some(super::MAX_TEXT_VALUES.to_string()),
        }],
    })?;
    for (contract, revision) in [
        (text_literal_contract(), TEXT_LITERAL_CONTRACT_REVISION),
        (text_upper_contract(), TEXT_UPPER_CONTRACT_REVISION),
        (text_join_contract(), TEXT_JOIN_CONTRACT_REVISION),
    ] {
        let configuration = contract
            .configuration
            .into_iter()
            .map(|field| ConfigurationField {
                key: field.key,
                default_value: field.default_value,
                validation: match field.rule {
                    StandardConfigurationRule::Any => ConfigurationRule::Any,
                    StandardConfigurationRule::U64Range { minimum, maximum } => {
                        ConfigurationRule::U64Range { minimum, maximum }
                    }
                    StandardConfigurationRule::I64Range { minimum, maximum } => {
                        ConfigurationRule::I64Range { minimum, maximum }
                    }
                    StandardConfigurationRule::DurationMillis { minimum, maximum } => {
                        ConfigurationRule::DurationMillis { minimum, maximum }
                    }
                    StandardConfigurationRule::TextBytes { maximum } => {
                        ConfigurationRule::TextBytes { maximum }
                    }
                    StandardConfigurationRule::TextOneOf { values } => {
                        ConfigurationRule::TextOneOf { values }
                    }
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
    let presentation = super::text_presentation_contract();
    profile
        .insert(KindDefinition {
            kind_id: presentation.kind_id,
            kind_contract_revision: KindContractRevision::from(
                super::TEXT_PRESENTATION_CONTRACT_REVISION,
            ),
            inputs: presentation.inputs,
            outputs: presentation.outputs,
            configuration: vec![ConfigurationField {
                key: "maximum-values".to_string(),
                default_value: ConfigurationValue::U64(super::MAX_TEXT_VALUES),
                validation: ConfigurationRule::U64Range {
                    minimum: 1,
                    maximum: super::MAX_TEXT_VALUES,
                },
            }],
        })
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn text_port(name: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(TEXT_PRESENTATION_VALUE_KIND),
        direction,
        temporal: conduit_core::PortTemporal::Value,
    }
}

fn text_limits() -> CapabilityLimits {
    CapabilityLimits {
        max_active_instances: 16,
        max_queue_items: 4,
        max_queue_bytes: MAX_TEXT_BYTES,
    }
}

#[allow(clippy::too_many_arguments)]
fn offer(
    contract: &StandardKindContract,
    capability: &str,
    revision: &str,
    profile: &str,
    implementation: &str,
    artifact: &str,
    startup_parameters: Vec<FaceStartupParameter>,
    shorthand: Option<(conduit_core::PortId, conduit_core::PortId)>,
) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters,
        shorthand,
        capability_id: CapabilityId::from(capability),
        kind_id: contract.kind_id.clone(),
        kind_contract_revision: KindContractRevision::from(revision),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(profile),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(artifact),
        },
        inputs: contract.inputs.clone(),
        outputs: contract.outputs.clone(),
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: contract.limits.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_pipeline_contracts_are_typed_bounded_and_hosted_only() {
        let literal = text_literal_offer();
        let upper = text_upper_offer();
        let join = text_join_offer();
        assert_eq!(literal.outputs[0].value_kind, upper.inputs[0].value_kind);
        assert_eq!(upper.inputs[0].value_kind, upper.outputs[0].value_kind);
        assert_eq!(join.inputs[0].value_kind, join.outputs[0].value_kind);
        assert_eq!(join.startup_parameters[0].value_type, "Text");
        assert_eq!(literal.startup_parameters[0].value_type, "Text");
        assert!(!literal.startup_parameters[0].has_default);
        assert!(!text_upper_contract().browser_manifestation_honest);
        assert!(!text_upper_contract().pico_manifestation_honest);
    }
}
