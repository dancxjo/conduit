use std::collections::BTreeMap;

use conduit_core::{
    kind_id, port_id, process_owned_line_offer_with_limits, ArtifactId, BootId, CapabilityId,
    CapabilityLimits, CapabilityOffer, ConnectionBase, GrayScottParameters, HostAdvertisement,
    HostId, HostProfileId, ImplementationId, ImplementationOffer, KindContractRevision, LinkLimits,
    OfferGeneration, PortDescriptor, PortDirection, PortTemporal, ReactionDiffusionBoundaryState,
    ReactionDiffusionEvolveRequest, ReactionDiffusionFieldId, ReactionDiffusionFieldState,
    ReactionDiffusionPartition, ReactionDiffusionRegion, ReactionDiffusionRegionId,
    ReactionDiffusionRegionWork, PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form_with_backs, parse_syntax_document,
    CanonicalBackCatalog, KindDefinition, KindSignature, ProfileCatalog, StartupCatalog,
};
use conduit_planner::{
    plan_expanded_canonical_with_options, PlacementChoice, PlacementChoices, PlanningOptions,
};
use conduit_wire::{
    decode_session_frame, encode_session_frame_into, SessionBinding, SessionMachine,
    SessionMessage, SessionRole, WireError,
};

const FIELD: &str = "field/evolve";
const WORKER: &str = "field/evolve-region";
const JOIN: &str = "field/join-regions";
const STATE: &str = "conduit.info/reaction-diffusion-state@1";
const REQUEST: &str = "conduit.info/reaction-diffusion-request@1";
const BOUNDARY: &str = "conduit.info/reaction-diffusion-boundary@1";
const RESULT: &str = "conduit.info/reaction-diffusion-region-result@1";
const MAX_PAYLOAD: u32 = 58;
const MAX_FRAME: u32 = 4_096;
const FIELD_ID: ReactionDiffusionFieldId = ReactionDiffusionFieldId(*b"field-a2-line001");

#[test]
fn two_hosts_exchange_every_cross_boundary_over_exact_planned_lines() {
    let (form, plan) = distributed_plan();
    assert_eq!(form.realization_backs.len(), 1);
    assert_eq!(plan.fragments.len(), 2);
    assert_eq!(plan.realization_backs.len(), 1);
    let remote = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.connections)
        .filter(|connection| connection.selected_line.is_some())
        .collect::<Vec<_>>();
    assert_eq!(remote.len(), 2);

    let partition = unequal_partition();
    let mut distributed = initial();
    let mut direct = initial();
    for generation in 0..4 {
        let source = distributed.clone();
        let partitioned =
            conduit_core::partition_reaction_diffusion_generation(&source, partition.clone())
                .unwrap();
        let mut results = Vec::new();
        for destination in [ReactionDiffusionRegionId(10), ReactionDiffusionRegionId(20)] {
            let (contract, cells) = partitioned.region_work_basis(destination).unwrap();
            let mut work = ReactionDiffusionRegionWork::new(contract, cells).unwrap();
            for boundary in partitioned
                .boundaries
                .iter()
                .filter(|boundary| boundary.destination_region == destination)
            {
                let admitted = if boundary.source_region == destination {
                    boundary.clone()
                } else {
                    transfer_boundary(&plan, boundary, generation == 0).unwrap()
                };
                work.admit_boundary(admitted).unwrap();
            }
            results.push(work.evolve().unwrap());
        }
        distributed = conduit_core::join_evolved_reaction_diffusion_regions(
            FIELD_ID,
            generation,
            source.width,
            source.height,
            source.parameters,
            &partition,
            &results,
        )
        .unwrap();
        direct = direct
            .evolve_reference(ReactionDiffusionEvolveRequest {
                field_id: FIELD_ID,
                expected_generation: generation,
                generations: 1,
                admitted_cell_generations: 80,
            })
            .unwrap();
        assert_eq!(distributed.encode().unwrap(), direct.encode().unwrap());
        assert_eq!(source.generation, generation);
    }
}

