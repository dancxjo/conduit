use super::{
    StandardConfigurationField, StandardConfigurationRule, StandardKindContract, TerminalBehavior,
};
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ConfigurationValue, ExecutionProfileId, HostOperationContractId, HostOperationRequirement,
    ImplementationId, KindContractRevision, PortDescriptor, PortDirection, PortTemporal,
    BOOL_INFO_ID, SCALAR_INFO_ID,
};

pub const LOGIC_COMPARE_KIND: &str = "logic/compare";
pub const LOGIC_NOT_KIND: &str = "logic/not";
pub const LOGIC_SELECT_KIND: &str = "logic/select";

pub const LOGIC_COMPARE_SCALAR_CONTRACT_REVISION: &str = "conduit.std/logic-compare-scalar@1";
pub const LOGIC_COMPARE_SCALAR_EXECUTION_PROFILE: &str =
    "conduit.std/logic-compare-scalar-kernel@1";
pub const LOGIC_COMPARE_SCALAR_IMPLEMENTATION: &str = "std/kernel-logic-compare-scalar@1";
pub const LOGIC_COMPARE_SCALAR_ARTIFACT: &str = "conduit-std-host/logic-compare-scalar@1";
pub const LOGIC_COMPARE_SCALAR_CAPABILITY: &str = "logic-compare-scalar-v1";

pub const LOGIC_NOT_CONTRACT_REVISION: &str = "conduit.std/logic-not@1";
pub const LOGIC_NOT_EXECUTION_PROFILE: &str = "conduit.std/logic-not-kernel@1";
pub const LOGIC_NOT_IMPLEMENTATION: &str = "std/kernel-logic-not@1";
pub const LOGIC_NOT_ARTIFACT: &str = "conduit-std-host/logic-not@1";
pub const LOGIC_NOT_CAPABILITY: &str = "logic-not-v1";
pub const CONDUITOS_LOGIC_NOT_CAPABILITY: &str = "conduitos/logic-not@1";
pub const CONDUITOS_LOGIC_NOT_EXECUTION_PROFILE: &str = "conduitos/functional-kernel@1";
pub const CONDUITOS_LOGIC_NOT_IMPLEMENTATION: &str = "conduitos/kernel-logic-not@1";
pub const CONDUITOS_LOGIC_NOT_ARTIFACT: &str = "conduitos/functional-kernel@1";
pub const CONDUITOS_LOGIC_NOT_HOST_OPERATION: &str = "conduit.host/logic-not@1";

pub const LOGIC_SELECT_SCALAR_CONTRACT_REVISION: &str = "conduit.std/logic-select-scalar@1";
pub const LOGIC_SELECT_SCALAR_EXECUTION_PROFILE: &str = "conduit.std/logic-select-scalar-kernel@1";
pub const LOGIC_SELECT_SCALAR_IMPLEMENTATION: &str = "std/kernel-logic-select-scalar@1";
pub const LOGIC_SELECT_SCALAR_ARTIFACT: &str = "conduit-std-host/logic-select-scalar@1";
pub const LOGIC_SELECT_SCALAR_CAPABILITY: &str = "logic-select-scalar-v1";

pub const COMPARE_LEFT_PORT: &str = "left";
pub const COMPARE_RIGHT_PORT: &str = "right";
pub const LOGIC_INPUT_PORT: &str = "in";
pub const LOGIC_OUTPUT_PORT: &str = "out";
pub const SELECT_SELECTOR_PORT: &str = "selector";
pub const SELECT_FALSE_PORT: &str = "when-false";
pub const SELECT_TRUE_PORT: &str = "when-true";
pub const COMPARE_OPERATOR_KEY: &str = "operator";
pub const COMPARISON_OPERATORS: [&str; 6] = ["lt", "le", "eq", "ne", "ge", "gt"];

