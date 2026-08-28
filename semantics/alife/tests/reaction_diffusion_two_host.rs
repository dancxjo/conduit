use conduit_alife::{
    ReactionDiffusionBoundaryState, ReactionDiffusionEvolveRequest, ReactionDiffusionRegionId,
    ReactionDiffusionRegionWork,
};
use conduit_wire::{
    decode_session_frame, encode_session_frame_into, SessionBinding, SessionMachine,
    SessionMessage, SessionRole, WireError,
};

#[path = "reaction_diffusion_two_host/common.rs"]
mod common;
use common::*;

#[test]
fn two_hosts_exchange_every_cross_boundary_over_exact_planned_lines() {
    let (form, plan) = distributed_plan();
    let plan_snapshot = plan.clone();
    assert_eq!(form.realization_backs.len(), 1);
    assert_eq!(plan.fragments.len(), 2);
    assert_eq!(plan.realization_backs.len(), 1);
    let remote = plan
        .fragments
        .iter()
        .flat_map(|fragment| {
            fragment
                .connections
                .iter()
                .map(move |connection| (fragment, connection))
        })
        .filter(|(fragment, connection)| {
            connection.value_kind.as_str() == BOUNDARY
                && connection
                    .selected_line
                    .as_ref()
                    .is_some_and(|line| line.binding.source.host_id == fragment.host_id)
        })
        .collect::<Vec<_>>();
    assert_eq!(remote.len(), 2, "{:#?}", plan.fragments);

    let partition = unequal_partition();
    let mut distributed = initial();
    let mut direct = initial();
    for generation in 0..4 {
        let source = distributed.clone();
        let partitioned =
            conduit_alife::partition_reaction_diffusion_generation(&source, partition.clone())
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
        distributed = conduit_alife::join_evolved_reaction_diffusion_regions(
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
    assert_eq!(plan, plan_snapshot);
}

#[test]
fn line_sessions_refuse_wrong_identity_order_size_and_late_traffic() {
    let (_, plan) = distributed_plan();
    let partitioned =
        conduit_alife::partition_reaction_diffusion_generation(&initial(), unequal_partition())
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
    assert_eq!(binding.source.host_id.as_str(), "host/east");
    assert_eq!(binding.sink.host_id.as_str(), "host/west");
    assert_eq!(binding.limits.maximum_in_flight_items, 1);
    assert_eq!(binding.limits.maximum_payload_bytes, MAX_PAYLOAD);
    assert_eq!(binding.attachment.limits.maximum_frame_bytes, MAX_FRAME);
    assert_eq!(
        sink.admit_outbound(binding.frame(SessionMessage::Offered {
            sequence: 0,
            payload: &boundary.encode().unwrap(),
        })),
        Err(WireError::ReorderedFrame)
    );
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
    wrong.connection_id = conduit_core::ConnectionId::from("wrong/session");
    assert_eq!(
        source.admit_outbound(wrong.frame(SessionMessage::Offered {
            sequence: 0,
            payload: &boundary.encode().unwrap(),
        })),
        Err(WireError::ConnectionMismatch)
    );
    let mut wrong_line = binding.clone();
    wrong_line.attachment.line_id = conduit_core::LineId::from("wrong/line");
    let mut unopened = SessionMachine::new(binding.clone(), SessionRole::Source).unwrap();
    assert_eq!(
        unopened.admit_outbound(wrong_line.hello_frame()),
        Err(WireError::SessionEpochMismatch)
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
    let final_sequence = source.next_sequence();
    let input_closed = binding.frame(SessionMessage::InputClosed { final_sequence });
    source.admit_outbound(input_closed).unwrap();
    sink.admit_inbound(input_closed).unwrap();
    let terminal = binding.frame(SessionMessage::Terminal {
        final_sequence,
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
            connection.value_kind.as_str() == BOUNDARY
                && connection.selected_line.as_ref().is_some_and(|line| {
                    line.binding.source.host_id.as_str() == source_host
                        && line.binding.sink.host_id.as_str() == sink_host
                })
        })
        .ok_or(WireError::InvalidSession)?;
    SessionBinding::from_planned_connection(
        plan.plan_id.clone(),
        source.fragment_id.clone(),
        sink.fragment_id.clone(),
        connection,
    )
}