#[test]
fn line_sessions_refuse_wrong_identity_order_size_and_late_traffic() {
    let (_, plan) = distributed_plan();
    let partitioned =
        conduit_core::partition_reaction_diffusion_generation(&initial(), unequal_partition())
            .unwrap();
    let boundary = partitioned
        .boundaries
        .iter()
        .find(|boundary| boundary.source_region != boundary.destination_region)
        .unwrap();
    assert_eq!(
        transfer_boundary(&plan, boundary, true),
        Ok(boundary.clone())
    );

    let binding = binding_for(&plan, boundary).unwrap();
    let mut source = SessionMachine::new(binding.clone(), SessionRole::Source).unwrap();
    let mut sink = SessionMachine::new(binding.clone(), SessionRole::Sink).unwrap();
    activate(&mut source, &mut sink);
    assert_eq!(
        source.admit_outbound(binding.frame(SessionMessage::Offered {
            sequence: 1,
            payload: &boundary.encode().unwrap(),
        })),
        Err(WireError::ReorderedFrame)
    );
    let oversized = vec![0; MAX_PAYLOAD as usize + 1];
    assert_eq!(
        source.admit_outbound(binding.frame(SessionMessage::Offered {
            sequence: 0,
            payload: &oversized,
        })),
        Err(WireError::OversizedPayload)
    );
    let mut wrong = binding.clone();
    wrong.attachment.line_id = conduit_core::LineId::from("wrong/line");
    assert_eq!(
        source.admit_outbound(wrong.frame(SessionMessage::Offered {
            sequence: 0,
            payload: &boundary.encode().unwrap(),
        })),
        Err(WireError::InvalidSession)
    );
    close(&binding, &mut source, &mut sink);
    assert_eq!(
        source.admit_outbound(binding.frame(SessionMessage::Offered {
            sequence: 0,
            payload: &[],
        })),
        Err(WireError::LateFrame)
    );
}

fn transfer_boundary(
    plan: &conduit_core::Plan,
    boundary: &ReactionDiffusionBoundaryState,
    prove_pressure: bool,
) -> Result<ReactionDiffusionBoundaryState, WireError> {
    let binding = binding_for(plan, boundary)?;
    let mut source = SessionMachine::new(binding.clone(), SessionRole::Source)?;
    let mut sink = SessionMachine::new(binding.clone(), SessionRole::Sink)?;
    activate(&mut source, &mut sink);
    let payload = boundary.encode().map_err(|_| WireError::InvalidState)?;
    let offered = binding.frame(SessionMessage::Offered {
        sequence: 0,
        payload: &payload,
    });
    source.admit_outbound(offered)?;
    sink.admit_inbound(offered)?;
    if prove_pressure {
        let pressure = binding.frame(SessionMessage::Pressure { sequence: 0 });
        sink.admit_outbound(pressure)?;
        source.admit_inbound(pressure)?;
        source.admit_outbound(offered)?;
        sink.admit_inbound(offered)?;
    }
    let mut encoded = vec![0; MAX_FRAME as usize];
    let length = encode_session_frame_into(offered, &mut encoded, MAX_PAYLOAD, MAX_FRAME)?;
    let decoded = decode_session_frame(&encoded[..length], MAX_PAYLOAD, MAX_FRAME)?;
    let SessionMessage::Offered {
        sequence: 0,
        payload: received,
    } = decoded.message
    else {
        return Err(WireError::InvalidState);
    };
    let accepted = binding.frame(SessionMessage::Accepted { sequence: 0 });
    sink.admit_outbound(accepted)?;
    source.admit_inbound(accepted)?;
    let delivered = binding.frame(SessionMessage::Delivered { sequence: 0 });
    sink.admit_outbound(delivered)?;
    source.admit_inbound(delivered)?;
    close(&binding, &mut source, &mut sink);
    ReactionDiffusionBoundaryState::decode(received).map_err(|_| WireError::InvalidState)
}

fn activate(source: &mut SessionMachine, sink: &mut SessionMachine) {
    let binding = source.binding().clone();
    for machine in [&mut *source, &mut *sink] {
        machine.admit_outbound(binding.hello_frame()).unwrap();
        machine.admit_inbound(binding.hello_frame()).unwrap();
        machine
            .admit_outbound(binding.frame(SessionMessage::Ready))
            .unwrap();
        machine
            .admit_inbound(binding.frame(SessionMessage::Ready))
            .unwrap();
    }
}

fn close(binding: &SessionBinding, source: &mut SessionMachine, sink: &mut SessionMachine) {
    let input_closed = binding.frame(SessionMessage::InputClosed { final_sequence: 0 });
    source.admit_outbound(input_closed).unwrap();
    sink.admit_inbound(input_closed).unwrap();
    let terminal = binding.frame(SessionMessage::Terminal {
        final_sequence: 0,
        disposition: conduit_wire::SessionTerminalDisposition::Completed,
    });
    source.admit_outbound(terminal).unwrap();
    sink.admit_inbound(terminal).unwrap();
    sink.admit_outbound(terminal).unwrap();
    source.admit_inbound(terminal).unwrap();
}

