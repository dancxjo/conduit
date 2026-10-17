//! Whole prepared graph witness with canonical simulated Host Call results.
//! No native admission, physical stop, or storage settlement claim is made.
use conduit_composite::KernelCompositeStatus;
use conduit_core::*;
use conduit_kernel::{KernelEventKind, scheduler::RemoteIngressOutcome};
use conduitos::protocol_source::*;
#[path = "usb_bot_reentry/closure_advertisement.rs"]
mod advertisement;
#[path = "usb_bot_reentry/closure_execution.rs"]
mod execution;
#[allow(dead_code)]
#[path = "usb_bot_reentry/fixture.rs"]
mod fixture;
use fixture::*;

fn control_result(ty: &StructuredInfoType, data: &[u8]) -> StructuredInfoValue {
    variant(ty, "completed", |p| {
        record(p, |f| match f.name() {
            "transferred" => StructuredInfoValue::leaf(
                f.value_type().clone(),
                (data.len() as u16).to_le_bytes().to_vec(),
            )
            .unwrap(),
            "short" => boolean(f.value_type(), false),
            "input" => {
                let StructuredInfoTypeShape::Sequence { element, .. } = f.value_type().shape()
                else {
                    panic!("input sequence")
                };
                StructuredInfoValue::sequence(
                    f.value_type().clone(),
                    data.iter()
                        .map(|b| StructuredInfoValue::leaf(element.clone(), vec![*b]).unwrap())
                        .collect(),
                )
                .unwrap()
            }
            _ => panic!("control member"),
        })
    })
}

