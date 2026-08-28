use std::collections::BTreeMap;

use conduit_alife::{
    GrayScottParameters, ReactionDiffusionFieldId, ReactionDiffusionFieldState,
    ReactionDiffusionPartition, ReactionDiffusionRegion, ReactionDiffusionRegionId,
    REACTION_DIFFUSION_REQUEST_INFO_ID, REACTION_DIFFUSION_STATE_INFO_ID,
};
use conduit_core::{
    kind_id, port_id, process_owned_line_offer_with_limits, ArtifactId, BaseImplementationId,
    BootId, CapabilityId, CapabilityLimits, CapabilityOffer, HostAdvertisement, HostId,
    HostProfileId, ImplementationId, ImplementationOffer, KindContractRevision, LinkLimits,
    OfferGeneration, PortDescriptor, PortDirection, PortTemporal, PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form_with_backs, parse_syntax_document,
    CanonicalBackCatalog, KindDefinition, KindSignature, ProfileCatalog, StartupCatalog,
};
use conduit_planner::{
    plan_expanded_canonical_with_options, PlacementChoice, PlacementChoices, PlanningOptions,
};

pub const FIELD: &str = "field/evolve";
const PREPARE: &str = "field/prepare-region";
const WORKER: &str = "field/evolve-region";
const JOIN: &str = "field/join-regions";
const STATE: &str = REACTION_DIFFUSION_STATE_INFO_ID;
const REQUEST: &str = REACTION_DIFFUSION_REQUEST_INFO_ID;
pub const BOUNDARY: &str = "conduit.info/reaction-diffusion-boundary@1";
const WORK: &str = "conduit.info/reaction-diffusion-region-work@1";
const RESULT: &str = "conduit.info/reaction-diffusion-region-result@1";
const NEXT_STATE: &str = "next-state";
pub const MAX_PAYLOAD: u32 = 58;
pub const MAX_FRAME: u32 = 4_096;
pub const FIELD_ID: ReactionDiffusionFieldId = ReactionDiffusionFieldId(*b"field-a2-line001");

pub fn distributed_plan() -> (conduit_form::ExpandedCanonicalForm, conduit_core::Plan) {
    let (startup, profile, field) = catalogs();
    let user = check_syntax_document(
        &parse_syntax_document("form field-step {\n evolve: field/evolve\n}\n"),
        &startup,
    )
    .unwrap();
    let back = check_syntax_document(
        &parse_syntax_document(&format!(
            "form field/evolve (\n > state: {STATE}\n > request: {REQUEST}\n {NEXT_STATE}: {STATE} >\n) {{\n prepare-west: {PREPARE}\n prepare-east: {PREPARE}\n west: {WORKER}\n east: {WORKER}\n join: {JOIN}\n state > prepare-west.state\n request > prepare-west.request\n state > prepare-east.state\n request > prepare-east.request\n prepare-west.work > west.work\n prepare-east.work > east.work\n prepare-west.boundary > east.boundary\n prepare-east.boundary > west.boundary\n west.result > join.west\n east.result > join.east\n join.state > {NEXT_STATE}\n}}\n"
        )),
        &startup,
    )
    .unwrap();
    let mut backs = CanonicalBackCatalog::new();
    backs.insert(&field, &back, FIELD).unwrap();
    let expanded = expand_canonical_form_with_backs(&user, "field-step", &profile, &backs).unwrap();
    let west = host("west", &[PREPARE, WORKER, JOIN], &profile);
    let east = host("east", &[PREPARE, WORKER], &profile);
    let placements = PlacementChoices {
        by_gear: expanded
            .gears
            .iter()
            .map(|gear| {
                let selected = if gear.gear_id.as_str().ends_with("east") {
                    &east
                } else {
                    &west
                };
                let offer = selected
                    .capabilities
                    .iter()
                    .find(|offer| offer.kind_id == gear.kind_id)
                    .unwrap();
                (
                    gear.gear_id.clone(),
                    PlacementChoice {
                        host_id: selected.host_id.clone(),
                        capability_id: offer.capability_id.clone(),
                    },
                )
            })
            .collect(),
    };
    let limits = LinkLimits {
        maximum_in_flight_items: 1,
        maximum_payload_bytes: MAX_PAYLOAD,
        maximum_buffered_bytes: MAX_PAYLOAD,
        maximum_frame_bytes: MAX_FRAME,
    };
    let mut lines = [
        process_owned_line_offer_with_limits(
            "line/west-east",
            "binding/west-east",
            BaseImplementationId::from("conduit.proof/frame@1"),
            "fixture/west-east",
            &west,
            &east,
            limits,
        ),
        process_owned_line_offer_with_limits(
            "line/east-west",
            "binding/east-west",
            BaseImplementationId::from("conduit.proof/frame@1"),
            "fixture/east-west",
            &east,
            &west,
            limits,
        ),
    ];
    for line in &mut lines {
        line.contract = conduit_core::LineContract {
            scope: conduit_core::LineScope::LocalNetwork,
            traffic_shape: conduit_core::LineTrafficShape::Message,
            duplex: conduit_core::LineDuplex::FullDuplex,
            ordering: conduit_core::LineOrdering::Ordered,
            reliability: conduit_core::LineReliability::Reliable,
            continuation: conduit_core::LineContinuation::None,
            security: conduit_core::LineSecurity::PlaintextNetwork,
        };
    }
    let plan = plan_expanded_canonical_with_options(
        &expanded,
        &[west, east],
        &placements,
        &[
            BaseImplementationId::from("conduit.base/local@1"),
            BaseImplementationId::from("conduit.proof/frame@1"),
        ],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: MAX_PAYLOAD,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &lines,
        },
    )
    .unwrap();
    (expanded, plan)
}

