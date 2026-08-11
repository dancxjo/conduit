use super::{
    StandardConfigurationField, StandardConfigurationRule, StandardKindContract, TerminalBehavior,
    TICK_VALUE_KIND, TIME_EVERY_COUNT,
};
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ConfigurationValue, ExecutionProfileId, ImplementationId, KindContractRevision, PortDescriptor,
    PortDirection, PortTemporal, BOOL_INFO_ID,
};

pub const STATE_TOGGLE_KIND: &str = "state/toggle";
pub const STATE_TOGGLE_CONTRACT_REVISION: &str = "conduit.std/state-toggle@1";
pub const STATE_TOGGLE_EXECUTION_PROFILE: &str = "conduit.std/state-toggle-kernel-hosted@1";
pub const STATE_TOGGLE_IMPLEMENTATION: &str = "std/kernel-state-toggle@1";
pub const STATE_TOGGLE_ARTIFACT: &str = "conduit-std-host/state-toggle@1";
pub const STATE_TOGGLE_CAPABILITY: &str = "state-toggle-v1";
pub const MAX_TOGGLE_VALUES: u64 = TIME_EVERY_COUNT + 1;

pub fn state_toggle_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(STATE_TOGGLE_KIND),
        plain_name: "Current Boolean toggle".to_string(),
        summary: "Emit one initial Boolean and invert it after each closing-flow Tick.".to_string(),
        inputs: vec![PortDescriptor {
            port_id: port_id("toggle"),
            value_kind: kind_id(TICK_VALUE_KIND),
            direction: PortDirection::Input,
            temporal: PortTemporal::Flow { closes: true },
        }],
        outputs: vec![PortDescriptor {
            port_id: port_id("value"),
            value_kind: kind_id(BOOL_INFO_ID),
            direction: PortDirection::Output,
            temporal: PortTemporal::Current,
        }],
        configuration: vec![StandardConfigurationField {
            key: "initial".to_string(),
            default_value: ConfigurationValue::Bool(false),
            rule: StandardConfigurationRule::Any,
        }],
        limits: CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: TIME_EVERY_COUNT as u16,
            max_queue_bytes: 64,
        },
        terminal_behavior: TerminalBehavior::EmitsInitialAndTogglesUntilInputCloses,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "toggle: state/toggle(false)".to_string(),
    }
}

pub fn state_toggle_offer() -> CapabilityOffer {
    let contract = state_toggle_contract();
    CapabilityOffer {
        startup_parameters: super::startup_face(&contract.configuration),
        shorthand: None,
        capability_id: CapabilityId::from(STATE_TOGGLE_CAPABILITY),
        kind_id: contract.kind_id,
        kind_contract_revision: KindContractRevision::from(STATE_TOGGLE_CONTRACT_REVISION),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(STATE_TOGGLE_EXECUTION_PROFILE),
            implementation_id: ImplementationId::from(STATE_TOGGLE_IMPLEMENTATION),
            artifact_id: ArtifactId::from(STATE_TOGGLE_ARTIFACT),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}

#[cfg(feature = "form-catalog")]
pub fn install_state_toggle_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    use conduit_form::{ConfigurationField, ConfigurationRule, KindDefinition, KindSignature};
    let contract = state_toggle_contract();
    startup.insert(KindSignature {
        kind: contract.kind_id.as_str().to_string(),
        startup_parameters: vec![conduit_form::StartupParameterSignature {
            name: "initial".to_string(),
            value_type: "Boolean".to_string(),
            default: Some("false".to_string()),
        }],
    })?;
    profile
        .insert(KindDefinition {
            kind_id: contract.kind_id,
            kind_contract_revision: KindContractRevision::from(STATE_TOGGLE_CONTRACT_REVISION),
            inputs: contract.inputs,
            outputs: contract.outputs,
            configuration: vec![ConfigurationField {
                key: "initial".to_string(),
                default_value: ConfigurationValue::Bool(false),
                validation: ConfigurationRule::Any,
            }],
        })
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_is_exact_tick_to_current_boolean() {
        let contract = state_toggle_contract();
        assert_eq!(contract.inputs[0].value_kind.as_str(), TICK_VALUE_KIND);
        assert_eq!(
            contract.inputs[0].temporal,
            PortTemporal::Flow { closes: true }
        );
        assert_eq!(contract.outputs[0].value_kind.as_str(), BOOL_INFO_ID);
        assert_eq!(contract.outputs[0].temporal, PortTemporal::Current);
        assert_eq!(
            contract.configuration[0].default_value,
            ConfigurationValue::Bool(false)
        );
    }
}
