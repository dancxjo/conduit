#![cfg(feature = "authoring")]

use conduit_core::{
    kind_id, port_id, verify_plan, ArtifactId, Back, BackOfferBuilder, BaseImplementationId,
    BootId, CapabilityId, CapabilityLimits, ExecutionProfileId, HostAdvertisement, HostId,
    HostProfileId, ImplementationId, OfferGeneration, PlannedActivationEntry, PortDirection,
    PROTOCOL_VERSION,
};
use conduit_planner::{
    default_expanded_placements, plan_expanded_authoring_with_activations, ConnectionQueueLimits,
    ForeBoundaryKey, PlanningOptions,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    CanonicalBackCatalog, ProfileCatalog, StartupCatalog,
};
use conduit_todo_plot::{
    install_todo_catalogs, todo_combine_kind, TodoState, COMMAND_MAX_BYTES, STATE_MAX_BYTES,
    TODO_COMBINE_KIND, TODO_COMMAND_INFO_ID, TODO_STATE_INFO_ID,
};
use std::collections::BTreeMap;

const SOURCE: &str = include_str!("../../live.conduit");
const SCAN_QUEUE_BYTES: u32 = (2 * STATE_MAX_BYTES + 2 * COMMAND_MAX_BYTES) as u32;

fn host(scan: &conduit_plot::CheckedGear, scan_queue_bytes: u32) -> HostAdvertisement {
    let combine = BackOfferBuilder::new(
        todo_combine_kind(),
        Back {
            capability_id: CapabilityId::from("todo/combine/proof"),
            execution_profile_id: ExecutionProfileId::from("todo/combine/proof@1"),
            implementation_id: ImplementationId::from("todo/combine/proof@1"),
            artifact_id: ArtifactId::from("todo/combine/proof@1"),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build();
    let coordinator = conduit_core::capability_offer_from_parts! {
        semantic_contract: scan.semantic_contract.clone(),
        startup_parameters: scan.startup_parameters.clone(),
        shorthand: scan.shorthand.clone(),
        capability_id: CapabilityId::from("flow/scan/todo-proof"),
        kind_id: scan.kind_id.clone(),
        kind_contract_revision: scan.kind_contract_revision.clone(),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("flow/scan/todo-proof@1"),
            implementation_id: ImplementationId::from("flow/scan/todo-proof@1"),
            artifact_id: ArtifactId::from("flow/scan/todo-proof@1"),
        },
        inputs: scan.inputs.clone(),
        outputs: scan.outputs.clone(),
        host_calls: vec![],
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 3,
            max_queue_bytes: scan_queue_bytes,
        },
    };
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("todo-proof-host"),
        boot_id: BootId::from("todo-proof-boot"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("todo-proof-profile"),
        bases: vec![],
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: vec![coordinator, combine],
    }
}

#[test]
fn authored_todo_scan_plans_exact_fore_child_back_and_initial_form() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_todo_catalogs(&mut startup, &mut profile, "Groceries").unwrap();
    let document = check_syntax_document(&parse_syntax_document(SOURCE), &startup).unwrap();
    let authoring = expand_canonical_plot_for_authoring(&document, "todo/main", &profile).unwrap();
    let scan_gear = &authoring.expanded.gears[0];
    let hosts = [host(scan_gear, SCAN_QUEUE_BYTES)];
    let placements = default_expanded_placements(&authoring.expanded, &hosts).unwrap();
    let empty_bases = BTreeMap::new();
    let empty_lines = BTreeMap::new();
    let options = PlanningOptions {
        connection_bases: &empty_bases,
        line_candidates: &empty_lines,
        connection_item_capacity: 1,
        connection_byte_capacity: SCAN_QUEUE_BYTES,
        authority_grants: &[],
        protected_resource_grants: &[],
        line_offers: &[],
    };
    let bounds = BTreeMap::from([
        (
            ForeBoundaryKey {
                direction: PortDirection::Input,
                front_port_id: port_id("commands"),
                track: conduit_core::ConnectionTrack::Payload,
            },
            ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: COMMAND_MAX_BYTES as u32,
            },
        ),
        (
            ForeBoundaryKey {
                direction: PortDirection::Output,
                front_port_id: port_id("states"),
                track: conduit_core::ConnectionTrack::Payload,
            },
            ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: STATE_MAX_BYTES as u32,
            },
        ),
    ]);
    let bases = [BaseImplementationId::from("conduit.base/local@1")];
    let plan = plan_expanded_authoring_with_activations(
        &document,
        &authoring,
        &profile,
        &CanonicalBackCatalog::new(),
        &hosts,
        &placements,
        &bases,
        options,
        &bounds,
    )
    .unwrap();
    assert!(verify_plan(&plan));
    assert_eq!(plan.fragments[0].fore_ports.len(), 2);
    assert!(plan.fragments[0].fore_ports.iter().any(|port| {
        port.front_port_id == port_id("commands")
            && port.value_kind == kind_id(TODO_COMMAND_INFO_ID)
            && port.direction == PortDirection::Input
    }));
    assert!(plan.fragments[0].fore_ports.iter().any(|port| {
        port.front_port_id == port_id("states")
            && port.value_kind == kind_id(TODO_STATE_INFO_ID)
            && port.direction == PortDirection::Output
    }));
    let PlannedActivationEntry::Scan(scan) = &plan.activations[0] else {
        panic!("Todo commands must select the planned scan")
    };
    assert_eq!(scan.limits.maximum_items, 64);
    assert_eq!(scan.limits.maximum_queue_bytes, SCAN_QUEUE_BYTES);
    assert_eq!(scan.selected_plan_id, scan.selected_plan.plan_id);
    assert_eq!(
        TodoState::decode_info(&scan.initial_accumulator).unwrap(),
        TodoState::new("Groceries".into()).unwrap()
    );
    assert_eq!(scan.selected_plan.fragments[0].fore_ports.len(), 3);
    assert!(scan
        .selected_plan
        .fragments
        .iter()
        .any(|fragment| fragment.placements.iter().any(|placement| {
            placement.kind_id == kind_id(TODO_COMBINE_KIND)
                && placement.implementation_id.as_str() == "todo/combine/proof@1"
        })));

    let undersized = [host(scan_gear, SCAN_QUEUE_BYTES - 1)];
    let undersized_placements =
        default_expanded_placements(&authoring.expanded, &undersized).unwrap();
    assert!(plan_expanded_authoring_with_activations(
        &document,
        &authoring,
        &profile,
        &CanonicalBackCatalog::new(),
        &undersized,
        &undersized_placements,
        &bases,
        options,
        &bounds,
    )
    .is_err());
}
