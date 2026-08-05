#![cfg_attr(not(feature = "host-profile"), no_std)]

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    await_activation_host_operation_requirement, kind_id, port_id,
    present_host_operation_requirement, resource_offer, resource_requirement,
    wait_host_operation_requirement, ArtifactId, BootId, CapabilityId, CapabilityLimits,
    CapabilityOffer, ConfigurationEntry, ConfigurationValue, ConnectionProvider,
    ConnectionProviderInstanceId, ExecutionProfileId, HostAdvertisement, HostId,
    HostOperationRequirement, HostProfileId, ImplementationId, KindContractRevision, KindId,
    LinkAuthorityReference, LinkAvailability, LinkBinding, LinkBindingId, LinkCredentialReference,
    LinkEndpoint, LinkEndpointId, LinkLimits, OfferGeneration, PortDescriptor, PortDirection,
    ResourceOffer, ResourceRequirement, ValuePayload, INPUT_RESOURCE_CLASS,
    PRESENTATION_RESOURCE_CLASS, PROTOCOL_VERSION, TIMER_RESOURCE_CLASS,
};
use serde::{Deserialize, Serialize};

pub const SIGNAL_VALUE_KIND: &str = "value/signal";
pub const ACTIVATION_VALUE_KIND: &str = "value/activation";
pub const PULSE_KIND: &str = "flow/pulse";
pub const SHOW_KIND: &str = "presentation/show";
pub const ACTIVATE_KIND: &str = "interaction/activate";
pub const TOGGLE_KIND: &str = "state/toggle";
pub const SIGNAL_PORT: &str = "signal";
pub const ACTIVATE_PORT: &str = "activate";
pub const SIGNAL_ENCODED_LEN: u32 = 9;
pub const ACTIVATION_ENCODED_LEN: u32 = 8;
pub const SIGNAL_PRESENTATION_KIND: &str = "presentation/signal";
pub const MAX_SIGNAL_COUNT: u64 = 4_096;
pub const PULSE_CONTRACT_REVISION: &str = "conduit.signal/flow-pulse@1";
pub const SHOW_CONTRACT_REVISION: &str = "conduit.signal/presentation-show@1";
pub const ACTIVATE_CONTRACT_REVISION: &str = "conduit.signal/interaction-activate@1";
pub const TOGGLE_CONTRACT_REVISION: &str = "conduit.signal/state-toggle@1";
pub const PULSE_EXECUTION_PROFILE: &str = "conduit.signal/pulse-hosted@1";
pub const SHOW_EXECUTION_PROFILE: &str = "conduit.signal/show-hosted@1";
pub const ACTIVATE_EXECUTION_PROFILE: &str = "conduit.signal/activate-hosted@1";
pub const TOGGLE_EXECUTION_PROFILE: &str = "conduit.signal/toggle-hosted@1";
pub const DISTRIBUTED_STD_HOST_ID: &str = "s4/std-source";
pub const DISTRIBUTED_STD_BOOT_ID: &str = "s4/std-source-boot";
pub const DISTRIBUTED_BROWSER_HOST_ID: &str = "s4/browser-sink";
pub const DISTRIBUTED_BROWSER_BOOT_ID: &str = "s4/browser-sink-boot";
pub const DISTRIBUTED_LINK_BINDING_ID: &str = "s4/std-browser-link";
pub const DISTRIBUTED_PROVIDER_INSTANCE_ID: &str = "s4/websocket-loopback-instance";
pub const DISTRIBUTED_TOGGLE_STD_HOST_ID: &str = "s4/toggle-std-source";
pub const DISTRIBUTED_TOGGLE_STD_BOOT_ID: &str = "s4/toggle-std-source-boot";
pub const DISTRIBUTED_TOGGLE_BROWSER_HOST_ID: &str = "s4/toggle-browser-sink";
pub const DISTRIBUTED_TOGGLE_BROWSER_BOOT_ID: &str = "s4/toggle-browser-sink-boot";
pub const DISTRIBUTED_TOGGLE_LINK_BINDING_ID: &str = "s4/toggle-std-browser-link";
pub const DISTRIBUTED_TOGGLE_PROVIDER_INSTANCE_ID: &str = "s4/toggle-websocket-loopback-instance";
pub const DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS: u16 = 1;
pub const DISTRIBUTED_MAXIMUM_BUFFERED_BYTES: u32 = SIGNAL_ENCODED_LEN;
pub const DISTRIBUTED_MAXIMUM_FRAME_BYTES: u32 = 2_048;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signal {
    pub sequence: u64,
    pub level: bool,
}

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PulseConfiguration {
    pub count: u64,
    pub period_ms: u64,
    pub initial_level: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignalProfileError {
    MissingConfiguration(&'static str),
    InvalidConfiguration(String),
    WrongValueKind(String),
    WrongEncodedLength(usize),
}

impl core::fmt::Display for SignalProfileError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SignalProfileError::MissingConfiguration(key) => {
                write!(f, "missing configuration '{key}'")
            }
            SignalProfileError::InvalidConfiguration(key) => {
                write!(f, "invalid configuration '{key}'")
            }
            SignalProfileError::WrongValueKind(kind) => {
                write!(f, "wrong value kind '{kind}'")
            }
            SignalProfileError::WrongEncodedLength(length) => {
                write!(f, "wrong encoded signal length {length}")
            }
        }
    }
}

