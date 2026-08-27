//! Portable trigger and toggle semantics.
//!
//! This module defines platform-neutral meaning, exact capability advertisements,
//! and the profile-catalog extension used by the production kernel hosts. It does
//! not provide a timer-backed compatibility implementation: deliberate input must
//! be fulfilled through an admitted host-operation boundary.

use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    await_trigger_host_operation_requirement, kind_id, port_id, resource_requirement,
    ConfigurationEntry, ConfigurationValue, ExecutionProfileId, HostOperationRequirement,
    KindContractRevision, KindId, PortDescriptor, PortDirection, ResourceRequirement, ValuePayload,
    INPUT_RESOURCE_CLASS,
};
use serde::{Deserialize, Serialize};

use crate::MAX_SIGNAL_COUNT;

pub const TRIGGER_VALUE_KIND: &str = conduit_time::TICK_VALUE_KIND;
pub const TRIGGER_KIND: &str = "interaction/trigger";
pub use conduit_std_catalog::{
    BOOL_PRESENTATION_CONTRACT_REVISION as TOGGLE_PRESENTATION_CONTRACT_REVISION,
    BOOL_PRESENTATION_KIND as TOGGLE_PRESENTATION_KIND,
    STATE_TOGGLE_CONTRACT_REVISION as TOGGLE_CONTRACT_REVISION, STATE_TOGGLE_KIND as TOGGLE_KIND,
};

pub fn trigger_face_startup_parameters() -> Vec<conduit_core::FaceStartupParameter> {
    vec![conduit_core::FaceStartupParameter {
        name: "count".to_string(),
        value_type: "Count".to_string(),
        has_default: true,
    }]
}

