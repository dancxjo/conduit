//! Exact planner output for the R1 std-to-Pico LED Cord over selected Lines.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;

use conduit_core::{
    ArtifactId, BootId, CapabilityId, CapabilityLimits, CapabilityOffer, ConnectionBase, GearId,
    HostAdvertisement, HostProfileId, ImplementationId, LineOffer, OfferGeneration, Plan,
    PROTOCOL_VERSION,
};
use conduit_planner::{plan_with_options, PlacementChoice, PlacementChoices, PlanningOptions};

use conduit_signal::{
    pulse_contract_revision, pulse_execution_profile, pulse_host_operation_requirements,
    pulse_outputs, pulse_resource_requirements, show_contract_revision, show_execution_profile,
    show_host_operation_requirements, show_inputs, show_resource_requirements,
    signal_profile_catalog, signal_resource_offers, DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
    SIGNAL_ENCODED_LEN,
};

pub const R1_PULSE_CAPABILITY_ID: &str = "r1/std-pulse";
pub const R1_LED_CAPABILITY_ID: &str = "r1/pico-cyw43-led";

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum R1SignalRouteSet {
    UsbOnly,
    WebSocketOnly,
    WebSocketThenUsb,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactR1SignalPlan {
    pub source_advertisement: HostAdvertisement,
    pub pico_advertisement: HostAdvertisement,
    pub observed_lines: [LineOffer; 2],
    pub route_set: R1SignalRouteSet,
    pub plan: Plan,
}

pub fn r1_signal_source_advertisement() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: conduit_core::HostId::from(conduit_net::R1_STD_HOST_ID),
        boot_id: BootId::from(conduit_net::R1_STD_BOOT_ID),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("rust-std-r1-control"),
        resources: signal_resource_offers("r1/std-timer", "r1/std-unused-presentation", 1)
            .into_iter()
            .filter(|resource| resource.class_id.as_str() == conduit_core::TIMER_RESOURCE_CLASS)
            .collect(),
        planner_capabilities: vec![],
        capabilities: vec![CapabilityOffer {
            startup_parameters: conduit_signal::pulse_face_startup_parameters(),
            shorthand: None,
            capability_id: CapabilityId::from(R1_PULSE_CAPABILITY_ID),
            kind_id: conduit_signal::pulse_kind(),
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

pub fn r1_signal_pico_advertisement(boot_id: BootId) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: conduit_core::HostId::from(conduit_net::R1_PICO_HOST_ID),
        boot_id,
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("rp2040-r1-kernel"),
        resources: signal_resource_offers("r1/pico-unused-timer", "r1/pico-cyw43-led", 1)
            .into_iter()
            .filter(|resource| {
                resource.class_id.as_str() == conduit_core::PRESENTATION_RESOURCE_CLASS
            })
            .collect(),
        planner_capabilities: vec![],
        capabilities: vec![CapabilityOffer {
            startup_parameters: vec![],
            shorthand: None,
            capability_id: CapabilityId::from(R1_LED_CAPABILITY_ID),
            kind_id: conduit_signal::show_kind(),
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

pub fn exact_r1_signal_plan(
    pico_boot_id: BootId,
    route_set: R1SignalRouteSet,
) -> Result<ExactR1SignalPlan, String> {
    let source_advertisement = r1_signal_source_advertisement();
    let pico_advertisement = r1_signal_pico_advertisement(pico_boot_id.clone());
    let observed_lines = conduit_net::r1_line_basis(pico_boot_id);
    let form = conduit_form::parse_with_startup(
        include_str!("../../../fixtures/forms/signal-demo.conduit"),
        &conduit_signal::signal_startup_catalog(),
        &signal_profile_catalog(),
    )
    .map_err(|error| error.to_string())?;
    let placements = PlacementChoices {
        by_gear: BTreeMap::from([
            (
                GearId::from("signal-demo/pulse"),
                PlacementChoice {
                    host_id: source_advertisement.host_id.clone(),
                    capability_id: CapabilityId::from(R1_PULSE_CAPABILITY_ID),
                },
            ),
            (
                GearId::from("signal-demo/show"),
                PlacementChoice {
                    host_id: pico_advertisement.host_id.clone(),
                    capability_id: CapabilityId::from(R1_LED_CAPABILITY_ID),
                },
            ),
        ]),
    };
    let selected_lines: Vec<LineOffer> = match route_set {
        R1SignalRouteSet::UsbOnly => vec![observed_lines[0].clone()],
        R1SignalRouteSet::WebSocketOnly => vec![observed_lines[1].clone()],
        R1SignalRouteSet::WebSocketThenUsb => {
            vec![observed_lines[1].clone(), observed_lines[0].clone()]
        }
    };
    let allowed_bases: Vec<ConnectionBase> = selected_lines
        .iter()
        .map(|line| line.binding.base)
        .collect();
    let candidate_order = BTreeMap::from([(
        (
            GearId::from("signal-demo/pulse"),
            GearId::from("signal-demo/show"),
        ),
        selected_lines
            .iter()
            .map(|line| line.line_id.clone())
            .collect(),
    )]);
    let plan = plan_with_options(
        &form,
        &[source_advertisement.clone(), pico_advertisement.clone()],
        &placements,
        &allowed_bases,
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &candidate_order,
            connection_item_capacity: DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
            connection_byte_capacity: SIGNAL_ENCODED_LEN,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &selected_lines,
        },
    )
    .map_err(|error| error.to_string())?;
    Ok(ExactR1SignalPlan {
        source_advertisement,
        pico_advertisement,
        observed_lines,
        route_set,
        plan,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn remote_connection(plan: &Plan) -> &conduit_core::PlannedConnection {
        plan.fragments
            .iter()
            .flat_map(|fragment| &fragment.connections)
            .find(|connection| {
                !connection.admitted_lines.is_empty() || connection.selected_line.is_some()
            })
            .expect("remote LED Cord")
    }

    #[test]
    fn policy_seals_single_and_dual_line_plans_from_one_observed_topology() {
        let boot = BootId::from("r1/pico-runtime-boot");
        let usb = exact_r1_signal_plan(boot.clone(), R1SignalRouteSet::UsbOnly).unwrap();
        let websocket =
            exact_r1_signal_plan(boot.clone(), R1SignalRouteSet::WebSocketOnly).unwrap();
        let dual = exact_r1_signal_plan(boot, R1SignalRouteSet::WebSocketThenUsb).unwrap();
        assert_eq!(remote_connection(&usb.plan).admitted_lines.len(), 1);
        assert_eq!(
            remote_connection(&usb.plan).admitted_lines[0].binding.base,
            ConnectionBase::UsbCdc
        );
        assert_eq!(remote_connection(&websocket.plan).admitted_lines.len(), 1);
        assert_eq!(
            remote_connection(&websocket.plan).admitted_lines[0]
                .binding
                .base,
            ConnectionBase::WebSocket
        );
        assert_ne!(usb.plan.plan_id, websocket.plan.plan_id);
        assert_eq!(remote_connection(&dual.plan).admitted_lines.len(), 2);
        assert_eq!(
            remote_connection(&dual.plan).admitted_lines[0].binding.base,
            ConnectionBase::WebSocket
        );
        assert_eq!(
            remote_connection(&dual.plan).admitted_lines[1].binding.base,
            ConnectionBase::UsbCdc
        );
        assert_ne!(dual.plan.plan_id, usb.plan.plan_id);
        assert_ne!(dual.plan.plan_id, websocket.plan.plan_id);
        assert_eq!(
            usb.pico_advertisement.host_id,
            dual.pico_advertisement.host_id
        );
        assert_eq!(
            usb.pico_advertisement.boot_id,
            dual.pico_advertisement.boot_id
        );
        assert_eq!(
            usb.pico_advertisement.capabilities,
            dual.pico_advertisement.capabilities
        );
    }

    #[test]
    fn stale_boot_link_cannot_enter_the_replacement_plan() {
        let exact = exact_r1_signal_plan(
            BootId::from("r1/current-boot"),
            R1SignalRouteSet::WebSocketOnly,
        )
        .unwrap();
        let mut stale = exact.observed_lines[1].clone();
        stale.binding.sink.boot_id = BootId::from("r1/stale-boot");
        let form = conduit_form::parse_with_startup(
            include_str!("../../../fixtures/forms/signal-demo.conduit"),
            &conduit_signal::signal_startup_catalog(),
            &signal_profile_catalog(),
        )
        .unwrap();
        let placements = PlacementChoices {
            by_gear: BTreeMap::from([
                (
                    GearId::from("signal-demo/pulse"),
                    PlacementChoice {
                        host_id: exact.source_advertisement.host_id.clone(),
                        capability_id: CapabilityId::from(R1_PULSE_CAPABILITY_ID),
                    },
                ),
                (
                    GearId::from("signal-demo/show"),
                    PlacementChoice {
                        host_id: exact.pico_advertisement.host_id.clone(),
                        capability_id: CapabilityId::from(R1_LED_CAPABILITY_ID),
                    },
                ),
            ]),
        };
        assert!(plan_with_options(
            &form,
            &[exact.source_advertisement, exact.pico_advertisement],
            &placements,
            &[ConnectionBase::WebSocket],
            PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::from([(
                    (
                        GearId::from("signal-demo/pulse"),
                        GearId::from("signal-demo/show")
                    ),
                    vec![stale.line_id.clone()],
                )]),
                connection_item_capacity: DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
                connection_byte_capacity: SIGNAL_ENCODED_LEN,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[stale],
            },
        )
        .is_err());
    }
}
