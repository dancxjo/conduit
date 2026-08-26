//! Fixed Pico-local and accepted std/browser Signal proof identities.

use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    resource_offer, ArtifactId, BootId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ConnectionBase, ConnectionBaseInstanceId, HostAdvertisement, HostId, HostProfileId,
    ImplementationId, LineAvailability, LineAvailabilitySign, LineContinuation, LineContract,
    LineDuplex, LineId, LineOffer, LineOrdering, LineReliability, LineScope, LineSecurity,
    LineTrafficShape, LinkAuthorityReference, LinkBinding, LinkBindingId, LinkCredentialReference,
    LinkEndpoint, LinkEndpointId, LinkLimits, OfferGeneration, SignId, PRESENTATION_RESOURCE_CLASS,
    PROTOCOL_VERSION, TIMER_RESOURCE_CLASS,
};

use crate::*;

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
            toggle_browser_presentation_offer(conduit_std_catalog::BOOL_PRESENTATION_CAPABILITY),
        ],
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pico_local_advertisement_names_exact_constrained_signal_profile() {
        let advertisement = pico_local_advertisement();
        assert_eq!(advertisement.host_id.as_str(), PICO_LOCAL_HOST_ID);
        assert_eq!(advertisement.boot_id.as_str(), PICO_LOCAL_BOOT_ID);
        assert_eq!(advertisement.capabilities.len(), 2);
        assert_eq!(advertisement.resources.len(), 2);
        assert!(advertisement.capabilities.iter().all(|capability| {
            capability.limits.max_active_instances == 1
                && capability.limits.max_queue_items == 1
                && capability.limits.max_queue_bytes == SIGNAL_ENCODED_LEN
        }));
        assert!(advertisement.capabilities.iter().any(|capability| {
            capability.kind_id == pulse_kind()
                && capability.outputs == pulse_outputs()
                && capability.inputs.is_empty()
        }));
        assert!(advertisement.capabilities.iter().any(|capability| {
            capability.kind_id == show_kind()
                && capability.inputs == show_inputs()
                && capability.outputs.is_empty()
        }));
    }
}
