//! One canonical planned std-to-Pico USB-CDC Signal execution.

use alloc::collections::BTreeMap;
use alloc::vec;

use conduit_core::{
    process_owned_line_offer_with_limits, ArtifactId, CapabilityId, CapabilityLimits,
    CapabilityOffer, ConnectionBase, GearId, HostAdvertisement, HostId, HostProfileId,
    ImplementationId, LineOffer, LinkLimits, OfferGeneration, Plan, PROTOCOL_VERSION,
};
use conduit_planner::{plan_with_line_offers, PlacementChoice, PlacementChoices};

use crate::{
    pulse_contract_revision, pulse_execution_profile, pulse_host_operation_requirements,
    pulse_outputs, pulse_resource_requirements, show_contract_revision, show_execution_profile,
    show_host_operation_requirements, show_inputs, show_resource_requirements,
    signal_profile_catalog, signal_resource_offers, DISTRIBUTED_MAXIMUM_FRAME_BYTES,
    DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS, SIGNAL_ENCODED_LEN,
};

pub const STD_PICO_USB_SOURCE_HOST_ID: &str = "s4/std-pico-source";
pub const STD_PICO_USB_SOURCE_BOOT_ID: &str = "s4/std-pico-source-boot";
pub const STD_PICO_USB_SINK_HOST_ID: &str = "s4/pico-usb-sink";
pub const STD_PICO_USB_SINK_BOOT_ID: &str = "s4/pico-usb-sink-image-boot";
pub const STD_PICO_USB_LINK_BINDING_ID: &str = "s4/std-pico-usb-cdc-link";
pub const STD_PICO_USB_LINE_ID: &str = "s4/line/std-pico-usb-cdc";
pub const STD_PICO_USB_BASE_INSTANCE_ID: &str = "s4/pico-usb-cdc-0";
pub const STD_PICO_USB_SOURCE_ENDPOINT_ID: &str = "s4/std-pico-usb-egress";
pub const STD_PICO_USB_SINK_ENDPOINT_ID: &str = "s4/pico-usb-ingress";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactStdPicoUsbPlan {
    pub source_advertisement: HostAdvertisement,
    pub sink_advertisement: HostAdvertisement,
    pub line_offer: LineOffer,
    pub plan: Plan,
}

pub fn std_pico_usb_source_advertisement() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(STD_PICO_USB_SOURCE_HOST_ID),
        boot_id: conduit_core::BootId::from(STD_PICO_USB_SOURCE_BOOT_ID),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("rust-std-kernel"),
        resources: signal_resource_offers("s4/std-pico-timer", "s4/std-unused-presentation", 1)
            .into_iter()
            .filter(|resource| resource.class_id.as_str() == conduit_core::TIMER_RESOURCE_CLASS)
            .collect(),
        planner_capabilities: vec![],
        capabilities: vec![CapabilityOffer {
            startup_parameters: crate::pulse_face_startup_parameters(),
            shorthand: None,
            capability_id: CapabilityId::from("std-pico-pulse-1"),
            kind_id: crate::pulse_kind(),
            kind_contract_revision: pulse_contract_revision(),
            implementation: conduit_core::ImplementationOffer {
                execution_profile_id: pulse_execution_profile(),
                implementation_id: ImplementationId::from("std/kernel-pulse-v1"),
                artifact_id: ArtifactId::from("conduit-signal/pulse-artifact-v1"),
            },
            inputs: vec![],
            outputs: pulse_outputs(),
            host_operations: pulse_host_operation_requirements(),
            resource_requirements: pulse_resource_requirements(),
            authority_requirements: vec![],
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
                max_queue_bytes: SIGNAL_ENCODED_LEN,
            },
        }],
    }
}

pub fn std_pico_usb_sink_advertisement() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(STD_PICO_USB_SINK_HOST_ID),
        boot_id: conduit_core::BootId::from(STD_PICO_USB_SINK_BOOT_ID),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("rp2040-kernel"),
        resources: signal_resource_offers("s4/pico-unused-timer", "s4/pico-cyw43-led", 1)
            .into_iter()
            .filter(|resource| {
                resource.class_id.as_str() == conduit_core::PRESENTATION_RESOURCE_CLASS
            })
            .collect(),
        planner_capabilities: vec![],
        capabilities: vec![CapabilityOffer {
            startup_parameters: vec![],
            shorthand: None,
            capability_id: CapabilityId::from("pico-cyw43-show-1"),
            kind_id: crate::show_kind(),
            kind_contract_revision: show_contract_revision(),
            implementation: conduit_core::ImplementationOffer {
                execution_profile_id: show_execution_profile(),
                implementation_id: ImplementationId::from("pico/kernel-cyw43-show-signal-v1"),
                artifact_id: ArtifactId::from("conduit-signal/show-artifact-v1"),
            },
            inputs: show_inputs(),
            outputs: vec![],
            host_operations: show_host_operation_requirements(),
            resource_requirements: show_resource_requirements(),
            authority_requirements: vec![],
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
                max_queue_bytes: SIGNAL_ENCODED_LEN,
            },
        }],
    }
}

pub fn std_pico_usb_line_offer() -> LineOffer {
    let source = std_pico_usb_source_advertisement();
    let sink = std_pico_usb_sink_advertisement();
    let mut line = process_owned_line_offer_with_limits(
        STD_PICO_USB_LINE_ID,
        STD_PICO_USB_LINK_BINDING_ID,
        ConnectionBase::UsbCdc,
        STD_PICO_USB_BASE_INSTANCE_ID,
        &source,
        &sink,
        LinkLimits {
            maximum_in_flight_items: DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
            maximum_payload_bytes: SIGNAL_ENCODED_LEN,
            maximum_buffered_bytes: SIGNAL_ENCODED_LEN,
            maximum_frame_bytes: DISTRIBUTED_MAXIMUM_FRAME_BYTES,
        },
    );
    line.binding.source.endpoint_id =
        conduit_core::LinkEndpointId::from(STD_PICO_USB_SOURCE_ENDPOINT_ID);
    line.binding.sink.endpoint_id =
        conduit_core::LinkEndpointId::from(STD_PICO_USB_SINK_ENDPOINT_ID);
    line
}

pub fn exact_std_pico_usb_plan() -> Result<ExactStdPicoUsbPlan, alloc::string::String> {
    let source_advertisement = std_pico_usb_source_advertisement();
    let sink_advertisement = std_pico_usb_sink_advertisement();
    let form = conduit_form::parse(
        include_str!("../../../examples/signal-demo.form"),
        &signal_profile_catalog(),
    )
    .map_err(|error| error.to_string())?;
    let placements = PlacementChoices {
        by_gear: BTreeMap::from([
            (
                GearId::from("pulse"),
                PlacementChoice {
                    host_id: source_advertisement.host_id.clone(),
                    capability_id: CapabilityId::from("std-pico-pulse-1"),
                },
            ),
            (
                GearId::from("show"),
                PlacementChoice {
                    host_id: sink_advertisement.host_id.clone(),
                    capability_id: CapabilityId::from("pico-cyw43-show-1"),
                },
            ),
        ]),
    };
    let line_offer = std_pico_usb_line_offer();
    let plan = plan_with_line_offers(
        &form,
        &[source_advertisement.clone(), sink_advertisement.clone()],
        &placements,
        &[ConnectionBase::UsbCdc],
        DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
        SIGNAL_ENCODED_LEN,
        core::slice::from_ref(&line_offer),
    )
    .map_err(|error| error.to_string())?;
    Ok(ExactStdPicoUsbPlan {
        source_advertisement,
        sink_advertisement,
        line_offer,
        plan,
    })
}