pub fn toggle_face_startup_parameters() -> Vec<conduit_core::FaceStartupParameter> {
    vec![conduit_core::FaceStartupParameter {
        name: "initial".to_string(),
        value_type: "Boolean".to_string(),
        has_default: true,
    }]
}
pub const TRIGGER_PORT: &str = "trigger";
pub const TRIGGER_ENCODED_LEN: u32 = 8;
pub const TRIGGER_CONTRACT_REVISION: &str = "conduit.signal/interaction-trigger@1";
pub const TRIGGER_EXECUTION_PROFILE: &str = "conduit.signal/trigger-hosted@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trigger {
    pub sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TriggerConfiguration {
    pub count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToggleConfiguration {
    pub initial: bool,
}

pub fn trigger_kind() -> KindId {
    kind_id(TRIGGER_KIND)
}

pub fn toggle_kind() -> KindId {
    kind_id(TOGGLE_KIND)
}

pub fn trigger_value_kind() -> KindId {
    kind_id(TRIGGER_VALUE_KIND)
}

pub fn trigger_contract_revision() -> KindContractRevision {
    KindContractRevision::from(TRIGGER_CONTRACT_REVISION)
}

pub fn toggle_contract_revision() -> KindContractRevision {
    KindContractRevision::from(TOGGLE_CONTRACT_REVISION)
}

pub fn trigger_execution_profile() -> ExecutionProfileId {
    ExecutionProfileId::from(TRIGGER_EXECUTION_PROFILE)
}

pub fn trigger_host_operation_requirements() -> Vec<HostOperationRequirement> {
    vec![await_trigger_host_operation_requirement()]
}

pub fn toggle_host_operation_requirements() -> Vec<HostOperationRequirement> {
    Vec::new()
}

pub fn trigger_resource_requirements() -> Vec<ResourceRequirement> {
    vec![resource_requirement(INPUT_RESOURCE_CLASS, 1)]
}

pub fn toggle_resource_requirements() -> Vec<ResourceRequirement> {
    Vec::new()
}

pub fn trigger_outputs() -> Vec<PortDescriptor> {
    vec![PortDescriptor {
        port_id: port_id(TRIGGER_PORT),
        value_kind: trigger_value_kind(),
        direction: PortDirection::Output,
        temporal: conduit_core::PortTemporal::Flow { closes: true },
    }]
}

pub fn toggle_inputs() -> Vec<PortDescriptor> {
    conduit_std_catalog::state_toggle_contract().inputs
}

pub fn toggle_outputs() -> Vec<PortDescriptor> {
    conduit_std_catalog::state_toggle_contract().outputs
}

pub fn trigger_configuration_entries(config: &TriggerConfiguration) -> Vec<ConfigurationEntry> {
    vec![ConfigurationEntry {
        key: "count".to_string(),
        value: ConfigurationValue::U64(config.count),
    }]
}

pub fn parse_trigger_configuration(
    entries: &[ConfigurationEntry],
) -> Result<TriggerConfiguration, crate::SignalProfileError> {
    let mut count = None;
    for entry in entries {
        match (entry.key.as_str(), &entry.value) {
            ("count", ConfigurationValue::U64(value)) => count = Some(*value),
            ("count", _) => {
                return Err(crate::SignalProfileError::InvalidConfiguration("count"));
            }
            _ => {}
        }
    }
    let count = count.ok_or(crate::SignalProfileError::MissingConfiguration("count"))?;
    if count > MAX_SIGNAL_COUNT {
        return Err(crate::SignalProfileError::InvalidConfiguration("count"));
    }
    Ok(TriggerConfiguration { count })
}

pub fn toggle_configuration_entries(config: &ToggleConfiguration) -> Vec<ConfigurationEntry> {
    vec![ConfigurationEntry {
        key: "initial".to_string(),
        value: ConfigurationValue::Bool(config.initial),
    }]
}

pub fn parse_toggle_configuration(
    entries: &[ConfigurationEntry],
) -> Result<ToggleConfiguration, crate::SignalProfileError> {
    let mut initial = None;
    for entry in entries {
        match (entry.key.as_str(), &entry.value) {
            ("initial", ConfigurationValue::Bool(value)) => initial = Some(*value),
            ("initial", _) => {
                return Err(crate::SignalProfileError::InvalidConfiguration("initial"));
            }
            _ => {}
        }
    }
    Ok(ToggleConfiguration {
        initial: initial.ok_or(crate::SignalProfileError::MissingConfiguration("initial"))?,
    })
}

pub fn encode_trigger(trigger: &Trigger) -> ValuePayload {
    let mut encoded = Vec::with_capacity(TRIGGER_ENCODED_LEN as usize);
    encoded.extend_from_slice(&trigger.sequence.to_le_bytes());
    ValuePayload {
        value_kind: trigger_value_kind(),
        encoded,
    }
}

pub fn decode_trigger(payload: &ValuePayload) -> Result<Trigger, crate::SignalProfileError> {
    if payload.value_kind.as_str() != TRIGGER_VALUE_KIND {
        return Err(crate::SignalProfileError::WrongValueKind);
    }
    decode_trigger_bytes(&payload.encoded)
}

pub fn decode_trigger_bytes(encoded: &[u8]) -> Result<Trigger, crate::SignalProfileError> {
    if encoded.len() != TRIGGER_ENCODED_LEN as usize {
        return Err(crate::SignalProfileError::WrongEncodedLength(encoded.len()));
    }
    let mut sequence = [0u8; 8];
    sequence.copy_from_slice(encoded);
    Ok(Trigger {
        sequence: u64::from_le_bytes(sequence),
    })
}

#[cfg(feature = "host-profile")]
pub(crate) fn extend_profile_catalog(catalog: &mut conduit_form::ProfileCatalog) {
    use conduit_form::{ConfigurationField, ConfigurationRule, KindDefinition};

    catalog
        .insert(KindDefinition {
            kind_id: trigger_kind(),
            kind_contract_revision: trigger_contract_revision(),
            inputs: Vec::new(),
            outputs: trigger_outputs(),
            configuration: vec![ConfigurationField {
                key: "count".to_string(),
                default_value: ConfigurationValue::U64(16),
                validation: ConfigurationRule::U64Range {
                    minimum: 0,
                    maximum: MAX_SIGNAL_COUNT,
                },
            }],
        })
        .expect("signal profile kinds are unique");
    conduit_std_catalog::install_bool_presentation_catalog(catalog)
        .expect("toggle presentation kind is unique");
    catalog
        .insert(KindDefinition {
            kind_id: toggle_kind(),
            kind_contract_revision: toggle_contract_revision(),
            inputs: toggle_inputs(),
            outputs: toggle_outputs(),
            configuration: vec![ConfigurationField {
                key: "initial".to_string(),
                default_value: ConfigurationValue::Bool(false),
                validation: ConfigurationRule::Any,
            }],
        })
        .expect("signal profile kinds are unique");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_trigger_payload() {
        let trigger = Trigger { sequence: 42 };
        let payload = encode_trigger(&trigger);
        assert_eq!(payload.encoded.len(), TRIGGER_ENCODED_LEN as usize);
        assert_eq!(decode_trigger(&payload).unwrap(), trigger);
    }

    #[test]
    fn round_trips_trigger_configuration_entries() {
        let config = TriggerConfiguration { count: 5 };
        let parsed = parse_trigger_configuration(&trigger_configuration_entries(&config))
            .expect("trigger configuration should parse");
        assert_eq!(parsed, config);
    }

    #[test]
    fn round_trips_toggle_configuration_entries() {
        let config = ToggleConfiguration { initial: true };
        let parsed = parse_toggle_configuration(&toggle_configuration_entries(&config))
            .expect("toggle configuration should parse");
        assert_eq!(parsed, config);
    }
}
