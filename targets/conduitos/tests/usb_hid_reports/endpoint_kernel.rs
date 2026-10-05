//! Deterministic endpoint completion fixture; this is not native device evidence.
use conduit_composite::KernelCompositeSignStorage;
use conduit_core::*;
use conduitos::usb_base::{
    endpoint_read_contract::EndpointReadContract,
    endpoint_read_proof_plan::EndpointReadProofSubject,
    endpoint_read_result::PreparedEndpointReadResultEncoder,
    hid_endpoint_proof_kernel::PreparedHidEndpointProofKernel, hid_endpoint_proof_plan,
};

#[test]
fn endpoint_completion_reaches_class_outputs_and_drains_under_pressure() {
    let subject = EndpointReadProofSubject {
        host_id: "proof/host",
        boot_id: "proof/boot",
        controller_base_id: "proof/controller",
        device_instance_id: "proof/device",
        root_port: 1,
        slot: 1,
        attachment_epoch: 1,
        endpoint_dci: 3,
        endpoint_epoch: 1,
    };
    for entry in ["usb-hid-keyboard-endpoint", "usb-hid-mouse-endpoint"] {
        for lost in [false, true] {
            let artifact = hid_endpoint_proof_plan::plan(&subject, entry).unwrap();
            let inputs = artifact
                .artifact()
                .definition()
                .external_capability
                .inputs
                .clone();
            let outputs = artifact
                .artifact()
                .definition()
                .external_capability
                .outputs
                .clone();
            let mut buffers: Vec<_> = outputs
                .iter()
                .map(|port| ValuePayload {
                    value_kind: port.value_kind.clone(),
                    encoded: Vec::with_capacity(4096),
                })
                .collect();
            let mut seen: Vec<_> = outputs
                .iter()
                .map(|port| lost && port.port_id.as_str() != "received")
                .collect();
            let contract = EndpointReadContract::prepare().unwrap();
            let mut encoder = PreparedEndpointReadResultEncoder::new(&contract).unwrap();
            let completed = if lost {
            encoder.disposition(conduitos::usb_base::endpoint_read_result::EndpointReadDisposition::ProviderLost).unwrap()
        } else {
            encoder.completed(8, 8, &[0, 0, 4, 0, 0, 0, 0, 0]).unwrap()
        }.to_vec();
            let mut play = PreparedHidEndpointProofKernel::prepare(
                artifact,
                KernelCompositeSignStorage {
                    additional_local_items: 4096,
                    additional_remote_items: 4096,
                },
            )
            .unwrap();
            play.kernel_mut().start().unwrap();
            for port in &inputs {
                let unit = ValuePayload {
                    value_kind: port.value_kind.clone(),
                    encoded: Vec::new(),
                };
                play.kernel_mut()
                    .admit_input(&port.port_id, 0, &unit)
                    .unwrap();
                play.kernel_mut().close_input(&port.port_id).unwrap();
            }
            let mut completed_once = false;
            for _ in 0..512 {
                play.kernel_mut().step().unwrap();
                if let Some(request) = play.kernel_mut().next_host_request() {
                    if !play.service_pure_call(&request).unwrap() {
                        assert!(!completed_once);
                        let kernel = play.kernel_mut();
                        let obligation = kernel.host_request_obligation(&request).unwrap();
                        let admitted = kernel
                            .admit_host_request(
                                &request,
                                &obligation.host,
                                &obligation.resources,
                                &obligation.authorities,
                            )
                            .unwrap();
                        kernel
                            .complete_host_call_bytes(&admitted, &completed)
                            .unwrap();
                        completed_once = true;
                        break;
                    }
                }
            }
            assert!(completed_once);
            let allocations = crate::allocation::allocations(|| {
                for step in 0..2048 {
                    play.kernel_mut().step().unwrap();
                    if let Some(request) = play.kernel_mut().next_host_request() {
                        assert!(play.service_pure_call(&request).unwrap());
                    }
                    for (index, port) in outputs.iter().enumerate() {
                        if let Some(sequence) = play
                            .kernel_mut()
                            .output_into(&port.port_id, &mut buffers[index])
                            .unwrap()
                        {
                            assert_eq!(sequence, 0);
                            if port.port_id.as_str() == "transition" && step < 128 {
                                continue;
                            }
                            assert!(!seen[index]);
                            let tag = match port.port_id.as_str() {
                                "received" => {
                                    if lost {
                                        "provider-lost"
                                    } else {
                                        "frame"
                                    }
                                }
                                "observed" => "keyboard",
                                "decoded" => "mouse",
                                "transition" => "key",
                                _ => panic!("unexpected output"),
                            };
                            if tag != "key" {
                                super::common::tag(&buffers[index].encoded, tag);
                            } else {
                                let value =
                                    validate_canonical_structured_value(&buffers[index].encoded)
                                        .unwrap();
                                assert_eq!(
                                    value
                                        .record_field("usage")
                                        .unwrap()
                                        .unwrap()
                                        .primitive_bytes("value/u8")
                                        .unwrap(),
                                    [4]
                                );
                                assert_eq!(
                                    value
                                        .record_field("pressed")
                                        .unwrap()
                                        .unwrap()
                                        .primitive_bytes("value/bool")
                                        .unwrap(),
                                    [1]
                                );
                                assert_eq!(
                                    value
                                        .record_field("modifiers")
                                        .unwrap()
                                        .unwrap()
                                        .primitive_bytes("value/u8")
                                        .unwrap(),
                                    [0]
                                );
                            }
                            seen[index] = true;
                            play.kernel_mut()
                                .complete_output(&port.port_id, sequence)
                                .unwrap();
                        }
                    }
                    if seen.iter().all(|seen| *seen) {
                        break;
                    }
                }
            });
            assert_eq!(allocations, 0);
            assert!(seen.iter().all(|seen| *seen));
            let mut complete = false;
            let allocations = crate::allocation::allocations(|| {
                for _ in 0..2048 {
                    let status = play.kernel_mut().step().unwrap();
                    if let Some(request) = play.kernel_mut().next_host_request() {
                        assert!(play.service_pure_call(&request).unwrap());
                    }
                    if status == conduit_composite::KernelCompositeStatus::Complete {
                        complete = true;
                        break;
                    }
                }
                for (index, port) in outputs.iter().enumerate() {
                    assert_eq!(
                        play.kernel_mut()
                            .output_terminal_into(&port.port_id, &mut buffers[index])
                            .unwrap(),
                        Some(conduit_composite::KernelCompositeTerminal::Normal)
                    );
                }
            });
            assert_eq!(allocations, 0);
            assert!(complete);
        }
    }
}
