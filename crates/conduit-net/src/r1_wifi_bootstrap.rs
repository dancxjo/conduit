//! Exact ordinary R1 Plan that carries volatile network credentials over the
//! already observed UsbCdc Line to the same Pico boot.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;

use conduit_core::{
    authority_grant, process_owned_link_binding_with_limits, ArtifactId, AuthorityGrant,
    CapabilityId, ConnectionBase, HostAdvertisement, HostId, HostProfileId, ImplementationId,
    LinkBinding, LinkEndpointId, LinkLimits, OfferGeneration, Plan, PROTOCOL_VERSION,
};
use conduit_planner::{
    plan_expanded_canonical_with_options, PlacementChoice, PlacementChoices, PlanningOptions,
};

use crate::{
    install_network_bootstrap_catalogs, network_attachment_clue_offer, network_credentials_offer,
    network_join_offer, wifi_station_resource, MAXIMUM_JOIN_INPUT_BYTES,
};

pub const R1_STD_HOST_ID: &str = "r1/std-bootstrap";
pub const R1_STD_BOOT_ID: &str = "r1/std-bootstrap-boot";
pub const R1_PICO_HOST_ID: &str = "r1/pico-w";
pub const R1_PICO_BOOT_ID: &str = "r1/pico-w-boot";
pub const R1_CREDENTIALS_CAPABILITY_ID: &str = "r1/std-network-credentials";
pub const R1_JOIN_CAPABILITY_ID: &str = "r1/pico-network-join";
pub const R1_ATTACHMENT_CLUE_CAPABILITY_ID: &str = "r1/pico-network-attachment-clue";
pub const R1_WIFI_POOL_ID: &str = crate::R1_WIFI_STATION_POOL_ID;
pub const R1_USB_LINK_BINDING_ID: &str = "r1/std-pico-usb-bootstrap";
pub const R1_USB_BASE_INSTANCE_ID: &str = "r1/pico-usb-cdc-0";
pub const R1_STD_USB_ENDPOINT_ID: &str = "r1/std-usb-egress";
pub const R1_PICO_USB_ENDPOINT_ID: &str = "r1/pico-usb-ingress";
pub const R1_CREDENTIALS_GRANT_ID: &str = "r1/grant/read-network-credentials";
pub const R1_JOIN_GRANT_ID: &str = "r1/grant/configure-pico-network";
pub const R1_MAXIMUM_FRAME_BYTES: u32 = 2_048;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactR1NetworkBootstrapPlan {
    pub source_advertisement: HostAdvertisement,
    pub pico_advertisement: HostAdvertisement,
    pub usb_link: LinkBinding,
    pub authority_grants: [AuthorityGrant; 2],
    pub plan: Plan,
}

pub fn r1_std_bootstrap_advertisement() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(R1_STD_HOST_ID),
        boot_id: conduit_core::BootId::from(R1_STD_BOOT_ID),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("rust-std-r1-bootstrap"),
        resources: vec![],
        capabilities: vec![network_credentials_offer(
            CapabilityId::from(R1_CREDENTIALS_CAPABILITY_ID),
            ImplementationId::from("std/protected-network-credentials@1"),
            ArtifactId::from("conduit-std-host/network-credentials@1"),
        )],
        planner_capabilities: vec![],
    }
}

pub fn r1_pico_network_advertisement() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(R1_PICO_HOST_ID),
        boot_id: conduit_core::BootId::from(R1_PICO_BOOT_ID),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("rp2040-r1-wifi-station"),
        resources: vec![wifi_station_resource(R1_WIFI_POOL_ID)],
        capabilities: vec![
            network_join_offer(
                CapabilityId::from(R1_JOIN_CAPABILITY_ID),
                ImplementationId::from("pico-w/cyw43-network-join@1"),
                ArtifactId::from("conduit-pico-w-signal/cyw43-network-join@1"),
            ),
            network_attachment_clue_offer(
                CapabilityId::from(R1_ATTACHMENT_CLUE_CAPABILITY_ID),
                ImplementationId::from("pico-w/usb-network-attachment-clue@1"),
                ArtifactId::from("conduit-pico-w-signal/usb-network-attachment-clue@1"),
            ),
        ],
        planner_capabilities: vec![],
    }
}

