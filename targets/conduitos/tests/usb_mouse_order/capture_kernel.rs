//! Two admitted calls through the production kernel; no physical possession proof.
use super::allocation;
#[path = "../usb_endpoint_capture/common.rs"]
mod endpoint_fixture;
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

#[test]
fn two_live_mouse_calls_wait_for_the_missing_ordinal_and_drain_under_pressure() {
    let contract = EndpointReadContract::prepare().unwrap();
    let subject = EndpointReadProofSubject {
        host_id: "fixture/mouse-capture-host",
        boot_id: "fixture/mouse-capture-boot",
        controller_base_id: "fixture/mouse-capture-base",
        device_instance_id: "fixture/mouse-capture-device",
        root_port: 1,
        slot: 1,
        attachment_epoch: 1,
        endpoint_dci: 3,
        endpoint_epoch: 1,
    };
    let (mut host, mut grants) = endpoint_fixture::host_and_grants(&contract, &subject).unwrap();
    host.capabilities[0] = capture_offer(
        &contract,
        "fixture/mouse-capture@1".into(),
        "fixture/mouse-capture-read".into(),
        2,
    )
    .unwrap();
    host.bases[0].capability_ids = vec![host.capabilities[0].capability_id.clone()];
    host.resources[0].capacity_units = 2;
    grants[0].capability_id = host.capabilities[0].capability_id.clone();
    let artifact = hid_source_plan::prepare(HidSourceRole::MouseCapture, &host, &grants).unwrap();
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
    let unit = ValuePayload {
        value_kind: kind_id("value/unit"),
        encoded: vec![],
    };
    let mut encoder = PreparedEndpointReadResultEncoder::new(&contract).unwrap();
    let replies = [
        encoder
            .completed(0, 8, 8, &[7, 127, 129, 0, 0, 0, 0, 0])
            .unwrap()
            .to_vec(),
        encoder.completed(1, 8, 2, &[0, 0]).unwrap().to_vec(),
    ];
    let mut play = PreparedHidSourceKernel::prepare(
        artifact,
        KernelCompositeSignStorage {
            additional_local_items: 60000,
            additional_remote_items: 60000,
        },
    )
    .unwrap();
    let mut pending = [None; 2];
    let mut nodes = [None; 2];
    let mut issued = 0;
    let mut observations = 0_u64;
    let mut held = 0;
    let mut ended = false;
    let mut complete = false;
    let allocations = allocation::allocations(|| {
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
                let request = validate_canonical_structured_value(
                    kernel.host_request_input(&admitted).unwrap(),
                )
                .unwrap();
                assert_eq!(
                    request
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
                if issued == 2 {
                    break;
                }
            }
        }
        assert_eq!(issued, 2);
        play.kernel_mut()
            .complete_host_call_bytes(&pending[1].take().unwrap(), &replies[1])
            .unwrap();
        // A completed invalid report still waits for its missing wire predecessor.
        for _ in 0..128 {
            assert_ne!(
                play.kernel_mut().step().unwrap(),
                KernelCompositeStatus::Complete
            );
            if let Some(request) = play.kernel_mut().next_host_request() {
                assert!(play.service_pure_call(&request).unwrap());
            }
            for (index, output) in outputs.iter().enumerate() {
                assert!(
                    play.kernel_mut()
                        .output_into(&output.port_id, &mut buffers[index])
                        .unwrap()
                        .is_none()
                );
            }
        }
        play.kernel_mut()
            .complete_host_call_bytes(&pending[0].take().unwrap(), &replies[0])
            .unwrap();
        for _ in 0..20000 {
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
                assert_eq!(output.port_id.as_str(), "event");
                let event = validate_canonical_structured_value(&buffers[index].encoded).unwrap();
                if let Some(sample) = event.variant_payload("sample").unwrap() {
                    assert_eq!(observations, 0);
                    assert_eq!(sequence, 0);
                    assert_eq!(
                        sample
                            .record_field("ordinal")
                            .unwrap()
                            .unwrap()
                            .primitive_bytes("value/u64")
                            .unwrap(),
                        0_u64.to_le_bytes()
                    );
                    let sample = sample.record_field("sample").unwrap().unwrap();
                    for (name, expected) in [
                        ("position-x", 1000000_i64),
                        ("position-y", 0),
                        ("delta-x", 508000),
                        ("delta-y", -508000),
                    ] {
                        assert_eq!(
                            sample
                                .record_field(name)
                                .unwrap()
                                .unwrap()
                                .primitive_bytes("value/i64")
                                .unwrap(),
                            expected.to_le_bytes()
                        );
                    }
                    assert_eq!(
                        sample
                            .record_field("sequence")
                            .unwrap()
                            .unwrap()
                            .primitive_bytes("value/u64")
                            .unwrap(),
                        1_u64.to_le_bytes()
                    );
                    if held < 128 {
                        held += 1;
                        continue;
                    }
                    observations += 1;
                } else if let Some(value) = event.variant_payload("observation").unwrap() {
                    assert_eq!(observations, 1);
                    assert_eq!(sequence, 1);
                    assert_eq!(
                        value
                            .record_field("ordinal")
                            .unwrap()
                            .unwrap()
                            .primitive_bytes("value/u64")
                            .unwrap(),
                        1_u64.to_le_bytes()
                    );
                    assert!(
                        value
                            .record_field("observed")
                            .unwrap()
                            .unwrap()
                            .variant_payload("short")
                            .unwrap()
                            .is_some()
                    );
                    observations += 1;
                } else {
                    assert!(!ended);
                    assert_eq!(observations, 2, "end cannot overtake held observations");
                    assert_eq!(sequence, 2);
                    assert!(
                        event
                            .variant_payload("ended")
                            .unwrap()
                            .unwrap()
                            .variant_payload("closed")
                            .unwrap()
                            .is_some()
                    );
                    ended = true;
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
    assert_eq!((issued, observations, held), (2, 2, 128));
}
