#![cfg_attr(not(feature = "host-profile"), no_std)]

#[cfg(feature = "host-profile")]
extern crate alloc;

#[cfg(feature = "host-profile")]
mod trigger;
#[cfg(feature = "host-profile")]
pub use trigger::*;
#[cfg(feature = "host-profile")]
mod control;
#[cfg(feature = "host-profile")]
pub use control::*;
#[cfg(feature = "host-profile")]
mod canonical;
#[cfg(feature = "host-profile")]
pub use canonical::signal_startup_catalog;
#[cfg(feature = "host-profile")]
mod distributed_identity;
#[cfg(feature = "host-profile")]
pub use distributed_identity::{
    distributed_source_advertisement_for, distributed_websocket_line_offer_for,
};
#[cfg(feature = "host-profile")]
mod distributed_plan;
#[cfg(feature = "host-profile")]
pub use distributed_plan::{
    exact_distributed_signal_plan, exact_distributed_signal_plan_for, DistributedSignalPlan,
};
#[cfg(feature = "host-profile")]
mod esp32_wroom;
#[cfg(feature = "host-profile")]
pub use esp32_wroom::*;
#[cfg(feature = "host-profile")]
mod std_pico_usb;
#[cfg(feature = "host-profile")]
pub use std_pico_usb::*;
#[cfg(feature = "host-profile")]
pub mod triple;

#[cfg(feature = "host-profile")]
use alloc::string::ToString;
#[cfg(feature = "host-profile")]
use alloc::vec;
#[cfg(feature = "host-profile")]
use alloc::vec::Vec;
#[cfg(feature = "host-profile")]
use conduit_core::{
    kind_id, port_id, present_host_operation_requirement, resource_offer, resource_requirement,
    wait_host_operation_requirement, ArtifactId, BootId, CapabilityId, CapabilityLimits,
    CapabilityOffer, ConfigurationEntry, ConfigurationValue, ConnectionBase,
    ConnectionBaseInstanceId, ExecutionProfileId, HostAdvertisement, HostId,
    HostOperationRequirement, HostProfileId, ImplementationId, KindContractRevision, KindId,
    LineAvailability, LineAvailabilitySign, LineContinuation, LineContract, LineDuplex, LineId,
    LineOffer, LineOrdering, LineReliability, LineScope, LineSecurity, LineTrafficShape,
    LinkAuthorityReference, LinkBinding, LinkBindingId, LinkCredentialReference, LinkEndpoint,
    LinkEndpointId, LinkLimits, OfferGeneration, PortDescriptor, PortDirection, ResourceOffer,
    ResourceRequirement, SignId, ValuePayload, PRESENTATION_RESOURCE_CLASS, PROTOCOL_VERSION,
    TIMER_RESOURCE_CLASS,
};
use serde::{Deserialize, Serialize};

pub const SIGNAL_VALUE_KIND: &str = "value/signal";
pub const PULSE_KIND: &str = "flow/pulse";
pub const SHOW_KIND: &str = "presentation/show";
pub const SIGNAL_PORT: &str = "signal";