pub fn pulse_kind() -> KindId {
    kind_id(PULSE_KIND)
}

pub fn show_kind() -> KindId {
    kind_id(SHOW_KIND)
}

pub fn signal_value_kind() -> KindId {
    kind_id(SIGNAL_VALUE_KIND)
}

pub fn pulse_contract_revision() -> KindContractRevision {
    KindContractRevision::from(PULSE_CONTRACT_REVISION)
}

pub fn show_contract_revision() -> KindContractRevision {
    KindContractRevision::from(SHOW_CONTRACT_REVISION)
}

pub fn pulse_execution_profile() -> ExecutionProfileId {
    ExecutionProfileId::from(PULSE_EXECUTION_PROFILE)
}

pub fn show_execution_profile() -> ExecutionProfileId {
    ExecutionProfileId::from(SHOW_EXECUTION_PROFILE)
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

pub fn pulse_host_operation_requirements() -> Vec<HostOperationRequirement> {
    vec![wait_host_operation_requirement()]
}

pub fn show_host_operation_requirements() -> Vec<HostOperationRequirement> {
    vec![present_host_operation_requirement(
        kind_id(SIGNAL_PRESENTATION_KIND),
        SIGNAL_ENCODED_LEN,
    )]
}

pub fn pulse_resource_requirements() -> Vec<ResourceRequirement> {
    vec![resource_requirement(TIMER_RESOURCE_CLASS, 1)]
}

pub fn show_resource_requirements() -> Vec<ResourceRequirement> {
    vec![resource_requirement(PRESENTATION_RESOURCE_CLASS, 1)]
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
        port_id: port_id(SIGNAL_PORT),
        value_kind: signal_value_kind(),
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
) -> Result<ActivateConfiguration, SignalProfileError> {
    let mut count = None;
    for entry in entries {
        match (entry.key.as_str(), &entry.value) {
            ("count", ConfigurationValue::U64(value)) => count = Some(*value),
            ("count", _) => {
                return Err(SignalProfileError::InvalidConfiguration(entry.key.clone()));
            }
            _ => {}
        }
    }
    let count = count.ok_or(SignalProfileError::MissingConfiguration("count"))?;
    if count > MAX_SIGNAL_COUNT {
        return Err(SignalProfileError::InvalidConfiguration("count".to_string()));
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
) -> Result<ToggleConfiguration, SignalProfileError> {
    let mut initial = None;
    for entry in entries {
        match (entry.key.as_str(), &entry.value) {
            ("initial", ConfigurationValue::Bool(value)) => initial = Some(*value),
            ("initial", _) => {
                return Err(SignalProfileError::InvalidConfiguration(entry.key.clone()));
            }
            _ => {}
        }
    }
    Ok(ToggleConfiguration {
        initial: initial.ok_or(SignalProfileError::MissingConfiguration("initial"))?,
    })
}

pub fn signal_resource_offers(
    timer_pool_id: &str,
    presentation_pool_id: &str,
    capacity_units: u32,
) -> Vec<ResourceOffer> {
    let mut offers = vec![
        resource_offer(timer_pool_id, TIMER_RESOURCE_CLASS, capacity_units),
        resource_offer(
            presentation_pool_id,
            PRESENTATION_RESOURCE_CLASS,
            capacity_units,
        ),
    ];
    offers.sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
    offers
}

pub fn pulse_outputs() -> Vec<PortDescriptor> {
    vec![PortDescriptor {
        port_id: port_id(SIGNAL_PORT),
        value_kind: signal_value_kind(),
        direction: PortDirection::Output,
    }]
}

pub fn show_inputs() -> Vec<PortDescriptor> {
    vec![PortDescriptor {
        port_id: port_id(SIGNAL_PORT),
        value_kind: signal_value_kind(),
        direction: PortDirection::Input,
    }]
}

pub fn pulse_configuration_entries(config: &PulseConfiguration) -> Vec<ConfigurationEntry> {
    vec![
        ConfigurationEntry {
            key: "count".to_string(),
            value: ConfigurationValue::U64(config.count),
        },
        ConfigurationEntry {
            key: "period-ms".to_string(),
            value: ConfigurationValue::U64(config.period_ms),
        },
        ConfigurationEntry {
            key: "initial".to_string(),
            value: ConfigurationValue::Bool(config.initial_level),
        },
    ]
}

pub fn parse_pulse_configuration(
    entries: &[ConfigurationEntry],
) -> Result<PulseConfiguration, SignalProfileError> {
    let mut count = None;
    let mut period_ms = None;
    let mut initial_level = None;
    for entry in entries {
        match (entry.key.as_str(), &entry.value) {
            ("count", ConfigurationValue::U64(value)) => count = Some(*value),
            ("period-ms", ConfigurationValue::U64(value)) => period_ms = Some(*value),
            ("initial", ConfigurationValue::Bool(value)) => initial_level = Some(*value),
            ("count", _) | ("period-ms", _) | ("initial", _) => {
                return Err(SignalProfileError::InvalidConfiguration(entry.key.clone()));
            }
            _ => {}
        }
    }
    let count = count.ok_or(SignalProfileError::MissingConfiguration("count"))?;
    if count > MAX_SIGNAL_COUNT {
        return Err(SignalProfileError::InvalidConfiguration(
            "count".to_string(),
        ));
    }
    Ok(PulseConfiguration {
        count,
        period_ms: period_ms.ok_or(SignalProfileError::MissingConfiguration("period-ms"))?,
        initial_level: initial_level.ok_or(SignalProfileError::MissingConfiguration("initial"))?,
    })
}

pub fn encode_signal(signal: &Signal) -> ValuePayload {
    let mut encoded = Vec::with_capacity(SIGNAL_ENCODED_LEN as usize);
    encoded.extend_from_slice(&signal.sequence.to_le_bytes());
    encoded.push(u8::from(signal.level));
    ValuePayload {
        value_kind: signal_value_kind(),
        encoded,
    }
}

pub fn decode_signal(payload: &ValuePayload) -> Result<Signal, SignalProfileError> {
    if payload.value_kind.as_str() != SIGNAL_VALUE_KIND {
        return Err(SignalProfileError::WrongValueKind(
            payload.value_kind.as_str().to_string(),
        ));
    }
    decode_signal_bytes(&payload.encoded)
}

pub fn decode_signal_bytes(encoded: &[u8]) -> Result<Signal, SignalProfileError> {
    if encoded.len() != SIGNAL_ENCODED_LEN as usize {
        return Err(SignalProfileError::WrongEncodedLength(encoded.len()));
    }
    let mut sequence = [0u8; 8];
    sequence.copy_from_slice(&encoded[..8]);
    Ok(Signal {
        sequence: u64::from_le_bytes(sequence),
        level: encoded[8] != 0,
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

pub fn decode_activation_bytes(encoded: &[u8]) -> Result<Activation, SignalProfileError> {
    if encoded.len() != ACTIVATION_ENCODED_LEN as usize {
        return Err(SignalProfileError::WrongEncodedLength(encoded.len()));
    }
    let mut sequence = [0u8; 8];
    sequence.copy_from_slice(&encoded[..8]);
    Ok(Activation {
        sequence: u64::from_le_bytes(sequence),
    })
}

pub fn signal_payload_size() -> u32 {
    SIGNAL_ENCODED_LEN
}

/// Exact production host facts used by the live S4 std-to-browser checkpoint.
/// The ephemeral loopback URL is carrier configuration and is deliberately not
/// part of these semantic or plan-visible identities.
pub fn distributed_std_source_advertisement() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(DISTRIBUTED_STD_HOST_ID),
        boot_id: BootId::from(DISTRIBUTED_STD_BOOT_ID),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("rust-std-kernel"),
        resources: vec![resource_offer("s4/std-timer", TIMER_RESOURCE_CLASS, 1)],
        capabilities: vec![CapabilityOffer {
            capability_id: CapabilityId::from("pulse-1"),
            kind_id: pulse_kind(),
            kind_contract_revision: pulse_contract_revision(),
            execution_profile_id: pulse_execution_profile(),
            implementation_id: ImplementationId::from("std/kernel-pulse-v1"),
            artifact_id: ArtifactId::from("conduit-signal/pulse-artifact-v1"),
            inputs: Vec::new(),
            outputs: pulse_outputs(),
            host_operations: pulse_host_operation_requirements(),
            resource_requirements: pulse_resource_requirements(),
            authority_requirements: Vec::new(),
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
                max_queue_bytes: DISTRIBUTED_MAXIMUM_BUFFERED_BYTES,
            },
        }],
    }
}

pub fn distributed_browser_sink_advertisement() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(DISTRIBUTED_BROWSER_HOST_ID),
        boot_id: BootId::from(DISTRIBUTED_BROWSER_BOOT_ID),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("browser-wasm-kernel"),
        resources: vec![resource_offer(
            "s4/browser-dom",
            PRESENTATION_RESOURCE_CLASS,
            1,
        )],
        capabilities: vec![CapabilityOffer {
            capability_id: CapabilityId::from("dom-show-1"),
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

pub fn distributed_websocket_link_binding() -> LinkBinding {
    LinkBinding {
        binding_id: LinkBindingId::from(DISTRIBUTED_LINK_BINDING_ID),
        source: LinkEndpoint {
            host_id: HostId::from(DISTRIBUTED_STD_HOST_ID),
            boot_id: BootId::from(DISTRIBUTED_STD_BOOT_ID),
            endpoint_id: LinkEndpointId::from("s4/std-websocket-egress"),
        },
        sink: LinkEndpoint {
            host_id: HostId::from(DISTRIBUTED_BROWSER_HOST_ID),
            boot_id: BootId::from(DISTRIBUTED_BROWSER_BOOT_ID),
            endpoint_id: LinkEndpointId::from("s4/browser-websocket-ingress"),
        },
        provider: ConnectionProvider::WebSocket,
        provider_instance_id: ConnectionProviderInstanceId::from(DISTRIBUTED_PROVIDER_INSTANCE_ID),
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

pub fn distributed_toggle_std_source_advertisement() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(DISTRIBUTED_TOGGLE_STD_HOST_ID),
        boot_id: BootId::from(DISTRIBUTED_TOGGLE_STD_BOOT_ID),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("rust-std-kernel"),
        resources: vec![resource_offer("s4/toggle-std-input", INPUT_RESOURCE_CLASS, 1)],
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
                    max_queue_bytes: DISTRIBUTED_MAXIMUM_BUFFERED_BYTES,
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
mod host_profile {
    use super::{
        activate_contract_revision, activate_execution_profile, activate_host_operation_requirements,
        activate_kind, activate_outputs, activate_resource_requirements,
        activation_value_kind, decode_activation_bytes, decode_signal, encode_activation,
        encode_signal, parse_activate_configuration, parse_pulse_configuration,
        parse_toggle_configuration, pulse_contract_revision, pulse_execution_profile,
        pulse_host_operation_requirements, pulse_kind, pulse_outputs, pulse_resource_requirements,
        show_contract_revision, show_execution_profile, show_host_operation_requirements,
        show_inputs, show_kind, show_resource_requirements, signal_value_kind,
        toggle_contract_revision, toggle_execution_profile, toggle_host_operation_requirements,
        toggle_inputs, toggle_kind, toggle_outputs, toggle_resource_requirements,
        ActivateConfiguration, Activation, PulseConfiguration, Signal, MAX_SIGNAL_COUNT,
        ACTIVATE_PORT, ACTIVATION_ENCODED_LEN, SIGNAL_ENCODED_LEN, SIGNAL_PORT,
        SIGNAL_PRESENTATION_KIND,
    };
    use alloc::boxed::Box;
    use conduit_core::{
        kind_id, port_id, ArtifactId, ConfigurationValue, FailureReason, ImplementationId, KindId,
        PlannedOperation,
    };
    use conduit_form::{ConfigurationField, ConfigurationRule, KindDefinition, ProfileCatalog};
    use conduit_runtime::{
        ImplementationFailure, ImplementationRegistry, OperationAction, OperationCompletion,
        OperationImplementation, OperationOutput, OperationState,
    };

    pub struct PulseImplementation {
        kind_id: KindId,
        implementation_id: ImplementationId,
        artifact_id: ArtifactId,
    }

    impl PulseImplementation {
        pub fn new(implementation_id: ImplementationId) -> Self {
            Self {
                kind_id: pulse_kind(),
                implementation_id,
                artifact_id: ArtifactId::from("conduit-signal/pulse-artifact-v1"),
            }
        }
    }

    impl OperationImplementation for PulseImplementation {
        fn kind_id(&self) -> &KindId {
            &self.kind_id
        }

        fn kind_contract_revision(&self) -> conduit_core::KindContractRevision {
            pulse_contract_revision()
        }

        fn execution_profile_id(&self) -> conduit_core::ExecutionProfileId {
            pulse_execution_profile()
        }

        fn implementation_id(&self) -> &ImplementationId {
            &self.implementation_id
        }

        fn artifact_id(&self) -> &ArtifactId {
            &self.artifact_id
        }

        fn host_operation_requirements(&self) -> Vec<conduit_core::HostOperationRequirement> {
            pulse_host_operation_requirements()
        }

        fn resource_requirements(&self) -> Vec<conduit_core::ResourceRequirement> {
            pulse_resource_requirements()
        }

        fn prepare(
            &self,
            placement: &PlannedOperation,
        ) -> Result<Box<dyn OperationState>, ImplementationFailure> {
            let configuration =
                parse_pulse_configuration(&placement.configuration).map_err(|err| {
                    ImplementationFailure::new(
                        FailureReason::InvalidOperationConfiguration,
                        err.to_string(),
                    )
                })?;
            Ok(Box::new(PulseState {
                configuration,
                next_sequence: 0,
            }))
        }

        fn minimum_value_size(&self, value_kind: &KindId) -> Option<u32> {
            (value_kind == &signal_value_kind()).then_some(SIGNAL_ENCODED_LEN)
        }
    }

    struct PulseState {
        configuration: PulseConfiguration,
        next_sequence: u64,
    }

    impl PulseState {
        fn next_emit_or_complete(&self) -> OperationAction {
            if self.next_sequence >= self.configuration.count {
                OperationAction::Complete
            } else {
                OperationAction::Emit(vec![OperationOutput {
                    port: port_id(SIGNAL_PORT),
                    value: encode_signal(&Signal {
                        sequence: self.next_sequence,
                        level: if self.next_sequence.is_multiple_of(2) {
                            self.configuration.initial_level
                        } else {
                            !self.configuration.initial_level
                        },
                    }),
                }])
            }
        }
    }

    impl OperationState for PulseState {
        fn start(&mut self) -> OperationAction {
            self.next_emit_or_complete()
        }

        fn resume(&mut self, completion: OperationCompletion) -> OperationAction {
            match completion {
                OperationCompletion::Emitted => {
                    self.next_sequence += 1;
                    if self.next_sequence >= self.configuration.count {
                        OperationAction::Complete
                    } else if self.configuration.period_ms > 0 {
                        OperationAction::Wait {
                            duration_ms: self.configuration.period_ms,
                        }
                    } else {
                        self.next_emit_or_complete()
                    }
                }
                OperationCompletion::TimerElapsed => self.next_emit_or_complete(),
                _ => OperationAction::Fail(ImplementationFailure::new(
                    FailureReason::InvalidLifecycleCommand,
                    "pulse received an incompatible runtime completion",
                )),
            }
        }
    }

    pub struct ShowImplementation {
        kind_id: KindId,
        implementation_id: ImplementationId,
        artifact_id: ArtifactId,
    }

    impl ShowImplementation {
        pub fn new(implementation_id: ImplementationId) -> Self {
            Self {
                kind_id: show_kind(),
                implementation_id,
                artifact_id: ArtifactId::from("conduit-signal/show-artifact-v1"),
            }
        }
    }

    impl OperationImplementation for ShowImplementation {
        fn kind_id(&self) -> &KindId {
            &self.kind_id
        }

        fn kind_contract_revision(&self) -> conduit_core::KindContractRevision {
            show_contract_revision()
        }

        fn execution_profile_id(&self) -> conduit_core::ExecutionProfileId {
            show_execution_profile()
        }

        fn implementation_id(&self) -> &ImplementationId {
            &self.implementation_id
        }

        fn artifact_id(&self) -> &ArtifactId {
            &self.artifact_id
        }

        fn host_operation_requirements(&self) -> Vec<conduit_core::HostOperationRequirement> {
            show_host_operation_requirements()
        }

        fn resource_requirements(&self) -> Vec<conduit_core::ResourceRequirement> {
            show_resource_requirements()
        }

        fn prepare(
            &self,
            _placement: &PlannedOperation,
        ) -> Result<Box<dyn OperationState>, ImplementationFailure> {
            Ok(Box::new(ShowState {
                expected_sequence: 0,
                pending: None,
            }))
        }

        fn minimum_value_size(&self, value_kind: &KindId) -> Option<u32> {
            (value_kind == &signal_value_kind()).then_some(SIGNAL_ENCODED_LEN)
        }
    }

    struct ShowState {
        expected_sequence: u64,
        pending: Option<Signal>,
    }

    impl OperationState for ShowState {
        fn start(&mut self) -> OperationAction {
            OperationAction::Idle
        }

        fn resume(&mut self, completion: OperationCompletion) -> OperationAction {
            match completion {
                OperationCompletion::Value { port, value } if port.as_str() == SIGNAL_PORT => {
                    match decode_signal(&value) {
                        Ok(signal) if signal.sequence == self.expected_sequence => {
                            self.pending = Some(signal);
                            OperationAction::Present {
                                presentation_kind: kind_id(SIGNAL_PRESENTATION_KIND),
                                value,
                            }
                        }
                        Ok(signal) => OperationAction::Fail(ImplementationFailure::new(
                            FailureReason::MalformedConnectionEnvelope,
                            format!(
                                "expected signal sequence {}, received {}",
                                self.expected_sequence, signal.sequence
                            ),
                        )),
                        Err(err) => OperationAction::Fail(ImplementationFailure::new(
                            FailureReason::UnsupportedValueKind,
                            err.to_string(),
                        )),
                    }
                }
                OperationCompletion::PresentationCompleted { success: true, .. } => {
                    self.pending = None;
                    self.expected_sequence += 1;
                    OperationAction::Idle
                }
                OperationCompletion::PresentationCompleted {
                    success: false,
                    message,
                } => OperationAction::Fail(ImplementationFailure {
                    reason: FailureReason::ManifestationFailed,
                    message,
                }),
                OperationCompletion::InputsClosed if self.pending.is_none() => {
                    OperationAction::Complete
                }
                _ => OperationAction::Fail(ImplementationFailure::new(
                    FailureReason::InvalidLifecycleCommand,
                    "show received an incompatible runtime completion",
                )),
            }
        }
    }

    pub fn install_signal_profile(
        registry: &mut ImplementationRegistry,
        pulse_implementation_id: ImplementationId,
        show_implementation_id: ImplementationId,
    ) -> Result<(), ImplementationFailure> {
        registry.install(PulseImplementation::new(pulse_implementation_id))?;
        registry.install(ShowImplementation::new(show_implementation_id))?;
        Ok(())
    }

    pub fn install_toggle_profile(
        registry: &mut ImplementationRegistry,
        activate_implementation_id: ImplementationId,
        toggle_implementation_id: ImplementationId,
    ) -> Result<(), ImplementationFailure> {
        registry.install(ActivateImplementation::new(activate_implementation_id))?;
        registry.install(ToggleImplementation::new(toggle_implementation_id))?;
        Ok(())
    }

    pub struct ActivateImplementation {
        kind_id: KindId,
        implementation_id: ImplementationId,
        artifact_id: ArtifactId,
    }

    impl ActivateImplementation {
        pub fn new(implementation_id: ImplementationId) -> Self {
            Self {
                kind_id: activate_kind(),
                implementation_id,
                artifact_id: ArtifactId::from("conduit-signal/activate-artifact-v1"),
            }
        }
    }

    impl OperationImplementation for ActivateImplementation {
        fn kind_id(&self) -> &KindId {
            &self.kind_id
        }

        fn kind_contract_revision(&self) -> conduit_core::KindContractRevision {
            activate_contract_revision()
        }

        fn execution_profile_id(&self) -> conduit_core::ExecutionProfileId {
            activate_execution_profile()
        }

        fn implementation_id(&self) -> &ImplementationId {
            &self.implementation_id
        }

        fn artifact_id(&self) -> &ArtifactId {
            &self.artifact_id
        }

        fn host_operation_requirements(&self) -> Vec<conduit_core::HostOperationRequirement> {
            activate_host_operation_requirements()
        }

        fn resource_requirements(&self) -> Vec<conduit_core::ResourceRequirement> {
            activate_resource_requirements()
        }

        fn prepare(
            &self,
            placement: &PlannedOperation,
        ) -> Result<Box<dyn OperationState>, ImplementationFailure> {
            let configuration =
                parse_activate_configuration(&placement.configuration).map_err(|err| {
                    ImplementationFailure::new(
                        FailureReason::InvalidOperationConfiguration,
                        err.to_string(),
                    )
                })?;
            Ok(Box::new(ActivateState {
                configuration,
                next_sequence: 0,
            }))
        }

        fn minimum_value_size(&self, value_kind: &KindId) -> Option<u32> {
            (value_kind == &activation_value_kind()).then_some(ACTIVATION_ENCODED_LEN)
        }
    }

    struct ActivateState {
        configuration: ActivateConfiguration,
        next_sequence: u64,
    }

    impl OperationState for ActivateState {
        fn start(&mut self) -> OperationAction {
            if self.next_sequence >= self.configuration.count {
                OperationAction::Complete
            } else {
                OperationAction::Wait { duration_ms: 0 }
            }
        }

        fn resume(&mut self, completion: OperationCompletion) -> OperationAction {
            match completion {
                OperationCompletion::TimerElapsed => {
                    if self.next_sequence >= self.configuration.count {
                        return OperationAction::Complete;
                    }
                    OperationAction::Emit(alloc::vec![OperationOutput {
                        port: port_id(ACTIVATE_PORT),
                        value: encode_activation(&Activation {
                            sequence: self.next_sequence,
                        }),
                    }])
                }
                OperationCompletion::Emitted => {
                    self.next_sequence += 1;
                    if self.next_sequence >= self.configuration.count {
                        OperationAction::Complete
                    } else {
                        OperationAction::Wait { duration_ms: 0 }
                    }
                }
                _ => OperationAction::Fail(ImplementationFailure::new(
                    FailureReason::InvalidLifecycleCommand,
                    "activate received an incompatible runtime completion",
                )),
            }
        }
    }

    pub struct ToggleImplementation {
        kind_id: KindId,
        implementation_id: ImplementationId,
        artifact_id: ArtifactId,
    }

    impl ToggleImplementation {
        pub fn new(implementation_id: ImplementationId) -> Self {
            Self {
                kind_id: toggle_kind(),
                implementation_id,
                artifact_id: ArtifactId::from("conduit-signal/toggle-artifact-v1"),
            }
        }
    }

    impl OperationImplementation for ToggleImplementation {
        fn kind_id(&self) -> &KindId {
            &self.kind_id
        }

        fn kind_contract_revision(&self) -> conduit_core::KindContractRevision {
            toggle_contract_revision()
        }

        fn execution_profile_id(&self) -> conduit_core::ExecutionProfileId {
            toggle_execution_profile()
        }

        fn implementation_id(&self) -> &ImplementationId {
            &self.implementation_id
        }

        fn artifact_id(&self) -> &ArtifactId {
            &self.artifact_id
        }

        fn host_operation_requirements(&self) -> Vec<conduit_core::HostOperationRequirement> {
            toggle_host_operation_requirements()
        }

        fn resource_requirements(&self) -> Vec<conduit_core::ResourceRequirement> {
            toggle_resource_requirements()
        }

        fn prepare(
            &self,
            placement: &PlannedOperation,
        ) -> Result<Box<dyn OperationState>, ImplementationFailure> {
            let configuration =
                parse_toggle_configuration(&placement.configuration).map_err(|err| {
                    ImplementationFailure::new(
                        FailureReason::InvalidOperationConfiguration,
                        err.to_string(),
                    )
                })?;
            Ok(Box::new(ToggleState {
                level: configuration.initial,
                expected_sequence: 0,
            }))
        }

        fn minimum_value_size(&self, value_kind: &KindId) -> Option<u32> {
            if value_kind == &activation_value_kind() {
                Some(ACTIVATION_ENCODED_LEN)
            } else if value_kind == &signal_value_kind() {
                Some(SIGNAL_ENCODED_LEN)
            } else {
                None
            }
        }
    }

    struct ToggleState {
        level: bool,
        expected_sequence: u64,
    }

    impl OperationState for ToggleState {
        fn start(&mut self) -> OperationAction {
            OperationAction::Idle
        }

        fn resume(&mut self, completion: OperationCompletion) -> OperationAction {
            match completion {
                OperationCompletion::Value { port, value } if port.as_str() == ACTIVATE_PORT => {
                    match decode_activation_bytes(&value.encoded) {
                        Ok(activation) if activation.sequence == self.expected_sequence => {
                            self.level = !self.level;
                            let signal = Signal {
                                sequence: self.expected_sequence,
                                level: self.level,
                            };
                            OperationAction::Emit(alloc::vec![OperationOutput {
                                port: port_id(SIGNAL_PORT),
                                value: encode_signal(&signal),
                            }])
                        }
                        Ok(activation) => OperationAction::Fail(ImplementationFailure::new(
                            FailureReason::MalformedConnectionEnvelope,
                            alloc::format!(
                                "expected activation sequence {}, received {}",
                                self.expected_sequence, activation.sequence
                            ),
                        )),
                        Err(err) => OperationAction::Fail(ImplementationFailure::new(
                            FailureReason::UnsupportedValueKind,
                            err.to_string(),
                        )),
                    }
                }
                OperationCompletion::Emitted => {
                    self.expected_sequence += 1;
                    OperationAction::Idle
                }
                OperationCompletion::InputsClosed => OperationAction::Complete,
                _ => OperationAction::Fail(ImplementationFailure::new(
                    FailureReason::InvalidLifecycleCommand,
                    "toggle received an incompatible runtime completion",
                )),
            }
        }
    }

    pub fn signal_registry(
        pulse_implementation_id: ImplementationId,
        show_implementation_id: ImplementationId,
    ) -> Result<ImplementationRegistry, ImplementationFailure> {
        let mut registry = ImplementationRegistry::new();
        install_signal_profile(
            &mut registry,
            pulse_implementation_id,
            show_implementation_id,
        )?;
        Ok(registry)
    }

    pub fn signal_profile_catalog() -> ProfileCatalog {
        let mut catalog = ProfileCatalog::new();
        catalog
            .insert(KindDefinition {
                kind_id: pulse_kind(),
                kind_contract_revision: pulse_contract_revision(),
                inputs: Vec::new(),
                outputs: pulse_outputs(),
                configuration: vec![
                    ConfigurationField {
                        key: "count".to_string(),
                        default_value: ConfigurationValue::U64(16),
                        validation: ConfigurationRule::U64Range {
                            minimum: 0,
                            maximum: MAX_SIGNAL_COUNT,
                        },
                    },
                    ConfigurationField {
                        key: "period-ms".to_string(),
                        default_value: ConfigurationValue::U64(250),
                        validation: ConfigurationRule::U64Range {
                            minimum: 0,
                            maximum: u64::MAX,
                        },
                    },
                    ConfigurationField {
                        key: "initial".to_string(),
                        default_value: ConfigurationValue::Bool(false),
                        validation: ConfigurationRule::Any,
                    },
                ],
            })
            .expect("signal profile kinds are unique");
        catalog
            .insert(KindDefinition {
                kind_id: show_kind(),
                kind_contract_revision: show_contract_revision(),
                inputs: show_inputs(),
                outputs: Vec::new(),
                configuration: Vec::new(),
            })
            .expect("signal profile kinds are unique");
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
        catalog
    }
}

#[cfg(feature = "host-profile")]
pub use host_profile::{
    install_signal_profile, install_toggle_profile, signal_profile_catalog, signal_registry,
    ActivateImplementation, PulseImplementation, ShowImplementation, ToggleImplementation,
};

#[cfg(test)]
mod tests {
    use super::{
        decode_activation_bytes, decode_signal, decode_signal_bytes, encode_activation,
        encode_signal, parse_activate_configuration, parse_pulse_configuration,
        parse_toggle_configuration, activate_configuration_entries, toggle_configuration_entries,
        pulse_configuration_entries, ActivateConfiguration, Activation, PulseConfiguration,
        ToggleConfiguration, Signal,
    };

    #[test]
    fn round_trips_signal_payload() {
        let payload = encode_signal(&Signal {
            sequence: 7,
            level: true,
        });
        let decoded = decode_signal(&payload).expect("signal payload should decode");
        assert_eq!(decoded.sequence, 7);
        assert!(decoded.level);
        assert_eq!(
            decode_signal_bytes(&payload.encoded).expect("fixed bytes should decode"),
            decoded
        );
    }

    #[test]
    fn round_trips_pulse_configuration_entries() {
        let config = PulseConfiguration {
            count: 3,
            period_ms: 0,
            initial_level: false,
        };
        let parsed = parse_pulse_configuration(&pulse_configuration_entries(&config))
            .expect("pulse configuration should parse");
        assert_eq!(parsed, config);
    }

    #[test]
    fn round_trips_activation_payload() {
        let activation = Activation { sequence: 42 };
        let payload = encode_activation(&activation);
        assert_eq!(payload.encoded.len(), super::ACTIVATION_ENCODED_LEN as usize);
        let decoded =
            decode_activation_bytes(&payload.encoded).expect("activation payload should decode");
        assert_eq!(decoded.sequence, 42);
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
