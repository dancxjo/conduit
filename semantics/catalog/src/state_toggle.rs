use super::{
    StandardConfigurationField, StandardConfigurationRule, StandardKindContract, TerminalBehavior,
};
#[cfg(feature = "form-catalog")]
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec;
#[cfg(feature = "form-catalog")]
use conduit_core::KindContractRevision;
use conduit_core::{
    kind_id, port_id, CapabilityLimits, ConfigurationValue, PortDescriptor, PortDirection,
    PortTemporal, BOOL_INFO_ID,
};

pub const STATE_TOGGLE_KIND: &str = "state/toggle";
pub const STATE_TOGGLE_CONTRACT_REVISION: &str = "conduit.std/state-toggle@1";
pub const MAX_TOGGLE_VALUES: u64 = conduit_time::TIME_EVERY_COUNT + 1;

pub const fn bounded_toggle_value(initial: bool, index: u64) -> Option<bool> {
    if index < MAX_TOGGLE_VALUES {
        Some(if index.is_multiple_of(2) {
            initial
        } else {
            !initial
        })
    } else {
        None
    }
}

pub fn state_toggle_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(STATE_TOGGLE_KIND),
        plain_name: "Current Boolean toggle".to_string(),
        summary: "Emit one initial Boolean and invert it after each closing-flow Tick.".to_string(),
        inputs: vec![PortDescriptor {
            port_id: port_id("toggle"),
            value_kind: kind_id(conduit_time::TICK_VALUE_KIND),
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
            max_queue_items: conduit_time::TIME_EVERY_COUNT as u16,
            max_queue_bytes: 64,
        },
        terminal_behavior: TerminalBehavior::EmitsInitialAndTogglesUntilInputCloses,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "toggle: state/toggle(false)".to_string(),
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
        assert_eq!(
            contract.inputs[0].value_kind.as_str(),
            conduit_time::TICK_VALUE_KIND
        );
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