fn catalogs() -> (StartupCatalog, ProfileCatalog, KindDefinition) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_alife::install_reaction_diffusion_catalogs(&mut startup, &mut profile).unwrap();
    let field = profile.get(&kind_id(FIELD)).unwrap().clone();
    let definitions = [
        definition(PREPARE, &[STATE, REQUEST], &[WORK, BOUNDARY]),
        definition(WORKER, &[WORK, BOUNDARY], &[RESULT]),
        definition(JOIN, &[RESULT, RESULT], &[STATE]),
    ];
    for definition in &definitions {
        startup
            .insert(KindSignature {
                kind: definition.kind_id.as_str().into(),
                startup_parameters: vec![],
            })
            .unwrap();
        profile.insert(definition.clone()).unwrap();
    }
    (startup, profile, field)
}

fn definition(kind: &str, inputs: &[&str], outputs: &[&str]) -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(format!("{kind}@1")),
        inputs: inputs
            .iter()
            .enumerate()
            .map(|(index, value)| port(input_name(kind, index), value, PortDirection::Input))
            .collect(),
        outputs: outputs
            .iter()
            .enumerate()
            .map(|(index, value)| port(output_name(kind, index), value, PortDirection::Output))
            .collect(),
        configuration: vec![],
    }
}

fn input_name(kind: &str, index: usize) -> &'static str {
    match (kind, index) {
        (FIELD, 0) | (PREPARE, 0) => "state",
        (FIELD, 1) | (PREPARE, 1) => "request",
        (WORKER, 0) => "work",
        (WORKER, 1) => "boundary",
        (JOIN, 0) => "west",
        (JOIN, 1) => "east",
        _ => unreachable!(),
    }
}

fn output_name(kind: &str, index: usize) -> &'static str {
    match (kind, index) {
        (FIELD, 0) => NEXT_STATE,
        (JOIN, 0) => "state",
        (PREPARE, 0) => "work",
        (PREPARE, 1) => "boundary",
        (WORKER, 0) => "result",
        _ => unreachable!(),
    }
}

fn port(name: &str, value: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value),
        direction,
        temporal: PortTemporal::Value,
    }
}

fn host(name: &str, kinds: &[&str], profile: &ProfileCatalog) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(format!("host/{name}")),
        boot_id: BootId::from(format!("boot/{name}")),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from(format!("std/a2-{name}@1")),
        resources: vec![],
        capabilities: kinds
            .iter()
            .map(|kind| {
                let definition = profile.get(&kind_id(kind)).unwrap();
                CapabilityOffer {
                    startup_parameters: vec![],
                    shorthand: None,
                    capability_id: CapabilityId::from(format!("{name}/{kind}")),
                    kind_id: definition.kind_id.clone(),
                    kind_contract_revision: definition.kind_contract_revision.clone(),
                    inputs: definition.inputs.clone(),
                    outputs: definition.outputs.clone(),
                    implementation: ImplementationOffer {
                        execution_profile_id: conduit_core::ExecutionProfileId::from(format!(
                            "std/a2-{name}@1"
                        )),
                        implementation_id: ImplementationId::from(format!("std/{name}/{kind}@1")),
                        artifact_id: ArtifactId::from(format!("std/a2-{name}-image@1")),
                    },
                    host_operations: vec![],
                    resource_requirements: vec![],
                    authority_requirements: vec![],
                    limits: CapabilityLimits {
                        max_active_instances: 2,
                        max_queue_items: 1,
                        max_queue_bytes: MAX_PAYLOAD,
                    },
                }
            })
            .collect(),
        planner_capabilities: vec![],
    }
}

pub fn initial() -> ReactionDiffusionFieldState {
    ReactionDiffusionFieldState::initialized(FIELD_ID, 8, 10, GrayScottParameters::REFERENCE, 1705)
        .unwrap()
}

pub fn unequal_partition() -> ReactionDiffusionPartition {
    ReactionDiffusionPartition {
        regions: vec![
            ReactionDiffusionRegion {
                region_id: ReactionDiffusionRegionId(10),
                origin_x: 0,
                origin_y: 0,
                width: 3,
                height: 10,
            },
            ReactionDiffusionRegion {
                region_id: ReactionDiffusionRegionId(20),
                origin_x: 3,
                origin_y: 0,
                width: 5,
                height: 10,
            },
        ],
    }
}