#[test]
fn whole_kernel_failed_clear_has_no_fresh_authority_or_usb_effect() {
    for count in [1usize, 2] {
        let specifications = serde_json::from_str::<Vec<ProtocolSpecializationRequest>>(
            include_str!("../plots/usb/mass-storage-read-reentry-specializations.json"),
        )
        .unwrap();
        let package = ProtocolSourcePackage::compile(
            include_str!("../plots/usb/mass-storage-read-reentry.conduit").into(),
            &specifications,
        )
        .unwrap();
        let mut run = execution::Execution::prepare(package);
        let input = run.kernel.definition().external_capability.inputs[0].clone();
        let output = run.kernel.definition().external_capability.outputs[0].clone();
        let storage =
            conduitos::storage_base::transaction_contract::BlockReadTransactionContract::prepare()
                .unwrap();
        let block = conduitos::storage_base::contract::BlockReadContract::prepare().unwrap();
        let control = conduitos::usb_base::control_contract::ControlContract::prepare().unwrap();
        let read =
            conduitos::usb_base::endpoint_read_contract::EndpointReadContract::prepare().unwrap();
        let write =
            conduitos::usb_base::endpoint_write_contract::EndpointWriteContract::prepare().unwrap();
        let value = ValuePayload {
            value_kind: input.value_kind,
            encoded: record(block.request_type(), |f| {
                scalar(
                    f.value_type(),
                    match f.name() {
                        "block_size" => 512,
                        "count" => 1,
                        "lba" => 0,
                        _ => panic!("request"),
                    },
                )
            })
            .canonical_bytes()
            .unwrap(),
        };
        let mut received = ValuePayload {
            value_kind: output.value_kind,
            encoded: Vec::with_capacity(4096),
        };
        let mut sent = 0;
        let mut closed = false;
        let mut controls = 0;
        let mut reads = 0;
        let mut writes = 0;
        let mut authorizations = 0;
        let mut retired = false;
        run.kernel.start().unwrap();
        for _ in 0..30000 {
            if sent < count
                && matches!(
                    run.kernel
                        .admit_input(&input.port_id, sent as u64, &value)
                        .unwrap(),
                    RemoteIngressOutcome::Accepted { .. }
                )
            {
                sent += 1;
            }
            if sent == count && !closed {
                run.kernel.close_input(&input.port_id).unwrap();
                closed = true;
            }
            let status = run.kernel.step().unwrap();
            while let Some(request) = run.kernel.next_host_request() {
                if run.service_pure(&request) {
                    continue;
                }
                let required = run.kernel.host_request_obligation(&request).unwrap();
                let admitted = run
                    .kernel
                    .admit_host_request(
                        &request,
                        &required.host,
                        &required.resources,
                        &required.authorities,
                    )
                    .unwrap();

                let response = match required.requirement.contract_id.as_str() {
                    "conduit.host/block-read-authorize@1" => {
                        authorizations += 1;
                        assert_eq!(authorizations, 1);
                        identity(storage.permit_type(), 0)
                    }
                    "conduit.host/usb-control@1" => {
                        controls += 1;
                        assert!(controls <= 4);
                        match controls {
                            1 => control_result(
                                control.result_type(),
                                &[
                                    9, 2, 32, 0, 1, 1, 5, 192, 0, 9, 4, 0, 0, 2, 8, 6, 80, 0, 7, 5,
                                    129, 2, 0, 2, 0, 7, 5, 2, 2, 0, 2, 0,
                                ],
                            ),
                            2 | 3 => control_result(control.result_type(), &[]),
                            4 => unit(control.result_type(), "stalled"),
                            _ => unreachable!(),
                        }
                    }
                    "conduit.host/usb-endpoint-write@1" => {
                        writes += 1;
                        assert_eq!(writes, 1);
                        variant(write.result_type(), "completed", |p| {
                            record(p, |f| scalar(f.value_type(), 31))
                        })
                    }
                    "conduit.host/usb-endpoint-read@2" => {
                        reads += 1;
                        assert_eq!(reads, 1);
                        unit(read.result_type(), "stalled")
                    }
                    other => panic!("unexpected effect after failed clear: {other}"),
                };
                run.kernel
                    .complete_host_call_bytes(&admitted, &response.canonical_bytes().unwrap())
                    .unwrap();
            }
            assert!(
                run.kernel
                    .output_into(&output.port_id, &mut received)
                    .unwrap()
                    .is_none(),
                "failed clear cannot fabricate settlement result"
            );
            if status == KernelCompositeStatus::Complete {
                retired = true;
                break;
            }
        }
        let signs = run.kernel.signs();
        let completed: Vec<_> = signs
            .values()
            .flatten()
            .filter(|e| e.kind == KernelEventKind::BackCompleted)
            .map(|e| e.node)
            .collect();
        let fragment = &run.kernel.definition().internal_plan.fragments[0];
        let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(fragment).unwrap();
        let waiting: Vec<_> = fragment
            .placements
            .iter()
            .filter_map(|p| {
                let node = lowered
                    .identity
                    .node_for_placement(&p.placement_id)
                    .unwrap();
                (!completed.contains(&node)).then_some((
                    p.placement_id.clone(),
                    p.implementation_id.clone(),
                    node,
                ))
            })
            .collect();
        let nodes: Vec<_> = fragment
            .placements
            .iter()
            .map(|p| {
                lowered
                    .identity
                    .node_for_placement(&p.placement_id)
                    .unwrap()
            })
            .collect();
        let retained = run
            .kernel
            .has_retained_host_call_for_nodes(&fragment.host_id, &nodes)
            .unwrap();
        let terminal = run
            .kernel
            .output_terminal_into(&output.port_id, &mut received)
            .unwrap();
        eprintln!("retained_host_calls={retained} external_terminal={terminal:?}");
        assert!(
            !retained,
            "every surfaced original Host Call must have retired"
        );
        // Event balances identify candidate retained queues without inventing
        // private scheduler occupancy. Fanout routes are counted per target.
        for placement in &fragment.placements {
            let node = lowered
                .identity
                .node_for_placement(&placement.placement_id)
                .unwrap();
            for port in 0..placement.outputs.len() {
                let port = conduit_kernel::PortId(port as u16);
                let routed = signs
                    .values()
                    .flatten()
                    .filter(|e| {
                        e.node == node
                            && e.port == Some(port)
                            && e.kind == KernelEventKind::ValueRouted
                    })
                    .count();
                let targets: Vec<_> = lowered
                    .cords
                    .iter()
                    .filter(|c| {
                        c.spec.source == (conduit_kernel::CordEndpoint::Local { node, port })
                    })
                    .collect();
                let consumed: usize = targets
                    .iter()
                    .filter(|c| c.spec.track == AssignedConnectionTrack::Payload)
                    .filter_map(|c| match c.spec.sink {
                        conduit_kernel::CordEndpoint::Local { node, port } => Some(
                            signs
                                .values()
                                .flatten()
                                .filter(|e| {
                                    e.node == node
                                        && e.port == Some(port)
                                        && e.kind == KernelEventKind::ValueConsumed
                                })
                                .count(),
                        ),
                        _ => None,
                    })
                    .sum();
                if routed != consumed {
                    eprintln!(
                        "route_balance {:?} port={port:?} routed={routed} consumed={consumed} targets={:?}",
                        placement.gear_id,
                        targets
                            .iter()
                            .map(|c| (&c.connection_id, c.spec.sink, c.spec.track))
                            .collect::<Vec<_>>()
                    );
                }
            }
        }
        eprintln!(
            "node4={:?}",
            fragment
                .placements
                .iter()
                .find(|p| lowered
                    .identity
                    .node_for_placement(&p.placement_id)
                    .unwrap()
                    == conduit_kernel::NodeId(4))
                .map(|p| &p.gear_id)
        );
        eprintln!("count={count} retired={retired} remaining={waiting:?}");
        assert_eq!((authorizations, controls, writes, reads), (1, 4, 1, 1));
        run.kernel.cancel().unwrap();
        assert!(retired, "graph closure witness: remaining={waiting:?}");
    }
}