pub fn logic_compare_scalar_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(LOGIC_COMPARE_KIND),
        plain_name: "Compare scalars".to_string(),
        summary: "Compare two exact scalar values with one finite configured operator.".to_string(),
        inputs: vec![
            value_port(COMPARE_LEFT_PORT, SCALAR_INFO_ID, PortDirection::Input),
            value_port(COMPARE_RIGHT_PORT, SCALAR_INFO_ID, PortDirection::Input),
        ],
        outputs: vec![value_port(
            LOGIC_OUTPUT_PORT,
            BOOL_INFO_ID,
            PortDirection::Output,
        )],
        configuration: vec![StandardConfigurationField {
            key: COMPARE_OPERATOR_KEY.to_string(),
            default_value: ConfigurationValue::Text("eq".to_string()),
            rule: StandardConfigurationRule::TextOneOf {
                values: comparison_operator_values(),
            },
        }],
        limits: limits(),
        terminal_behavior:
            TerminalBehavior::EmitsOneDecisionOrCompletesWhenDecisionBecomesImpossible,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "compare: logic/compare(operator = \"lt\")".to_string(),
    }
}

pub fn logic_not_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(LOGIC_NOT_KIND),
        plain_name: "Boolean not".to_string(),
        summary: "Invert one exact Boolean value without truthiness coercion.".to_string(),
        inputs: vec![value_port(
            LOGIC_INPUT_PORT,
            BOOL_INFO_ID,
            PortDirection::Input,
        )],
        outputs: vec![value_port(
            LOGIC_OUTPUT_PORT,
            BOOL_INFO_ID,
            PortDirection::Output,
        )],
        configuration: Vec::new(),
        limits: limits(),
        terminal_behavior:
            TerminalBehavior::EmitsOneDecisionOrCompletesWhenDecisionBecomesImpossible,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "invert: logic/not".to_string(),
    }
}

pub fn logic_select_scalar_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(LOGIC_SELECT_KIND),
        plain_name: "Select scalar".to_string(),
        summary: "Select one of two exact scalar values using one exact Boolean.".to_string(),
        inputs: vec![
            value_port(SELECT_SELECTOR_PORT, BOOL_INFO_ID, PortDirection::Input),
            value_port(SELECT_FALSE_PORT, SCALAR_INFO_ID, PortDirection::Input),
            value_port(SELECT_TRUE_PORT, SCALAR_INFO_ID, PortDirection::Input),
        ],
        outputs: vec![value_port(
            LOGIC_OUTPUT_PORT,
            SCALAR_INFO_ID,
            PortDirection::Output,
        )],
        configuration: Vec::new(),
        limits: limits(),
        terminal_behavior:
            TerminalBehavior::EmitsOneDecisionOrCompletesWhenDecisionBecomesImpossible,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "choice: logic/select".to_string(),
    }
}

pub fn logic_compare_scalar_offer() -> CapabilityOffer {
    offer(
        logic_compare_scalar_contract(),
        LOGIC_COMPARE_SCALAR_CAPABILITY,
        LOGIC_COMPARE_SCALAR_CONTRACT_REVISION,
        LOGIC_COMPARE_SCALAR_EXECUTION_PROFILE,
        LOGIC_COMPARE_SCALAR_IMPLEMENTATION,
        LOGIC_COMPARE_SCALAR_ARTIFACT,
    )
}

pub fn logic_not_offer() -> CapabilityOffer {
    offer(
        logic_not_contract(),
        LOGIC_NOT_CAPABILITY,
        LOGIC_NOT_CONTRACT_REVISION,
        LOGIC_NOT_EXECUTION_PROFILE,
        LOGIC_NOT_IMPLEMENTATION,
        LOGIC_NOT_ARTIFACT,
    )
}

pub fn conduitos_logic_not_offer() -> CapabilityOffer {
    let mut offer = logic_not_offer();
    offer.capability_id = CapabilityId::from(CONDUITOS_LOGIC_NOT_CAPABILITY);
    offer.implementation.execution_profile_id =
        ExecutionProfileId::from(CONDUITOS_LOGIC_NOT_EXECUTION_PROFILE);
    offer.implementation.implementation_id =
        ImplementationId::from(CONDUITOS_LOGIC_NOT_IMPLEMENTATION);
    offer.implementation.artifact_id = ArtifactId::from(CONDUITOS_LOGIC_NOT_ARTIFACT);
    offer.host_operations = vec![HostOperationRequirement {
        contract_id: HostOperationContractId::from(CONDUITOS_LOGIC_NOT_HOST_OPERATION),
        target_kind: Some(kind_id(LOGIC_NOT_KIND)),
        maximum_in_flight: 1,
        maximum_input_bytes: conduit_core::BOOL_ENCODED_LEN as u32,
        maximum_output_bytes: conduit_core::BOOL_ENCODED_LEN as u32,
    }];
    offer
}

