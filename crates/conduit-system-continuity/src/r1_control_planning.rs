//! Exact ordinary-planner output for three deliberate R1 control peers.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;

use conduit_core::{
    BootId, CapabilityId, ConnectionBase, GearId, HostAdvertisement, HostProfileId,
    OfferGeneration, Plan, PROTOCOL_VERSION,
};
use conduit_planner::{plan_with_options, PlacementChoice, PlacementChoices, PlanningOptions};

use crate::{r1_signal_pico_advertisement, R1SignalRouteSet};

pub const R1_LEVEL_INPUT_CAPABILITY_ID: &str = "r1/std-level-input";
pub const R1_MERGE_CAPABILITY_ID: &str = "r1/std-merge-three-signal";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactR1ControlPlan {
    pub source_advertisement: HostAdvertisement,
    pub pico_advertisement: HostAdvertisement,
    pub route_set: R1SignalRouteSet,
    pub plan: Plan,
}

pub fn r1_control_source_advertisement() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: conduit_core::HostId::from(conduit_r1_network_conformance::R1_STD_HOST_ID),
        boot_id: BootId::from(conduit_r1_network_conformance::R1_STD_BOOT_ID),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("rust-std-r1-three-peer-control"),
        resources: vec![conduit_core::resource_offer(
            "r1/std-deliberate-inputs",
            conduit_core::INPUT_RESOURCE_CLASS,
            3,
        )],
        planner_capabilities: vec![],
        capabilities: vec![
            conduit_signal::level_input_capability(
                R1_LEVEL_INPUT_CAPABILITY_ID,
                "std/kernel-level-input-v1",
                3,
            ),
            conduit_signal::merge_three_signal_capability(
                R1_MERGE_CAPABILITY_ID,
                "std/kernel-merge-three-signal-v1",
            ),
        ],
    }
}

pub fn exact_r1_control_plan(
    pico_boot_id: BootId,
    route_set: R1SignalRouteSet,
) -> Result<ExactR1ControlPlan, String> {
    let source = r1_control_source_advertisement();
    let pico = r1_signal_pico_advertisement(pico_boot_id.clone());
    let observed_lines = conduit_r1_network_conformance::r1_line_basis(pico_boot_id);
    let plan = plan_r1_control(&source, &pico, &observed_lines, route_set)?;

    Ok(ExactR1ControlPlan {
        source_advertisement: source,
        pico_advertisement: pico,
        route_set,
        plan,
    })
}

