//! Fixed std/browser Trigger and Toggle proof identities.

use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    resource_offer, ArtifactId, BootId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ConnectionBase, ConnectionBaseInstanceId, HostAdvertisement, HostId, HostProfileId,
    ImplementationId, LineAvailability, LineAvailabilitySign, LineContinuation, LineContract,
    LineDuplex, LineId, LineOffer, LineOrdering, LineReliability, LineScope, LineSecurity,
    LineTrafficShape, LinkAuthorityReference, LinkBinding, LinkBindingId, LinkCredentialReference,
    LinkEndpoint, LinkEndpointId, LinkLimits, OfferGeneration, INPUT_RESOURCE_CLASS,
    PRESENTATION_RESOURCE_CLASS, PROTOCOL_VERSION,
};

use crate::*;

pub const DISTRIBUTED_TOGGLE_STD_HOST_ID: &str = "s4/toggle-std-source";
pub const DISTRIBUTED_TOGGLE_STD_BOOT_ID: &str = "s4/toggle-std-source-boot";
pub const DISTRIBUTED_TOGGLE_BROWSER_HOST_ID: &str = "s4/toggle-browser-sink";
pub const DISTRIBUTED_TOGGLE_BROWSER_BOOT_ID: &str = "s4/toggle-browser-sink-boot";
pub const DISTRIBUTED_TOGGLE_LINK_BINDING_ID: &str = "s4/toggle-std-browser-link";
pub const DISTRIBUTED_TOGGLE_BASE_INSTANCE_ID: &str = "s4/toggle-websocket-loopback-instance";

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
        planner_capabilities: vec![],
        capabilities: vec![
            CapabilityOffer {
                startup_parameters: trigger_face_startup_parameters(),
                shorthand: None,
                capability_id: CapabilityId::from("trigger-1"),
                kind_id: trigger_kind(),
                kind_contract_revision: trigger_contract_revision(),
                implementation: conduit_core::ImplementationOffer {
                    execution_profile_id: trigger_execution_profile(),
                    implementation_id: ImplementationId::from("std/kernel-trigger-v1"),
                    artifact_id: ArtifactId::from("conduit-signal/trigger-artifact-v1"),
                },
                inputs: Vec::new(),
                outputs: trigger_outputs(),
                host_operations: trigger_host_operation_requirements(),
                resource_requirements: trigger_resource_requirements(),
                authority_requirements: Vec::new(),
                limits: CapabilityLimits {
                    max_active_instances: 1,
                    max_queue_items: DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
                    max_queue_bytes: DISTRIBUTED_MAXIMUM_BUFFERED_BYTES,
                },
            },
            CapabilityOffer {
                startup_parameters: toggle_face_startup_parameters(),
                shorthand: None,
                capability_id: CapabilityId::from("toggle-1"),
                kind_id: toggle_kind(),
                kind_contract_revision: toggle_contract_revision(),
                implementation: conduit_std_catalog::state_toggle_offer().implementation,
                inputs: toggle_inputs(),
                outputs: toggle_outputs(),
                host_operations: toggle_host_operation_requirements(),
                resource_requirements: toggle_resource_requirements(),
                authority_requirements: Vec::new(),
                limits: conduit_std_catalog::state_toggle_offer().limits,
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
        planner_capabilities: vec![],
        capabilities: vec![toggle_browser_presentation_offer("toggle-dom-show-1")],
    }
}

pub fn toggle_browser_presentation_offer(capability_id: &str) -> CapabilityOffer {
    let mut offer = conduit_std_catalog::bool_presentation_browser_offer();
    offer.capability_id = CapabilityId::from(capability_id);
    offer
}

pub fn distributed_toggle_websocket_line_offer() -> LineOffer {
    let binding = LinkBinding {
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
        base: ConnectionBase::WebSocket,
        base_instance_id: ConnectionBaseInstanceId::from(DISTRIBUTED_TOGGLE_BASE_INSTANCE_ID),
        credential: LinkCredentialReference::None,
        authority: LinkAuthorityReference::ProcessOwned,
        limits: LinkLimits {
            maximum_in_flight_items: DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
            maximum_payload_bytes: TRIGGER_ENCODED_LEN,
            maximum_buffered_bytes: TRIGGER_ENCODED_LEN,
            maximum_frame_bytes: DISTRIBUTED_MAXIMUM_FRAME_BYTES,
        },
    };
    LineOffer {
        line_id: LineId::from("s4/line/toggle-websocket"),
        availability: LineAvailabilitySign {
            line_id: LineId::from("s4/line/toggle-websocket"),
            binding_id: binding.binding_id.clone(),
            availability: LineAvailability::Ready,
            sign_id: conduit_core::SignId::from("s4/line/toggle-websocket/ready"),
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
