//! Cooperative scripted transfers; actual Source and one production probe.kernel.
use conduit_composite::{
    KernelCompositeSignStorage, KernelCompositeStatus, KernelCompositeTerminal,
};
use conduit_core::{PortDescriptor, ValuePayload, validate_canonical_structured_value};
use conduitos::usb_base::{
    control_contract::ControlContract,
    control_proof_plan::ControlProofSubject,
    control_request::ControlTransferRequest,
    control_result::{ControlTransferDisposition, PreparedControlResultEncoder},
    device_probe_proof_kernel::PreparedDeviceProbeKernel,
};

fn run(actual: Option<u16>, expected_observed: &str, expected_decoded: Option<&str>, calls: u64) {
    let subject = ControlProofSubject {
        host_id: "host/device-script",
        boot_id: "boot/device-script",
        controller_base_id: "base/device-script",
        device_instance_id: "device/device-script",
        root_port: 1,
        slot: 1,
        attachment_epoch: 1,
    };
    let mut probe = PreparedDeviceProbeKernel::prepare(
        &subject,
        KernelCompositeSignStorage {
            additional_local_items: 4096,
            additional_remote_items: 512,
        },
    )
    .unwrap();
    let definition = probe.kernel.definition();
    let input = definition.boundary.input_fronts[0].external_port.clone();
    let outputs: Vec<PortDescriptor> = definition
        .boundary
        .output_fronts
        .iter()
        .map(|front| front.external_port.clone())
        .collect();
    let contract = ControlContract::prepare().unwrap();
    let mut encoder = PreparedControlResultEncoder::new(&contract).unwrap();
    let request = ControlTransferRequest::new([128, 6, 0, 1, 0, 0, 18, 0], &[], 256).unwrap();
    let wire = [18, 1, 0, 2, 0, 0, 0, 64, 0x27, 6, 1, 0, 0, 0, 0, 0, 0, 1];
    let reply = match actual {
        Some(count) => encoder
            .completed(&request, count, &wire[..count as usize])
            .unwrap()
            .to_vec(),
        None => encoder
            .disposition(ControlTransferDisposition::Stalled)
            .unwrap()
            .to_vec(),
    };
    let mut storage: Vec<ValuePayload> = outputs
        .iter()
        .map(|port| ValuePayload {
            value_kind: port.value_kind.clone(),
            encoded: Vec::with_capacity(4096),
        })
        .collect();
    let capacities: Vec<usize> = storage
        .iter()
        .map(|value| value.encoded.capacity())
        .collect();
    let mut seen = vec![false; outputs.len()];
    let mut transfers = 0;
    let mut complete = false;
    let pulse = ValuePayload {
        value_kind: input.value_kind.clone(),
        encoded: vec![],
    };
    probe.kernel.start().unwrap();
    for invocation in 0..calls {
        seen.fill(false);
        probe
            .kernel
            .admit_input(&input.port_id, invocation, &pulse)
            .unwrap();
        if invocation + 1 == calls {
            probe.kernel.close_input(&input.port_id).unwrap();
        }
        for _ in 0..128 {
            let status = probe.kernel.step().unwrap();
            if let Some(request) = probe.kernel.next_host_request() {
                if !probe.dispatch_pure(&request).unwrap() {
                    let obligation = probe.kernel.host_request_obligation(&request).unwrap();
                    let admitted = probe
                        .kernel
                        .admit_host_request(
                            &request,
                            &obligation.host,
                            &obligation.resources,
                            &obligation.authorities,
                        )
                        .unwrap();
                    let bytes = probe.kernel.host_request_input(&admitted).unwrap();
                    assert_eq!(
                        obligation.requirement.contract_id.as_str(),
                        conduitos::usb_base::control_contract::CONTROL_CALL
                    );
                    transfers += 1;
                    assert_eq!(transfers, invocation + 1, "hidden transfer or retry");
                    let value = validate_canonical_structured_value(bytes).unwrap();
                    assert_eq!(
                        value
                            .record_field("setup")
                            .unwrap()
                            .unwrap()
                            .primitive_bytes("value/u64")
                            .unwrap(),
                        [128, 6, 0, 1, 0, 0, 18, 0]
                    );
                    probe
                        .kernel
                        .complete_host_call_bytes(&admitted, &reply)
                        .unwrap();
                }
            }
            for (index, port) in outputs.iter().enumerate() {
                if let Some(sequence) = probe
                    .kernel
                    .output_into(&port.port_id, &mut storage[index])
                    .unwrap()
                {
                    assert_eq!(sequence, invocation);
                    assert!(!seen[index]);
                    seen[index] = true;
                    assert_eq!(storage[index].encoded.capacity(), capacities[index]);
                    let expected = if port.port_id.as_str() == "observed" {
                        expected_observed
                    } else {
                        expected_decoded.expect("unexpected decode of a refused transfer")
                    };
                    assert!(
                        validate_canonical_structured_value(&storage[index].encoded)
                            .unwrap()
                            .variant_payload(expected)
                            .unwrap()
                            .is_some()
                    );
                    if expected == "device" {
                        let device = validate_canonical_structured_value(&storage[index].encoded)
                            .unwrap()
                            .variant_payload("device")
                            .unwrap()
                            .unwrap();
                        for (field, expected) in [("vendor_id", 1575_u64), ("product_id", 1_u64)] {
                            assert_eq!(
                                device
                                    .record_field(field)
                                    .unwrap()
                                    .unwrap()
                                    .primitive_bytes("value/u64")
                                    .unwrap(),
                                expected.to_le_bytes()
                            );
                        }
                    }
                    probe
                        .kernel
                        .complete_output(&port.port_id, sequence)
                        .unwrap();
                }
            }
            if status == KernelCompositeStatus::Complete {
                complete = true;
                break;
            }
            if invocation + 1 < calls && seen.iter().all(|seen| *seen) {
                break;
            }
        }
        assert!(
            seen[outputs
                .iter()
                .position(|port| port.port_id.as_str() == "observed")
                .unwrap()]
        );
    }
    assert!(complete);
    assert_eq!(transfers, calls);
    for (index, port) in outputs.iter().enumerate() {
        assert_eq!(
            seen[index],
            port.port_id.as_str() == "observed" || expected_decoded.is_some()
        );
        assert_eq!(
            probe
                .kernel
                .output_terminal_into(&port.port_id, &mut storage[index])
                .unwrap(),
            Some(KernelCompositeTerminal::Normal)
        );
    }
}

#[test]
fn checked_probe_constructs_request_decodes_reply_and_closes_in_one_kernel() {
    run(Some(18), "frame", Some("device"), 1);
}

#[test]
fn short_or_stalled_transfer_remains_observed_and_never_decodes_or_retries() {
    run(Some(0), "short", None, 1);
    run(Some(17), "short", None, 1);
    run(None, "stalled", None, 1);
}

#[test]
fn descriptor_exchange_reuses_value_slots_across_64_calls() {
    run(Some(18), "frame", Some("device"), 64);
}