#[cfg(feature = "host-profile")]
pub fn pulse_face_startup_parameters() -> Vec<conduit_core::FaceStartupParameter> {
    vec![
        conduit_core::FaceStartupParameter {
            name: "count".to_string(),
            value_type: "Count".to_string(),
            has_default: true,
        },
        conduit_core::FaceStartupParameter {
            name: "period-ms".to_string(),
            value_type: "Count".to_string(),
            has_default: true,
        },
        conduit_core::FaceStartupParameter {
            name: "initial".to_string(),
            value_type: "Boolean".to_string(),
            has_default: true,
        },
    ]
}
pub const SIGNAL_ENCODED_LEN: u32 = 9;
pub const SIGNAL_PRESENTATION_KIND: &str = "presentation/signal";
pub const MAX_SIGNAL_COUNT: u64 = 4_096;
pub const PULSE_CONTRACT_REVISION: &str = "conduit.signal/flow-pulse@1";
pub const SHOW_CONTRACT_REVISION: &str = "conduit.signal/presentation-show@1";
pub const PULSE_EXECUTION_PROFILE: &str = "conduit.signal/pulse-hosted@1";
pub const SHOW_EXECUTION_PROFILE: &str = "conduit.signal/show-hosted@1";
pub const SIGNAL_ENCODED_LEN_USIZE: usize = SIGNAL_ENCODED_LEN as usize;
pub type EncodedSignal = [u8; SIGNAL_ENCODED_LEN_USIZE];
pub const DISTRIBUTED_STD_HOST_ID: &str = "s4/std-source";
pub const DISTRIBUTED_STD_BOOT_ID: &str = "s4/std-source-boot";
pub const DISTRIBUTED_BROWSER_HOST_ID: &str = "s4/browser-sink";
pub const DISTRIBUTED_BROWSER_BOOT_ID: &str = "s4/browser-sink-boot";
pub const DISTRIBUTED_LINK_BINDING_ID: &str = "s4/std-browser-link";
pub const DISTRIBUTED_LINE_ID: &str = "s4/line/distributed-websocket";
pub const DISTRIBUTED_BASE_INSTANCE_ID: &str = "s4/websocket-loopback-instance";
pub const DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS: u16 = 1;
pub const DISTRIBUTED_MAXIMUM_BUFFERED_BYTES: u32 = SIGNAL_ENCODED_LEN;
pub const DISTRIBUTED_MAXIMUM_FRAME_BYTES: u32 = 2_048;
pub const PICO_LOCAL_HOST_ID: &str = "s4/pico-local";
pub const PICO_LOCAL_BOOT_ID: &str = "s4/pico-local-boot";
pub const PICO_TIMER_POOL_ID: &str = "s4/pico-timer";
pub const PICO_PRESENTATION_POOL_ID: &str = "s4/pico-cyw43-led";

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
    InvalidConfiguration(&'static str),
    WrongValueKind,
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
            SignalProfileError::WrongValueKind => f.write_str("wrong value kind"),
            SignalProfileError::WrongEncodedLength(length) => {
                write!(f, "wrong encoded signal length {length}")
            }
        }
    }
}

#[cfg(feature = "host-profile")]
pub fn pulse_kind() -> KindId {
    kind_id(PULSE_KIND)
}

#[cfg(feature = "host-profile")]
pub fn show_kind() -> KindId {
    kind_id(SHOW_KIND)
}

#[cfg(feature = "host-profile")]
pub fn signal_value_kind() -> KindId {
    kind_id(SIGNAL_VALUE_KIND)
}

#[cfg(feature = "host-profile")]
pub fn pulse_contract_revision() -> KindContractRevision {
    KindContractRevision::from(PULSE_CONTRACT_REVISION)
}

#[cfg(feature = "host-profile")]
pub fn show_contract_revision() -> KindContractRevision {
    KindContractRevision::from(SHOW_CONTRACT_REVISION)
}

#[cfg(feature = "host-profile")]
pub fn pulse_execution_profile() -> ExecutionProfileId {
    ExecutionProfileId::from(PULSE_EXECUTION_PROFILE)
}

#[cfg(feature = "host-profile")]
pub fn show_execution_profile() -> ExecutionProfileId {
    ExecutionProfileId::from(SHOW_EXECUTION_PROFILE)
}

#[cfg(feature = "host-profile")]
pub fn pulse_host_operation_requirements() -> Vec<HostOperationRequirement> {
    vec![wait_host_operation_requirement()]
}

#[cfg(feature = "host-profile")]
pub fn show_host_operation_requirements() -> Vec<HostOperationRequirement> {
    vec![present_host_operation_requirement(
        kind_id(SIGNAL_PRESENTATION_KIND),
        SIGNAL_ENCODED_LEN,
    )]
}

#[cfg(feature = "host-profile")]
pub fn pulse_resource_requirements() -> Vec<ResourceRequirement> {
    vec![resource_requirement(TIMER_RESOURCE_CLASS, 1)]
}

#[cfg(feature = "host-profile")]
pub fn show_resource_requirements() -> Vec<ResourceRequirement> {
    vec![resource_requirement(PRESENTATION_RESOURCE_CLASS, 1)]
}

#[cfg(feature = "host-profile")]
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

#[cfg(feature = "host-profile")]
pub fn pulse_outputs() -> Vec<PortDescriptor> {
    vec![PortDescriptor {
        port_id: port_id(SIGNAL_PORT),
        value_kind: signal_value_kind(),
        direction: PortDirection::Output,
        temporal: conduit_core::PortTemporal::Value,
    }]
}

#[cfg(feature = "host-profile")]
pub fn show_inputs() -> Vec<PortDescriptor> {
    vec![PortDescriptor {
        port_id: port_id(SIGNAL_PORT),
        value_kind: signal_value_kind(),
        direction: PortDirection::Input,
        temporal: conduit_core::PortTemporal::Value,
    }]
}

