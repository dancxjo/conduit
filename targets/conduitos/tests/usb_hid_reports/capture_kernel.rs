//! Eight-call completion fixture through the production kernel; no device proof.
use conduit_composite::{
    KernelCompositeSignStorage, KernelCompositeStatus, KernelCompositeTerminal,
};
use conduit_core::*;
use conduit_kernel::scheduler::RemoteIngressOutcome;
use conduitos::usb_base::{
    endpoint_read_contract::EndpointReadContract,
    endpoint_read_offer::capture_offer,
    endpoint_read_proof_plan::EndpointReadProofSubject,
    endpoint_read_result::PreparedEndpointReadResultEncoder,
    hid_source_kernel::PreparedHidSourceKernel,
    hid_source_plan::{self, HidSourceRole},
};

#[path = "../usb_endpoint_capture/common.rs"]
mod endpoint_fixture;
use endpoint_fixture::host_and_grants;

#[test]
fn eight_live_endpoint_calls_complete_out_of_order_and_drain_without_growth() {
    let contract = EndpointReadContract::prepare().unwrap();
    let subject = EndpointReadProofSubject {
        host_id: "fixture/capture-kernel-host",
        boot_id: "fixture/capture-kernel-boot",
        controller_base_id: "fixture/capture-kernel-base",
        device_instance_id: "fixture/capture-kernel-device",
        root_port: 1,
        slot: 1,
        attachment_epoch: 1,
        endpoint_dci: 3,
        endpoint_epoch: 1,
    };
    let (mut host, mut grants) = host_and_grants(&contract, &subject).unwrap();
    host.capabilities[0] = capture_offer(
        &contract,
        "fixture/capture-kernel@1".into(),
        "fixture/capture-kernel-read".into(),
        8,
    )
    .unwrap();
    host.bases[0].capability_ids = vec![host.capabilities[0].capability_id.clone()];
    host.resources[0].capacity_units = 8;
    grants[0].capability_id = host.capabilities[0].capability_id.clone();
    let artifact =
        hid_source_plan::prepare(HidSourceRole::KeyboardCapture, &host, &grants).unwrap();
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
    let package = conduitos::protocol_source::usb_hid_keyboard_order_package().unwrap();
    let entry = conduitos::protocol_source::PreparedProtocolEntry::prepare(
        &serde_json::to_vec(&package).unwrap(),
        "usb-hid-keyboard-capture-window",
    )
    .unwrap();
    let changes = outputs
        .iter()
        .position(|p| p.port_id.as_str() == "changes")
        .unwrap();
    let decoder = conduitos::source_keyboard_batch::SourceKeyboardBatchDecoder::prepare(
        &entry.output_schema(&outputs[changes].port_id).unwrap(),
    )
    .unwrap();
    let mut buffers: Vec<_> = outputs
        .iter()
        .map(|p| ValuePayload {
            value_kind: p.value_kind.clone(),
            encoded: Vec::with_capacity(4096),
        })
        .collect();
    let unit = ValuePayload {
        value_kind: kind_id("value/unit"),
        encoded: vec![],
    };
    let mut encoder = PreparedEndpointReadResultEncoder::new(&contract).unwrap();
    let replies: Vec<_> = (0..8_u64)
        .map(|ordinal| {
            encoder
                .completed(ordinal, 8, 8, &[0, 0, 4 + ordinal as u8, 0, 0, 0, 0, 0])
                .unwrap()
                .to_vec()
        })
        .collect();
    let mut play = PreparedHidSourceKernel::prepare(
        artifact,
        KernelCompositeSignStorage {
            additional_local_items: 60_000,
            additional_remote_items: 60_000,
        },
    )
    .unwrap();
    let mut pending = [None; 8];
    let mut nodes = [None; 8];
    let mut issued = 0;
    let mut observations = 0;
    let mut batches = 0;
    let mut held_observation = 0;
    let mut held_batch = 0;
    let mut ended = false;
    let mut complete = false;
    let allocations = crate::allocation::allocations(|| {
        play.kernel_mut().start().unwrap();
        for input in &inputs {
            assert!(matches!(
                play.kernel_mut()
                    .admit_input(&input.port_id, 0, &unit)
                    .unwrap(),
                RemoteIngressOutcome::Accepted { .. }
            ));
            play.kernel_mut().close_input(&input.port_id).unwrap();
        }
        // Retain all eight admitted native calls before completing any of them.
        for _ in 0..8192 {
            play.kernel_mut().step().unwrap();
            if let Some(request) = play.kernel_mut().next_host_request()
                && !play.service_pure_call(&request).unwrap()
            {
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
                let node = kernel
                    .admitted_host_request_view(&admitted)
                    .unwrap()
                    .request
                    .node;
                assert!(!nodes[..issued].contains(&Some(node)));
                let input = validate_canonical_structured_value(
                    kernel.host_request_input(&admitted).unwrap(),
                )
                .unwrap();
                assert_eq!(
                    input
                        .record_field("length")
                        .unwrap()
                        .unwrap()
                        .primitive_bytes("value/u64")
                        .unwrap(),
                    8_u64.to_le_bytes()
                );
                nodes[issued] = Some(node);
                pending[issued] = Some(admitted);
                issued += 1;
                if issued == 8 {
                    break;
                }
            }
        }
        assert_eq!(issued, 8);
        for index in [7, 3, 0, 6, 1, 5, 2, 4] {
            play.kernel_mut()
                .complete_host_call_bytes(&pending[index].take().unwrap(), &replies[index])
                .unwrap();
        }
        for _ in 0..20_000 {
            let status = play.kernel_mut().step().unwrap();
            if let Some(request) = play.kernel_mut().next_host_request() {
                assert!(
                    play.service_pure_call(&request).unwrap(),
                    "no undeclared read or retry"
                );
            }
            for (index, output) in outputs.iter().enumerate() {
                let Some(sequence) = play
                    .kernel_mut()
                    .output_into(&output.port_id, &mut buffers[index])
                    .unwrap()
                else {
                    continue;
                };
                match output.port_id.as_str() {
                    "observation" => {
                        assert_eq!(sequence, observations);
                        let value =
                            validate_canonical_structured_value(&buffers[index].encoded).unwrap();
                        assert_eq!(
                            value
                                .record_field("ordinal")
                                .unwrap()
                                .unwrap()
                                .primitive_bytes("value/u64")
                                .unwrap(),
                            observations.to_le_bytes()
                        );
                        assert!(
                            value
                                .record_field("observed")
                                .unwrap()
                                .unwrap()
                                .variant_payload("keyboard")
                                .unwrap()
                                .is_some()
                        );
                        if observations == 0 && held_observation < 128 {
                            held_observation += 1;
                            continue;
                        }
                        observations += 1;
                    }
                    "changes" => {
                        assert_eq!(sequence, batches);
                        if batches == 0 && held_batch < 128 {
                            held_batch += 1;
                            continue;
                        }
                        let batch = decoder.decode(&buffers[index].encoded).unwrap();
                        let transitions = batch.transitions();
                        assert_eq!(transitions.len(), if batches == 0 { 1 } else { 2 });
                        if batches != 0 {
                            assert_eq!(
                                (transitions[0].usage(), transitions[0].pressed()),
                                (3 + batches as u8, false)
                            );
                        }
                        let press = transitions.last().unwrap();
                        assert_eq!(
                            (press.usage(), press.pressed(), press.modifiers()),
                            (4 + batches as u8, true, 0)
                        );
                        batches += 1;
                    }
                    "ended" => {
                        assert!(!ended);
                        super::common::tag(&buffers[index].encoded, "closed");
                        ended = true;
                    }
                    _ => panic!("undeclared class output"),
                }
                play.kernel_mut()
                    .complete_output(&output.port_id, sequence)
                    .unwrap();
            }
            if status == KernelCompositeStatus::Complete {
                for (index, output) in outputs.iter().enumerate() {
                    assert_eq!(
                        play.kernel_mut()
                            .output_terminal_into(&output.port_id, &mut buffers[index])
                            .unwrap(),
                        Some(KernelCompositeTerminal::Normal)
                    );
                }
                complete = true;
                break;
            }
        }
    });
    assert_eq!(allocations, 0);
    assert!(complete && ended);
    assert_eq!((issued, observations, batches), (8, 8, 8));
    assert_eq!((held_observation, held_batch), (128, 128));
}
