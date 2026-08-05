//! Portable activation and toggle semantics.
//!
//! This module defines platform-neutral meaning, exact capability advertisements,
//! and the profile-catalog extension used by the production kernel hosts. It does
//! not provide a timer-backed compatibility implementation: deliberate input must
//! be fulfilled through an admitted host-operation boundary.

use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    await_activation_host_operation_requirement, kind_id, port_id, resource_offer,
    resource_requirement, ArtifactId, BootId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ConfigurationEntry, ConfigurationValue, ConnectionProvider, ConnectionProviderInstanceId,
    ExecutionProfileId, HostAdvertisement, HostId, HostOperationRequirement, HostProfileId,
    ImplementationId, KindContractRevision, KindId, LinkAuthorityReference, LinkAvailability,
    LinkBinding, LinkBindingId, LinkCredentialReference, LinkEndpoint, LinkEndpointId, LinkLimits,
    OfferGeneration, PortDescriptor, PortDirection, ResourceRequirement, ValuePayload,
    INPUT_RESOURCE_CLASS, PRESENTATION_RESOURCE_CLASS, PROTOCOL_VERSION,
};
use serde::{Deserialize, Serialize};

use crate::{
    show_contract_revision, show_execution_profile, show_host_operation_requirements, show_inputs,
    show_kind, show_resource_requirements, DISTRIBUTED_MAXIMUM_BUFFERED_BYTES,
    DISTRIBUTED_MAXIMUM_FRAME_BYTES, DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS, MAX_SIGNAL_COUNT,
    SIGNAL_ENCODED_LEN,
};

pub const ACTIVATION_VALUE_KIND: &str = "value/activation";
pub const ACTIVATE_KIND: &str = "interaction/activate";
pub const TOGGLE_KIND: &str = "state/toggle";
pub const ACTIVATE_PORT: &str = "activate";
pub const ACTIVATION_ENCODED_LEN: u32 = 8;
pub const ACTIVATE_CONTRACT_REVISION: &str = "conduit.signal/interaction-activate@1";
pub const TOGGLE_CONTRACT_REVISION: &str = "conduit.signal/state-toggle@1";
pub const ACTIVATE_EXECUTION_PROFILE: &str = "conduit.signal/activate-hosted@1";
pub const TOGGLE_EXECUTION_PROFILE: &str = "conduit.signal/toggle-hosted@1";
pub const DISTRIBUTED_TOGGLE_STD_HOST_ID: &str = "s4/toggle-std-source";
pub const DISTRIBUTED_TOGGLE_STD_BOOT_ID: &str = "s4/toggle-std-source-boot";
pub const DISTRIBUTED_TOGGLE_BROWSER_HOST_ID: &str = "s4/toggle-browser-sink";
pub const DISTRIBUTED_TOGGLE_BROWSER_BOOT_ID: &str = "s4/toggle-browser-sink-boot";
pub const DISTRIBUTED_TOGGLE_LINK_BINDING_ID: &str = "s4/toggle-std-browser-link";
pub const DISTRIBUTED_TOGGLE_PROVIDER_INSTANCE_ID: &str = "s4/toggle-websocket-loopback-instance";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Activation {
    pub sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivateConfiguration {
    pub count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToggleConfiguration {
    pub initial: bool,
}

pub fn activate_kind() -> KindId {
    kind_id(ACTIVATE_KIND)
}

pub fn toggle_kind() -> KindId {
    kind_id(TOGGLE_KIND)
}

pub fn activation_value_kind() -> KindId {
    kind_id(ACTIVATION_VALUE_KIND)
}

pub fn activate_contract_revision() -> KindContractRevision {
    KindContractRevision::from(ACTIVATE_CONTRACT_REVISION)
}

pub fn toggle_contract_revision() -> KindContractRevision {
    KindContractRevision::from(TOGGLE_CONTRACT_REVISION)
}

pub fn activate_execution_profile() -> ExecutionProfileId {
    ExecutionProfileId::from(ACTIVATE_EXECUTION_PROFILE)
}

pub fn toggle_execution_profile() -> ExecutionProfileId {
    ExecutionProfileId::from(TOGGLE_EXECUTION_PROFILE)
}

pub fn activate_host_operation_requirements() -> Vec<HostOperationRequirement> {
    vec![await_activation_host_operation_requirement()]
}

pub fn toggle_host_operation_requirements() -> Vec<HostOperationRequirement> {
    Vec::new()
}

pub fn activate_resource_requirements() -> Vec<ResourceRequirement> {
    vec![resource_requirement(INPUT_RESOURCE_CLASS, 1)]
}

pub fn toggle_resource_requirements() -> Vec<ResourceRequirement> {
    Vec::new()
}

pub fn activate_outputs() -> Vec<PortDescriptor> {
    vec![PortDescriptor {
        port_id: port_id(ACTIVATE_PORT),
        value_kind: activation_value_kind(),
        direction: PortDirection::Output,
    }]
}

pub fn toggle_inputs() -> Vec<PortDescriptor> {
    vec![PortDescriptor {
        port_id: port_id(ACTIVATE_PORT),
        value_kind: activation_value_kind(),
        direction: PortDirection::Input,
    }]
}

pub fn toggle_outputs() -> Vec<PortDescriptor> {
    vec![PortDescriptor {
        port_id: port_id(crate::SIGNAL_PORT),
        value_kind: crate::signal_value_kind(),
        direction: PortDirection::Output,
    }]
}

pub fn activate_configuration_entries(config: &ActivateConfiguration) -> Vec<ConfigurationEntry> {
    vec![ConfigurationEntry {
        key: "count".to_string(),
        value: ConfigurationValue::U64(config.count),
    }]
}

pub fn parse_activate_configuration(
    entries: &[ConfigurationEntry],
) -> Result<ActivateConfiguration, crate::SignalProfileError> {
    let mut count = None;
    for entry in entries {
        match (entry.key.as_str(), &entry.value) {
            ("count", ConfigurationValue::U64(value)) => count = Some(*value),
            ("count", _) => {
                return Err(crate::SignalProfileError::InvalidConfiguration(
                    entry.key.clone(),
                ));
            }
            _ => {}
        }
    }
    let count = count.ok_or(crate::SignalProfileError::MissingConfiguration("count"))?;
    if count > MAX_SIGNAL_COUNT {
        return Err(crate::SignalProfileError::InvalidConfiguration(
            "count".to_string(),
        ));
    }
    Ok(ActivateConfiguration { count })
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
                return Err(crate::SignalProfileError::InvalidConfiguration(
                    entry.key.clone(),
                ));
            }
            _ => {}
        }
    }
    Ok(ToggleConfiguration {
        initial: initial.ok_or(crate::SignalProfileError::MissingConfiguration("initial"))?,
    })
}