#[cfg(feature = "host-profile")]
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

#[cfg(feature = "host-profile")]
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
                return Err(SignalProfileError::InvalidConfiguration(
                    match entry.key.as_str() {
                        "count" => "count",
                        "period-ms" => "period-ms",
                        "initial" => "initial",
                        _ => "unknown",
                    },
                ));
            }
            _ => {}
        }
    }
    let count = count.ok_or(SignalProfileError::MissingConfiguration("count"))?;
    if count > MAX_SIGNAL_COUNT {
        return Err(SignalProfileError::InvalidConfiguration("count"));
    }
    Ok(PulseConfiguration {
        count,
        period_ms: period_ms.ok_or(SignalProfileError::MissingConfiguration("period-ms"))?,
        initial_level: initial_level.ok_or(SignalProfileError::MissingConfiguration("initial"))?,
    })
}

pub fn signal_level_for_sequence(sequence: u64, initial_level: bool) -> bool {
    if sequence.is_multiple_of(2) {
        initial_level
    } else {
        !initial_level
    }
}

pub fn encode_signal_fixed(signal: &Signal) -> EncodedSignal {
    let mut encoded = [0u8; SIGNAL_ENCODED_LEN_USIZE];
    encoded[..8].copy_from_slice(&signal.sequence.to_le_bytes());
    encoded[8] = u8::from(signal.level);
    encoded
}

pub fn encode_signal_into(signal: &Signal, encoded: &mut [u8]) -> Result<(), SignalProfileError> {
    if encoded.len() != SIGNAL_ENCODED_LEN_USIZE {
        return Err(SignalProfileError::WrongEncodedLength(encoded.len()));
    }
    encoded.copy_from_slice(&encode_signal_fixed(signal));
    Ok(())
}

pub fn decode_signal_fixed(encoded: &EncodedSignal) -> Signal {
    let mut sequence = [0u8; 8];
    sequence.copy_from_slice(&encoded[..8]);
    Signal {
        sequence: u64::from_le_bytes(sequence),
        level: encoded[8] != 0,
    }
}

#[cfg(feature = "host-profile")]
pub fn encode_signal(signal: &Signal) -> ValuePayload {
    let mut encoded = Vec::with_capacity(SIGNAL_ENCODED_LEN_USIZE);
    encoded.extend_from_slice(&encode_signal_fixed(signal));
    ValuePayload {
        value_kind: signal_value_kind(),
        encoded,
    }
}

#[cfg(feature = "host-profile")]
pub fn decode_signal(payload: &ValuePayload) -> Result<Signal, SignalProfileError> {
    if payload.value_kind.as_str() != SIGNAL_VALUE_KIND {
        return Err(SignalProfileError::WrongValueKind);
    }
    decode_signal_bytes(&payload.encoded)
}

pub fn decode_signal_bytes(encoded: &[u8]) -> Result<Signal, SignalProfileError> {
    if encoded.len() != SIGNAL_ENCODED_LEN_USIZE {
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

/// Exact Pico-local Signal offer used to plan the constrained firmware image.
///
/// This is a capability advertisement only. It does not claim that firmware was
/// built, flashed, booted, or physically observed.
#[cfg(feature = "host-profile")]
pub fn pico_local_advertisement() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(PICO_LOCAL_HOST_ID),
        boot_id: BootId::from(PICO_LOCAL_BOOT_ID),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("pico-w-signal-kernel"),
        resources: signal_resource_offers(PICO_TIMER_POOL_ID, PICO_PRESENTATION_POOL_ID, 1),
        planner_capabilities: vec![],
        capabilities: vec![
            CapabilityOffer {
                startup_parameters: pulse_face_startup_parameters(),
                shorthand: None,
                capability_id: CapabilityId::from("pico-pulse-1"),
                kind_id: pulse_kind(),
                kind_contract_revision: pulse_contract_revision(),
                implementation: conduit_core::ImplementationOffer {
                    execution_profile_id: pulse_execution_profile(),
                    implementation_id: ImplementationId::from("pico-w/kernel-pulse-timer-v1"),
                    artifact_id: ArtifactId::from("conduit-signal/pico-pulse-artifact-v1"),
                },
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
            },
            CapabilityOffer {
                startup_parameters: vec![],
                shorthand: None,
                capability_id: CapabilityId::from("pico-led-show-1"),
                kind_id: show_kind(),
                kind_contract_revision: show_contract_revision(),
                implementation: conduit_core::ImplementationOffer {
                    execution_profile_id: show_execution_profile(),
                    implementation_id: ImplementationId::from("pico-w/kernel-cyw43-show-signal-v1"),
                    artifact_id: ArtifactId::from("conduit-signal/pico-cyw43-show-artifact-v1"),
                },
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
            },
        ],
    }
}