fn binding_for(
    plan: &conduit_core::Plan,
    boundary: &ReactionDiffusionBoundaryState,
) -> Result<SessionBinding, WireError> {
    let (source_host, sink_host) = if boundary.source_region == ReactionDiffusionRegionId(10) {
        ("host/west", "host/east")
    } else {
        ("host/east", "host/west")
    };
    let source = plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id.as_str() == source_host)
        .ok_or(WireError::InvalidSession)?;
    let sink = plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id.as_str() == sink_host)
        .ok_or(WireError::InvalidSession)?;
    let connection = source
        .connections
        .iter()
        .find(|connection| {
            connection
                .selected_line
                .as_ref()
                .is_some_and(|line| line.binding.sink.host_id.as_str() == sink_host)
        })
        .ok_or(WireError::InvalidSession)?;
    SessionBinding::from_planned_connection(
        plan.plan_id.clone(),
        source.fragment_id.clone(),
        sink.fragment_id.clone(),
        connection,
    )
}

fn distributed_plan() -> (conduit_form::ExpandedCanonicalForm, conduit_core::Plan) {
    let (startup, profile, field) = catalogs();
    let user = check_syntax_document(
        &parse_syntax_document("form field-step {\n evolve: field/evolve\n}\n"),
        &startup,
    )
    .unwrap();
    let back = check_syntax_document(
        &parse_syntax_document(&format!(
            "form field/evolve (\n state: {STATE} >\n request: {REQUEST} >\n > state: {STATE}\n) {{\n west: {WORKER}\n east: {WORKER}\n join: {JOIN}\n state > west.state\n request > west.request\n state > east.state\n request > east.request\n west.boundary > east.boundary\n east.boundary > west.boundary\n west.result > join.west\n east.result > join.east\n join.state > state\n}}\n"
        )),
        &startup,
    )
    .unwrap();
    let mut backs = CanonicalBackCatalog::new();
    backs.insert(&field, &back, FIELD).unwrap();
    let expanded = expand_canonical_form_with_backs(&user, "field-step", &profile, &backs).unwrap();
    let west = host("west", &[WORKER, JOIN], &profile);
    let east = host("east", &[WORKER], &profile);
    let placements = PlacementChoices {
        by_gear: expanded
            .gears
            .iter()
            .map(|gear| {
                let selected = if gear.gear_id.as_str().contains("/east") {
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
    let lines = [
        process_owned_line_offer_with_limits(
            "line/west-east",
            "binding/west-east",
            ConnectionBase::FixtureFrame,
            "fixture/west-east",
            &west,
            &east,
            limits,
        ),
        process_owned_line_offer_with_limits(
            "line/east-west",
            "binding/east-west",
            ConnectionBase::FixtureFrame,
            "fixture/east-west",
            &east,
            &west,
            limits,
        ),
    ];
    let plan = plan_expanded_canonical_with_options(
        &expanded,
        &[west, east],
        &placements,
        &[ConnectionBase::Local, ConnectionBase::FixtureFrame],
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
    let definitions = [
        definition(FIELD, &[STATE, REQUEST], &[STATE]),
        definition(WORKER, &[STATE, REQUEST, BOUNDARY], &[BOUNDARY, RESULT]),
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
    (startup, profile, definitions[0].clone())
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
        (FIELD, 0) | (WORKER, 0) => "state",
        (FIELD, 1) | (WORKER, 1) => "request",
        (WORKER, 2) => "boundary",
        (JOIN, 0) => "west",
        (JOIN, 1) => "east",
        _ => unreachable!(),
    }
}

fn output_name(kind: &str, index: usize) -> &'static str {
    match (kind, index) {
        (FIELD, 0) | (JOIN, 0) => "state",
        (WORKER, 0) => "boundary",
        (WORKER, 1) => "result",
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

fn initial() -> ReactionDiffusionFieldState {
    ReactionDiffusionFieldState::initialized(FIELD_ID, 8, 10, GrayScottParameters::REFERENCE, 1705)
        .unwrap()
}

fn unequal_partition() -> ReactionDiffusionPartition {
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
