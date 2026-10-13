//! Actual checked ECM Source/Plan/Kernel lifecycle with simulated provider replies.
//! This does not claim native DMA, USB device effects, or network possession.
use conduit_composite::KernelCompositeStatus;
use conduit_core::*;
use conduit_kernel::scheduler::RemoteIngressOutcome;
use conduitos::protocol_source::*;
#[path = "usb_ecm_lifecycle/advertisement.rs"]
mod advertisement;
#[path = "usb_ecm_lifecycle/execution.rs"]
mod execution;
#[path = "usb_ecm_lifecycle/package.rs"]
mod package;
#[allow(dead_code)]
#[path = "usb_bot_reentry/fixture.rs"]
mod values;
use conduitos::usb_base::{
    control_contract::*, control_decode::PreparedControlRequestDecoder,
    control_result::PreparedControlResultEncoder, endpoint_read_contract::*,
    endpoint_read_request::PreparedEndpointReadRequestDecoder,
    endpoint_read_result::PreparedEndpointReadResultEncoder, endpoint_write_contract::*,
};
use values::{record, scalar, unit, variant};
const CONFIG: &[u8] = &[
    9, 2, 80, 0, 2, 1, 0, 128, 50, 9, 4, 0, 0, 1, 2, 6, 0, 0, 5, 36, 0, 16, 1, 5, 36, 6, 0, 1, 13,
    36, 15, 4, 0, 0, 0, 0, 234, 5, 0, 0, 0, 7, 5, 131, 3, 16, 0, 16, 9, 4, 1, 0, 0, 10, 0, 0, 0, 9,
    4, 1, 1, 2, 10, 0, 0, 0, 7, 5, 129, 2, 64, 0, 0, 7, 5, 2, 2, 64, 0, 0,
];
#[derive(Clone, Copy, Debug)]
enum Fault {
    CancelRead,
    CancelWrite,
    LostRead,
    LostWrite,
    ShortRead,
    OversizedRead,
    StaleRead,
    MalformedFrame,
    ShortWrite,
}
#[test]
fn pending_read_or_write_cancel_rejects_original_kernel_token_and_late_success() {
    for fault in [Fault::CancelRead, Fault::CancelWrite] {
        execute(fault);
    }
}
#[test]
fn terminal_transport_faults_fence_further_usb_and_network_delivery() {
    for fault in [
        Fault::LostRead,
        Fault::LostWrite,
        Fault::ShortRead,
        Fault::OversizedRead,
        Fault::StaleRead,
        Fault::MalformedFrame,
        Fault::ShortWrite,
    ] {
        execute(fault);
    }
}
fn execute(fault: Fault) {
    let package = package::package();
    let source = PreparedProtocolSource::prepare(package.clone()).unwrap();
    let start = source
        .checked
        .native_types
        .iter()
        .find(|t| t.name == "CdcEcmStart")
        .unwrap();
    let initial = record(&start.value_type, |f| {
        record(f.value_type(), |wire| {
            StructuredInfoValue::leaf(wire.value_type().clone(), vec![]).unwrap()
        })
    });
    let mut run = execution::Execution::prepare(package);
    let input = run.kernel.definition().external_capability.inputs[0].clone();
    let outputs = run.kernel.definition().external_capability.outputs.clone();
    let mut buffers: Vec<_> = outputs
        .iter()
        .map(|p| ValuePayload {
            value_kind: p.value_kind.clone(),
            encoded: Vec::with_capacity(4096),
        })
        .collect();
    let control = ControlContract::prepare().unwrap();
    let cd = PreparedControlRequestDecoder::new(&control).unwrap();
    let mut ce = PreparedControlResultEncoder::new(&control).unwrap();
    let read = EndpointReadContract::prepare().unwrap();
    let rd = PreparedEndpointReadRequestDecoder::new(&read).unwrap();
    let mut re = PreparedEndpointReadResultEncoder::new(&read).unwrap();
    let write = EndpointWriteContract::prepare().unwrap();
    let frame = conduitos::ethernet_frame::contract::EthernetFrameContract::prepare().unwrap();
    let writing = matches!(
        fault,
        Fault::CancelWrite | Fault::LostWrite | Fault::ShortWrite
    );
    let mut controls = 0;
    let mut transfers = 0;
    let mut queues = 0;
    let mut delivered = 0;
    let mut ended = 0;
    let mut terminal_sent = false;
    let mut retired = false;
    run.kernel.start().unwrap();
    assert!(matches!(
        run.kernel
            .admit_input(
                &input.port_id,
                0,
                &ValuePayload {
                    value_kind: input.value_kind.clone(),
                    encoded: initial.canonical_bytes().unwrap()
                }
            )
            .unwrap(),
        RemoteIngressOutcome::Accepted { .. }
    ));
    run.kernel.close_input(&input.port_id).unwrap();
    for _ in 0..12000 {
        let status = run.kernel.step().unwrap();
        while let Some(request) = run.kernel.next_host_request() {
            if run.service_pure(&request) {
                continue;
            }
            assert!(
                !terminal_sent,
                "{fault:?}: an effect escaped the terminal Source fence"
            );
            let obligation = run.kernel.host_request_obligation(&request).unwrap();
            let admitted = run
                .kernel
                .admit_host_request(
                    &request,
                    &obligation.host,
                    &obligation.resources,
                    &obligation.authorities,
                )
                .unwrap();
            let response = match obligation.requirement.contract_id.as_str() {
                CONTROL_CALL => {
                    let bytes = run.kernel.host_request_input(&admitted).unwrap();
                    let decoded = cd.decode(bytes).unwrap();
                    let request = decoded.request(256).unwrap();
                    let mut mac = vec![26, 3];
                    for b in b"020000000001" {
                        mac.extend([*b, 0]);
                    }
                    let data = match controls {
                        0 => CONFIG,
                        2 => mac.as_slice(),
                        _ => &[],
                    };
                    controls += 1;
                    ce.completed(&request, data.len() as u16, data)
                        .unwrap()
                        .to_vec()
                }
                conduitos::ethernet_frame::contract::FRAME_CALL => {
                    queues += 1;
                    let bytes = run.kernel.host_request_input(&admitted).unwrap();
                    assert_eq!(
                        validate_canonical_structured_value(bytes)
                            .unwrap()
                            .variant_tag()
                            .unwrap(),
                        "take-transmit",
                        "no malformed receive or failed transmit may reach network delivery/ack"
                    );
                    if writing {
                        let mut wire = vec![0x55; 14];
                        wire[..6].fill(255);
                        wire[6..12].copy_from_slice(&[2, 0, 0, 0, 0, 1]);
                        wire[12..14].copy_from_slice(&[8, 0]);
                        variant(frame.result_type(), "transmit", |p| {
                            record(p, |f| {
                                if f.name() == "ordinal" {
                                    scalar(f.value_type(), 0)
                                } else {
                                    StructuredInfoValue::leaf(f.value_type().clone(), wire.clone())
                                        .unwrap()
                                }
                            })
                        })
                        .canonical_bytes()
                        .unwrap()
                    } else {
                        unit(frame.result_type(), "pressure")
                            .canonical_bytes()
                            .unwrap()
                    }
                }
                ENDPOINT_READ_CALL | ENDPOINT_WRITE_CALL => {
                    transfers += 1;
                    assert_eq!(transfers, 1);
                    if matches!(fault, Fault::CancelRead | Fault::CancelWrite) {
                        let node = run
                            .kernel
                            .admitted_host_request_view(&admitted)
                            .unwrap()
                            .request
                            .node;
                        assert!(
                            run.kernel
                                .has_retained_host_call_for_nodes(
                                    run.kernel.child_identity(0).unwrap(),
                                    &[node]
                                )
                                .unwrap()
                        );
                        run.kernel.cancel().unwrap();
                        let late = if writing {
                            variant(write.result_type(), "completed", |p| {
                                record(p, |f| scalar(f.value_type(), 14))
                            })
                            .canonical_bytes()
                            .unwrap()
                        } else {
                            re.completed(0, 1515, 14, &[0; 14]).unwrap().to_vec()
                        };
                        assert!(run.kernel.cancellation_failures().is_empty());
                        for _ in 0..2 {
                            assert_eq!(
                                run.kernel.complete_host_call_bytes(&admitted, &late),
                                Err(conduit_composite::KernelCompositeError::InvalidLifecycle),
                                "original cancelled token must reject late success"
                            );
                        }
                        assert!(run.kernel.next_host_request().is_none());
                        assert_eq!(run.kernel.step().unwrap(), KernelCompositeStatus::Cancelled);
                        assert_eq!((controls, queues, delivered, ended), (5, 1, 0, 0));
                        return;
                    }
                    terminal_sent = true;
                    if writing {
                        if matches!(fault, Fault::LostWrite) {
                            unit(write.result_type(), "provider-lost")
                                .canonical_bytes()
                                .unwrap()
                        } else {
                            variant(write.result_type(), "completed", |p| {
                                record(p, |f| scalar(f.value_type(), 13))
                            })
                            .canonical_bytes()
                            .unwrap()
                        }
                    } else {
                        let length = rd
                            .decode(run.kernel.host_request_input(&admitted).unwrap(), 1515)
                            .unwrap();
                        assert_eq!(length, 1515);
                        if matches!(fault, Fault::LostRead) {
                            unit(read.result_type(), "provider-lost")
                                .canonical_bytes()
                                .unwrap()
                        } else {
                            let size = match fault {
                                Fault::ShortRead => 13,
                                Fault::OversizedRead => 1515,
                                _ => 14,
                            };
                            let mut wire = vec![0x55; size];
                            if size >= 14 {
                                wire[..6].fill(255);
                                wire[6..12].copy_from_slice(&[2, 0, 0, 0, 0, 1]);
                                wire[12..14].copy_from_slice(&[8, 0]);
                            }
                            if matches!(fault, Fault::MalformedFrame) {
                                wire[6] = 3;
                            }
                            re.completed(
                                u64::from(matches!(fault, Fault::StaleRead)),
                                length,
                                size as u16,
                                &wire,
                            )
                            .unwrap()
                            .to_vec()
                        }
                    }
                }
                other => panic!("unexpected provider {other}"),
            };
            run.kernel
                .complete_host_call_bytes(&admitted, &response)
                .unwrap();
        }
        for (port, buffer) in outputs.iter().zip(&mut buffers) {
            if let Some(sequence) = run.kernel.output_into(&port.port_id, buffer).unwrap() {
                if port.port_id.as_str() == "ended" {
                    ended += 1;
                    let result = validate_canonical_structured_value(&buffer.encoded).unwrap();
                    assert_eq!(
                        result.variant_tag().unwrap(),
                        if matches!(fault, Fault::LostRead | Fault::LostWrite) {
                            "provider-lost"
                        } else {
                            "malformed"
                        }
                    );
                } else {
                    let result = validate_canonical_structured_value(&buffer.encoded).unwrap();
                    if result.variant_tag().unwrap() == "frame" {
                        let queue = result.variant_payload("frame").unwrap().unwrap();
                        if queue.variant_tag().unwrap() == "accepted" {
                            delivered += 1;
                        }
                    }
                }
                run.kernel.complete_output(&port.port_id, sequence).unwrap();
            }
        }
        if status == KernelCompositeStatus::Complete {
            retired = true;
            break;
        }
    }
    assert!(retired, "{fault:?}: actual ECM graph must drain");
    assert_eq!(
        (controls, queues, transfers, delivered, ended),
        (5, 1, 1, 0, 1)
    );
    run.kernel.cancel().unwrap();
}