/// Exact production host facts used by the live S4 std-to-browser checkpoint.
/// The ephemeral loopback URL is line configuration and is deliberately not
/// part of these semantic or plan-visible identities.
#[cfg(feature = "host-profile")]
pub fn distributed_std_source_advertisement() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(DISTRIBUTED_STD_HOST_ID),
        boot_id: BootId::from(DISTRIBUTED_STD_BOOT_ID),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("rust-std-kernel"),
        resources: vec![resource_offer("s4/std-timer", TIMER_RESOURCE_CLASS, 1)],
        planner_capabilities: vec![],
        capabilities: vec![CapabilityOffer {
            startup_parameters: pulse_face_startup_parameters(),
            shorthand: None,
            capability_id: CapabilityId::from("pulse-1"),
            kind_id: pulse_kind(),
            kind_contract_revision: pulse_contract_revision(),
            implementation: conduit_core::ImplementationOffer {
                execution_profile_id: pulse_execution_profile(),
                implementation_id: ImplementationId::from("std/kernel-pulse-v1"),
                artifact_id: ArtifactId::from("conduit-signal/pulse-artifact-v1"),
            },
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

#[cfg(feature = "host-profile")]
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
        planner_capabilities: vec![],
        capabilities: vec![
            CapabilityOffer {
                startup_parameters: vec![],
                shorthand: None,
                capability_id: CapabilityId::from("dom-show-1"),
                kind_id: show_kind(),
                kind_contract_revision: show_contract_revision(),
                implementation: conduit_core::ImplementationOffer {
                    execution_profile_id: show_execution_profile(),
                    implementation_id: ImplementationId::from("browser/kernel-dom-show-signal-v1"),
                    artifact_id: ArtifactId::from("conduit-signal/show-artifact-v1"),
                },
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
            },
            trigger::toggle_browser_presentation_offer(
                conduit_std_catalog::BOOL_PRESENTATION_CAPABILITY,
            ),
        ],
    }
}

#[cfg(feature = "host-profile")]
pub fn distributed_websocket_line_offer() -> LineOffer {
    let binding = LinkBinding {
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
        base: ConnectionBase::WebSocket,
        base_instance_id: ConnectionBaseInstanceId::from(DISTRIBUTED_BASE_INSTANCE_ID),
        credential: LinkCredentialReference::None,
        authority: LinkAuthorityReference::ProcessOwned,
        limits: LinkLimits {
            maximum_in_flight_items: DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
            maximum_payload_bytes: SIGNAL_ENCODED_LEN,
            maximum_buffered_bytes: DISTRIBUTED_MAXIMUM_BUFFERED_BYTES,
            maximum_frame_bytes: DISTRIBUTED_MAXIMUM_FRAME_BYTES,
        },
    };
    LineOffer {
        line_id: LineId::from(DISTRIBUTED_LINE_ID),
        availability: LineAvailabilitySign {
            line_id: LineId::from(DISTRIBUTED_LINE_ID),
            binding_id: binding.binding_id.clone(),
            availability: LineAvailability::Ready,
            sign_id: SignId::from("s4/line/distributed-websocket/ready"),
        },
        binding,
        contract: LineContract {
            scope: LineScope::Machine,
            traffic_shape: LineTrafficShape::Message,
            duplex: LineDuplex::FullDuplex,
            ordering: LineOrdering::Ordered,
            reliability: LineReliability::Reliable,
            continuation: LineContinuation::None,
            security: LineSecurity::PlaintextNetwork,
        },
    }
}

#[cfg(feature = "legacy-fixture-driver")]
mod host_profile;
#[cfg(feature = "legacy-fixture-driver")]
pub use host_profile::{
    install_signal_profile, signal_registry, PulseImplementation, ShowImplementation,
};

#[cfg(feature = "host-profile")]
mod profile_catalog;
#[cfg(feature = "host-profile")]
pub fn signal_profile_catalog() -> conduit_form::ProfileCatalog {
    let mut catalog = profile_catalog::signal_profile_catalog();
    trigger::extend_profile_catalog(&mut catalog);
    control::extend_control_profile_catalog(&mut catalog);
    catalog
}

#[cfg(test)]
mod tests;