pub fn encode_activation(activation: &Activation) -> ValuePayload {
    let mut encoded = Vec::with_capacity(ACTIVATION_ENCODED_LEN as usize);
    encoded.extend_from_slice(&activation.sequence.to_le_bytes());
    ValuePayload {
        value_kind: activation_value_kind(),
        encoded,
    }
}

pub fn decode_activation(payload: &ValuePayload) -> Result<Activation, crate::SignalProfileError> {
    if payload.value_kind.as_str() != ACTIVATION_VALUE_KIND {
        return Err(crate::SignalProfileError::WrongValueKind(
            payload.value_kind.as_str().to_string(),
        ));
    }
    decode_activation_bytes(&payload.encoded)
}

pub fn decode_activation_bytes(
    encoded: &[u8],
) -> Result<Activation, crate::SignalProfileError> {
    if encoded.len() != ACTIVATION_ENCODED_LEN as usize {
        return Err(crate::SignalProfileError::WrongEncodedLength(encoded.len()));
    }
    let mut sequence = [0u8; 8];
    sequence.copy_from_slice(encoded);
    Ok(Activation {
        sequence: u64::from_le_bytes(sequence),
    })
}

pub fn distributed_toggle_std_source_advertisement() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(DISTRIBUTED_TOGGLE_STD_HOST_ID),
        boot_id: BootId::from(DISTRIBUTED_TOGGLE_STD_BOOT_ID),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("rust-std-kernel"),
        resources: vec![resource_offer(
            "s4/toggle-std-input",
            INPUT_RESOURCE_CLASS,
            1,
        )],
        capabilities: vec![
            CapabilityOffer {
                capability_id: CapabilityId::from("activate-1"),
                kind_id: activate_kind(),
                kind_contract_revision: activate_contract_revision(),
                execution_profile_id: activate_execution_profile(),
                implementation_id: ImplementationId::from("std/kernel-activate-v1"),
                artifact_id: ArtifactId::from("conduit-signal/activate-artifact-v1"),
                inputs: Vec::new(),
                outputs: activate_outputs(),
                host_operations: activate_host_operation_requirements(),
                resource_requirements: activate_resource_requirements(),
                authority_requirements: Vec::new(),
                limits: CapabilityLimits {
                    max_active_instances: 1,
                    max_queue_items: DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
                    max_queue_bytes: ACTIVATION_ENCODED_LEN,
                },
            },
            CapabilityOffer {
                capability_id: CapabilityId::from("toggle-1"),
                kind_id: toggle_kind(),
                kind_contract_revision: toggle_contract_revision(),
                execution_profile_id: toggle_execution_profile(),
                implementation_id: ImplementationId::from("std/kernel-toggle-v1"),
                artifact_id: ArtifactId::from("conduit-signal/toggle-artifact-v1"),
                inputs: toggle_inputs(),
                outputs: toggle_outputs(),
                host_operations: toggle_host_operation_requirements(),
                resource_requirements: toggle_resource_requirements(),
                authority_requirements: Vec::new(),
                limits: CapabilityLimits {
                    max_active_instances: 1,
                    max_queue_items: DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
                    max_queue_bytes: SIGNAL_ENCODED_LEN,
                },
            },
        ],
    }
}