fn plan_r1_control(
    source: &HostAdvertisement,
    pico: &HostAdvertisement,
    observed_lines: &[conduit_core::LineOffer; 2],
    route_set: R1SignalRouteSet,
) -> Result<Plan, String> {
    let form = conduit_form::parse_with_startup(
        include_str!("../../../fixtures/forms/r1-three-peer-control.conduit"),
        &conduit_signal::signal_startup_catalog(),
        &conduit_signal::signal_profile_catalog(),
    )
    .map_err(|error| error.to_string())?;
    let mut by_gear = BTreeMap::new();
    for gear in ["terminal", "browser-a", "browser-b"] {
        by_gear.insert(
            GearId::from(format!("r1-three-peer-control/{gear}")),
            PlacementChoice {
                host_id: source.host_id.clone(),
                capability_id: CapabilityId::from(R1_LEVEL_INPUT_CAPABILITY_ID),
            },
        );
    }
    by_gear.insert(
        GearId::from("r1-three-peer-control/merge"),
        PlacementChoice {
            host_id: source.host_id.clone(),
            capability_id: CapabilityId::from(R1_MERGE_CAPABILITY_ID),
        },
    );
    by_gear.insert(
        GearId::from("r1-three-peer-control/show"),
        PlacementChoice {
            host_id: pico.host_id.clone(),
            capability_id: CapabilityId::from(crate::R1_LED_CAPABILITY_ID),
        },
    );

    let selected_lines = match route_set {
        R1SignalRouteSet::UsbOnly => vec![observed_lines[0].clone()],
        R1SignalRouteSet::WebSocketOnly => vec![observed_lines[1].clone()],
        R1SignalRouteSet::WebSocketThenUsb => {
            vec![observed_lines[1].clone(), observed_lines[0].clone()]
        }
    };
    let mut allowed_bases = vec![ConnectionBase::Local];
    for base in selected_lines.iter().map(|line| line.binding.base) {
        if !allowed_bases.contains(&base) {
            allowed_bases.push(base);
        }
    }
    let line_candidates = BTreeMap::from([(
        (
            GearId::from("r1-three-peer-control/merge"),
            GearId::from("r1-three-peer-control/show"),
        ),
        selected_lines
            .iter()
            .map(|line| line.line_id.clone())
            .collect(),
    )]);
    let plan = plan_with_options(
        &form,
        &[source.clone(), pico.clone()],
        &PlacementChoices { by_gear },
        &allowed_bases,
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &line_candidates,
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_signal::SIGNAL_ENCODED_LEN,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &selected_lines,
        },
    )
    .map_err(|error| error.to_string())?;

    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    fn remote_connection(plan: &Plan) -> &conduit_core::PlannedConnection {
        plan.fragments
            .iter()
            .flat_map(|fragment| &fragment.connections)
            .find(|connection| !connection.admitted_lines.is_empty())
            .expect("one remote Pico LED Cord")
    }

    #[test]
    fn ordinary_plan_seals_three_inputs_one_merge_and_the_selected_pico_lines() {
        let exact = exact_r1_control_plan(
            BootId::from("r1/pico-runtime-boot"),
            R1SignalRouteSet::WebSocketThenUsb,
        )
        .unwrap();
        let source = exact
            .plan
            .fragments
            .iter()
            .find(|fragment| fragment.host_id == exact.source_advertisement.host_id)
            .unwrap();
        assert_eq!(source.placements.len(), 4);
        assert_eq!(source.connections.len(), 4);
        let level_gears = source
            .placements
            .iter()
            .filter(|placement| placement.kind_id.as_str() == conduit_signal::LEVEL_INPUT_KIND)
            .map(|placement| placement.gear_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            level_gears,
            [
                "r1-three-peer-control/browser-a",
                "r1-three-peer-control/browser-b",
                "r1-three-peer-control/terminal",
            ]
        );
        assert!(source.placements.iter().any(|placement| {
            placement.kind_id.as_str() == conduit_signal::MERGE_THREE_SIGNAL_KIND
        }));
        let remote = remote_connection(&exact.plan);
        assert_eq!(remote.admitted_lines.len(), 2);
        assert_eq!(
            remote.admitted_lines[0].binding.base,
            ConnectionBase::WebSocket
        );
        assert_eq!(
            remote.admitted_lines[1].binding.base,
            ConnectionBase::UsbCdc
        );
    }

    #[test]
    fn input_slots_are_finite_and_not_confused_with_pico_lines() {
        let source = r1_control_source_advertisement();
        assert_eq!(source.resources.len(), 1);
        assert_eq!(
            source.resources[0].class_id.as_str(),
            conduit_core::INPUT_RESOURCE_CLASS
        );
        assert_eq!(source.resources[0].capacity_units, 3);
        let input = source
            .capabilities
            .iter()
            .find(|capability| capability.kind_id.as_str() == conduit_signal::LEVEL_INPUT_KIND)
            .unwrap();
        assert_eq!(input.limits.max_active_instances, 3);
        assert_eq!(input.host_operations.len(), 1);
        assert_eq!(
            input.host_operations[0].contract_id.as_str(),
            conduit_signal::AWAIT_LEVEL_HOST_OPERATION_CONTRACT
        );
        assert_eq!(input.host_operations[0].maximum_output_bytes, 1);
    }

    #[test]
    fn planner_rejects_two_input_slots_for_three_exact_input_gears() {
        let mut source = r1_control_source_advertisement();
        source.resources[0].capacity_units = 2;
        let boot = BootId::from("r1/pico-runtime-boot");
        let pico = r1_signal_pico_advertisement(boot.clone());
        let observed_links = conduit_r1_network_conformance::r1_line_basis(boot);
        assert!(plan_r1_control(
            &source,
            &pico,
            &observed_links,
            R1SignalRouteSet::WebSocketOnly,
        )
        .is_err());
    }
}
