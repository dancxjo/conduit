#![cfg_attr(not(feature = "host-profile"), no_std)]

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    kind_id, port_id, present_host_operation_requirement, resource_offer, resource_requirement,
    wait_host_operation_requirement, ArtifactId, BootId, CapabilityId, CapabilityLimits,
    CapabilityOffer, ConfigurationEntry, ConfigurationValue, ConnectionProvider,
    ConnectionProviderInstanceId, ExecutionProfileId, HostAdvertisement, HostId,
    HostOperationRequirement, HostProfileId, ImplementationId, KindContractRevision, KindId,
    LinkAuthorityReference, LinkAvailability, LinkBinding, LinkBindingId, LinkCredentialReference,
    LinkEndpoint, LinkEndpointId, LinkLimits, OfferGeneration, PortDescriptor, PortDirection,
    ResourceOffer, ResourceRequirement, ValuePayload, PRESENTATION_RESOURCE_CLASS,
    PROTOCOL_VERSION, TIMER_RESOURCE_CLASS,
};
use serde::{Deserialize, Serialize};

pub const SIGNAL_VALUE_KIND: &str = "value/signal";
pub const PULSE_KIND: &str = "flow/pulse";
pub const SHOW_KIND: &str = "presentation/show";
pub const SIGNAL_PORT: &str = "signal";
pub const SIGNAL_ENCODED_LEN: u32 = 9;
pub const SIGNAL_PRESENTATION_KIND: &str = "presentation/signal";
pub const MAX_SIGNAL_COUNT: u64 = 4_096;
pub const PULSE_CONTRACT_REVISION: &str = "conduit.signal/flow-pulse@1";
pub const SHOW_CONTRACT_REVISION: &str = "conduit.signal/presentation-show@1";
pub const PULSE_EXECUTION_PROFILE: &str = "conduit.signal/pulse-hosted@1";
pub const SHOW_EXECUTION_PROFILE: &str = "conduit.signal/show-hosted@1";
pub const DISTRIBUTED_STD_HOST_ID: &str = "s4/std-source";
pub const DISTRIBUTED_STD_BOOT_ID: &str = "s4/std-source-boot";
pub const DISTRIBUTED_BROWSER_HOST_ID: &str = "s4/browser-sink";
pub const DISTRIBUTED_BROWSER_BOOT_ID: &str = "s4/browser-sink-boot";
pub const DISTRIBUTED_LINK_BINDING_ID: &str = "s4/std-browser-link";
pub const DISTRIBUTED_PROVIDER_INSTANCE_ID: &str = "s4/websocket-loopback-instance";
pub const DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS: u16 = 1;
pub const DISTRIBUTED_MAXIMUM_BUFFERED_BYTES: u32 = SIGNAL_ENCODED_LEN;
pub const DISTRIBUTED_MAXIMUM_FRAME_BYTES: u32 = 2_048;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signal {
    pub sequence: u64,
    pub level: bool,
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

#[cfg(feature = "host-profile")]
mod host_profile;
#[cfg(feature = "host-profile")]
pub use host_profile::{
    install_signal_profile, signal_profile_catalog, signal_registry, PulseImplementation,
    ShowImplementation,
};

#[cfg(test)]
mod tests;