pub fn distributed_toggle_browser_sink_advertisement() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(DISTRIBUTED_TOGGLE_BROWSER_HOST_ID),
        boot_id: BootId::from(DISTRIBUTED_TOGGLE_BROWSER_BOOT_ID),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("browser-wasm-kernel"),
        resources: vec![resource_offer(
            "s4/toggle-browser-dom",
            PRESENTATION_RESOURCE_CLASS,
            1,
        )],
        capabilities: vec![CapabilityOffer {
            capability_id: CapabilityId::from("toggle-dom-show-1"),
            kind_id: show_kind(),
            kind_contract_revision: show_contract_revision(),
            execution_profile_id: show_execution_profile(),
            implementation_id: ImplementationId::from("browser/kernel-dom-show-signal-v1"),
            artifact_id: ArtifactId::from("conduit-signal/show-artifact-v1"),
            inputs: show_inputs(),
            outputs: Vec::new(),
            host_operations: show_host_operation_requirements(),
            resource_requirements: show_resource_requirements(),
            authority_requirements: Vec::new(),
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
                max_queue_bytes: DISTRIBUTED_MAXIMUM_BUFFERED_BYTES,
            },
        }],
    }
}

pub fn distributed_toggle_websocket_link_binding() -> LinkBinding {
    LinkBinding {
        binding_id: LinkBindingId::from(DISTRIBUTED_TOGGLE_LINK_BINDING_ID),
        source: LinkEndpoint {
            host_id: HostId::from(DISTRIBUTED_TOGGLE_STD_HOST_ID),
            boot_id: BootId::from(DISTRIBUTED_TOGGLE_STD_BOOT_ID),
            endpoint_id: LinkEndpointId::from("s4/toggle-std-websocket-egress"),
        },
        sink: LinkEndpoint {
            host_id: HostId::from(DISTRIBUTED_TOGGLE_BROWSER_HOST_ID),
            boot_id: BootId::from(DISTRIBUTED_TOGGLE_BROWSER_BOOT_ID),
            endpoint_id: LinkEndpointId::from("s4/toggle-browser-websocket-ingress"),
        },
        provider: ConnectionProvider::WebSocket,
        provider_instance_id: ConnectionProviderInstanceId::from(
            DISTRIBUTED_TOGGLE_PROVIDER_INSTANCE_ID,
        ),
        availability: LinkAvailability::Ready,
        credential: LinkCredentialReference::None,
        authority: LinkAuthorityReference::ProcessOwned,
        limits: LinkLimits {
            maximum_in_flight_items: DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
            maximum_payload_bytes: SIGNAL_ENCODED_LEN,
            maximum_buffered_bytes: DISTRIBUTED_MAXIMUM_BUFFERED_BYTES,
            maximum_frame_bytes: DISTRIBUTED_MAXIMUM_FRAME_BYTES,
        },
    }
}

#[cfg(feature = "host-profile")]
pub(crate) fn extend_profile_catalog(catalog: &mut conduit_form::ProfileCatalog) {
    use conduit_form::{ConfigurationField, ConfigurationRule, KindDefinition};

    catalog
        .insert(KindDefinition {
            kind_id: activate_kind(),
            kind_contract_revision: activate_contract_revision(),
            inputs: Vec::new(),
            outputs: activate_outputs(),
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
    fn round_trips_activation_payload() {
        let activation = Activation { sequence: 42 };
        let payload = encode_activation(&activation);
        assert_eq!(payload.encoded.len(), ACTIVATION_ENCODED_LEN as usize);
        assert_eq!(decode_activation(&payload).unwrap(), activation);
    }

    #[test]
    fn round_trips_activate_configuration_entries() {
        let config = ActivateConfiguration { count: 5 };
        let parsed = parse_activate_configuration(&activate_configuration_entries(&config))
            .expect("activate configuration should parse");
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