pub fn r1_usb_bootstrap_link() -> LinkBinding {
    let source = r1_std_bootstrap_advertisement();
    let sink = r1_pico_network_advertisement();
    let mut binding = process_owned_link_binding_with_limits(
        R1_USB_LINK_BINDING_ID,
        ConnectionBase::UsbCdc,
        R1_USB_BASE_INSTANCE_ID,
        &source,
        &sink,
        LinkLimits {
            maximum_in_flight_items: 1,
            maximum_payload_bytes: MAXIMUM_JOIN_INPUT_BYTES,
            maximum_buffered_bytes: MAXIMUM_JOIN_INPUT_BYTES,
            maximum_frame_bytes: R1_MAXIMUM_FRAME_BYTES,
        },
    );
    binding.source.endpoint_id = LinkEndpointId::from(R1_STD_USB_ENDPOINT_ID);
    binding.sink.endpoint_id = LinkEndpointId::from(R1_PICO_USB_ENDPOINT_ID);
    binding
}

pub fn exact_r1_network_bootstrap_plan() -> Result<ExactR1NetworkBootstrapPlan, String> {
    let source_advertisement = r1_std_bootstrap_advertisement();
    let pico_advertisement = r1_pico_network_advertisement();
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    install_network_bootstrap_catalogs(&mut startup, &mut profile)?;
    let syntax = conduit_form::parse_syntax_document(include_str!(
        "../../../examples/r1-network-bootstrap.conduit"
    ));
    let checked = conduit_form::check_syntax_document(&syntax, &startup)
        .map_err(|error| format!("{}: {}", error.code, error.message))?;
    let form = conduit_form::expand_canonical_form(&checked, "r1-network-bootstrap", &profile)
        .map_err(|error| error.to_string())?;
    let credentials_gear = form
        .gears
        .iter()
        .find(|gear| gear.kind_id.as_str() == crate::NETWORK_CREDENTIALS_OPERATION)
        .ok_or_else(|| "expanded bootstrap Form has no credentials Gear".to_string())?;
    let join_gear = form
        .gears
        .iter()
        .find(|gear| gear.kind_id.as_str() == crate::NETWORK_JOIN_OPERATION)
        .ok_or_else(|| "expanded bootstrap Form has no join Gear".to_string())?;
    let attachment_clue_gear = form
        .gears
        .iter()
        .find(|gear| gear.kind_id.as_str() == crate::NETWORK_ATTACHMENT_CLUE_OPERATION)
        .ok_or_else(|| "expanded bootstrap Form has no attachment Clue Gear".to_string())?;
    let placements = PlacementChoices {
        by_gear: BTreeMap::from([
            (
                credentials_gear.gear_id.clone(),
                PlacementChoice {
                    host_id: source_advertisement.host_id.clone(),
                    capability_id: CapabilityId::from(R1_CREDENTIALS_CAPABILITY_ID),
                },
            ),
            (
                join_gear.gear_id.clone(),
                PlacementChoice {
                    host_id: pico_advertisement.host_id.clone(),
                    capability_id: CapabilityId::from(R1_JOIN_CAPABILITY_ID),
                },
            ),
            (
                attachment_clue_gear.gear_id.clone(),
                PlacementChoice {
                    host_id: pico_advertisement.host_id.clone(),
                    capability_id: CapabilityId::from(R1_ATTACHMENT_CLUE_CAPABILITY_ID),
                },
            ),
        ]),
    };
    let credential_requirement = &source_advertisement.capabilities[0].authority_requirements[0];
    let join_requirement = &pico_advertisement.capabilities[0].authority_requirements[0];
    let authority_grants = [
        authority_grant(
            R1_CREDENTIALS_GRANT_ID,
            credential_requirement,
            source_advertisement.host_id.clone(),
            source_advertisement.boot_id.clone(),
            CapabilityId::from(R1_CREDENTIALS_CAPABILITY_ID),
        ),
        authority_grant(
            R1_JOIN_GRANT_ID,
            join_requirement,
            pico_advertisement.host_id.clone(),
            pico_advertisement.boot_id.clone(),
            CapabilityId::from(R1_JOIN_CAPABILITY_ID),
        ),
    ];
    let usb_link = r1_usb_bootstrap_link();
    let plan = plan_expanded_canonical_with_options(
        &form,
        &[source_advertisement.clone(), pico_advertisement.clone()],
        &placements,
        &[ConnectionBase::Local, ConnectionBase::UsbCdc],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            route_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: MAXIMUM_JOIN_INPUT_BYTES,
            authority_grants: &authority_grants,
            protected_resource_grants: &[],
            link_bindings: core::slice::from_ref(&usb_link),
        },
    )
    .map_err(|error| error.to_string())?;
    Ok(ExactR1NetworkBootstrapPlan {
        source_advertisement,
        pico_advertisement,
        usb_link,
        authority_grants,
        plan,
    })
}