pub fn logic_select_scalar_offer() -> CapabilityOffer {
    offer(
        logic_select_scalar_contract(),
        LOGIC_SELECT_SCALAR_CAPABILITY,
        LOGIC_SELECT_SCALAR_CONTRACT_REVISION,
        LOGIC_SELECT_SCALAR_EXECUTION_PROFILE,
        LOGIC_SELECT_SCALAR_IMPLEMENTATION,
        LOGIC_SELECT_SCALAR_ARTIFACT,
    )
}

fn offer(
    contract: StandardKindContract,
    capability: &str,
    revision: &str,
    profile: &str,
    implementation: &str,
    artifact: &str,
) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: super::functional_face::startup_face(&contract.configuration),
        shorthand: None,
        capability_id: CapabilityId::from(capability),
        kind_id: contract.kind_id,
        kind_contract_revision: KindContractRevision::from(revision),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(profile),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(artifact),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}

fn value_port(name: &str, info: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(info),
        direction,
        temporal: PortTemporal::Value,
    }
}

fn limits() -> CapabilityLimits {
    CapabilityLimits {
        max_active_instances: 16,
        max_queue_items: 1,
        max_queue_bytes: 8,
    }
}

fn comparison_operator_values() -> Vec<String> {
    COMPARISON_OPERATORS
        .iter()
        .map(|operator| (*operator).to_string())
        .collect()
}

#[cfg(feature = "form-catalog")]
pub fn install_logic_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    use conduit_form::{ConfigurationField, ConfigurationRule, KindDefinition, KindSignature};
    for (contract, revision) in [
        (
            logic_compare_scalar_contract(),
            LOGIC_COMPARE_SCALAR_CONTRACT_REVISION,
        ),
        (logic_not_contract(), LOGIC_NOT_CONTRACT_REVISION),
        (
            logic_select_scalar_contract(),
            LOGIC_SELECT_SCALAR_CONTRACT_REVISION,
        ),
    ] {
        startup.insert(KindSignature {
            kind: contract.kind_id.as_str().to_string(),
            startup_parameters: contract
                .configuration
                .iter()
                .map(|field| conduit_form::StartupParameterSignature {
                    name: field.key.clone(),
                    value_type: "Text".to_string(),
                    default: Some(match &field.default_value {
                        ConfigurationValue::Text(value) => value.clone(),
                        _ => unreachable!("logic configuration is finite text"),
                    }),
                })
                .collect(),
        })?;
        let configuration = contract
            .configuration
            .into_iter()
            .map(|field| ConfigurationField {
                key: field.key,
                default_value: field.default_value,
                validation: match field.rule {
                    StandardConfigurationRule::TextOneOf { values } => {
                        ConfigurationRule::TextOneOf { values }
                    }
                    _ => unreachable!("logic configuration is one finite text choice"),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logic_contracts_are_exact_one_shot_and_finite() {
        let compare = logic_compare_scalar_contract();
        assert_eq!(compare.inputs[0].value_kind.as_str(), SCALAR_INFO_ID);
        assert_eq!(compare.outputs[0].value_kind.as_str(), BOOL_INFO_ID);
        assert!(compare
            .inputs
            .iter()
            .chain(compare.outputs.iter())
            .all(|port| port.temporal == PortTemporal::Value));
        assert!(matches!(
            &compare.configuration[0].rule,
            StandardConfigurationRule::TextOneOf { values }
                if values == &comparison_operator_values()
        ));

        let not = logic_not_contract();
        assert_eq!(not.inputs[0].value_kind.as_str(), BOOL_INFO_ID);
        assert_eq!(not.outputs[0].value_kind.as_str(), BOOL_INFO_ID);

        let select = logic_select_scalar_contract();
        assert_eq!(select.inputs[0].value_kind.as_str(), BOOL_INFO_ID);
        assert!(select.inputs[1..]
            .iter()
            .chain(select.outputs.iter())
            .all(|port| port.value_kind.as_str() == SCALAR_INFO_ID));
    }
}
