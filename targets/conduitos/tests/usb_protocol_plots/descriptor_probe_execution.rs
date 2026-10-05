//! Shared scripted descriptor carrier through one production proof kernel.
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

#[derive(Clone, Copy)]
pub(super) enum Descriptor {
    Device,
    Configuration,
}

pub(super) fn run(
    descriptor: Descriptor,
    actual: Option<u16>,
    expected_observed: &str,
    expected_decoded: Option<&str>,
    calls: u64,
) {
    let cases =
        vec![(actual, expected_observed, expected_decoded); usize::try_from(calls).unwrap()];
    run_script(descriptor, &cases);
}

type Outcome<'a> = (Option<u16>, &'a str, Option<&'a str>);

pub(super) fn run_script(descriptor: Descriptor, cases: &[Outcome<'_>]) {
    assert!(!cases.is_empty());
    let calls = u64::try_from(cases.len()).unwrap();
    let subject = ControlProofSubject {
        host_id: "host/device-script",
        boot_id: "boot/device-script",
        controller_base_id: "base/device-script",
        device_instance_id: "device/device-script",
        root_port: 1,
        slot: 1,
        attachment_epoch: 1,
    };
    let storage = match descriptor {
        Descriptor::Device => KernelCompositeSignStorage {
            additional_local_items: 4096,
            additional_remote_items: 512,
        },
        Descriptor::Configuration => KernelCompositeSignStorage {
            additional_local_items:
                conduitos::usb_base::configuration_probe_proof_plan::ADDITIONAL_LOCAL_SIGN_ITEMS,
            additional_remote_items:
                conduitos::usb_base::configuration_probe_proof_plan::ADDITIONAL_REMOTE_SIGN_ITEMS,
        },
    };
    let mut probe = match descriptor {
        Descriptor::Device => PreparedDeviceProbeKernel::prepare(&subject, storage),
        Descriptor::Configuration => {
            PreparedDeviceProbeKernel::prepare_configuration(&subject, storage)
        }
    }
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
    let (setup, wire): ([u8; 8], &[u8]) = match descriptor {
        Descriptor::Device => (
            [128, 6, 0, 1, 0, 0, 18, 0],
            &[18, 1, 0, 2, 0, 0, 0, 64, 0x27, 6, 1, 0, 0, 0, 0, 0, 0, 1],
        ),
        Descriptor::Configuration => (
            [128, 6, 0, 2, 0, 0, 0, 1],
            &[
                9, 2, 25, 0, 1, 1, 0, 128, 50, 9, 4, 3, 0, 1, 3, 1, 1, 0, 7, 5, 129, 3, 8, 0, 10,
            ],
        ),
    };
    let request = ControlTransferRequest::new(setup, &[], 256).unwrap();
    let replies: Vec<_> = cases
        .iter()
        .map(|(actual, _, _)| match actual {
            Some(count) => encoder
                .completed(&request, *count, &wire[..usize::from(*count)])
                .unwrap()
                .to_vec(),
            None => encoder
                .disposition(ControlTransferDisposition::Stalled)
                .unwrap()
                .to_vec(),
        })
        .collect();
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
    let mut sequences = vec![0; outputs.len()];
    let mut transfers = 0;
    let mut complete = false;
    let pulse = ValuePayload {
        value_kind: input.value_kind.clone(),
        encoded: vec![],
    };
    let allocations = super::allocation::allocations(|| {
        probe.kernel.start().unwrap();
        for (at, (_, expected_observed, expected_decoded)) in cases.iter().enumerate() {
            let invocation = u64::try_from(at).unwrap();
            seen.fill(false);
            probe
                .kernel
                .admit_input(&input.port_id, invocation, &pulse)
                .unwrap();
            if invocation + 1 == calls {
                probe.kernel.close_input(&input.port_id).unwrap();
            }
            for _ in 0..512 {
                let status = probe.kernel.step().unwrap();
                if let Some(request) = probe.kernel.next_host_request()
                    && !probe.dispatch_pure(&request).unwrap()
                {
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
                        setup
                    );
                    probe
                        .kernel
                        .complete_host_call_bytes(&admitted, &replies[at])
                        .unwrap();
                }
                for (index, port) in outputs.iter().enumerate() {
                    if let Some(sequence) = probe
                        .kernel
                        .output_into(&port.port_id, &mut storage[index])
                        .unwrap()
                    {
                        assert_eq!(sequence, sequences[index]);
                        assert!(!seen[index]);
                        seen[index] = true;
                        assert_eq!(storage[index].encoded.capacity(), capacities[index]);
                        let expected = if port.port_id.as_str() == "observed" {
                            *expected_observed
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
                        if expected == "configuration" {
                            let topology =
                                validate_canonical_structured_value(&storage[index].encoded)
                                    .unwrap()
                                    .variant_payload("configuration")
                                    .unwrap()
                                    .unwrap();
                            for field in
                                ["interface_count", "endpoint_count", "distinct_interfaces"]
                            {
                                assert_eq!(
                                    topology
                                        .record_field(field)
                                        .unwrap()
                                        .unwrap()
                                        .primitive_bytes("value/u64")
                                        .unwrap(),
                                    1_u64.to_le_bytes()
                                );
                            }
                            let interface = topology
                                .record_field("interfaces")
                                .unwrap()
                                .unwrap()
                                .collection_index(0)
                                .unwrap()
                                .unwrap();
                            assert_eq!(
                                interface
                                    .record_field("number")
                                    .unwrap()
                                    .unwrap()
                                    .primitive_bytes("value/u8")
                                    .unwrap(),
                                [3]
                            );
                            let endpoint = topology
                                .record_field("endpoints")
                                .unwrap()
                                .unwrap()
                                .collection_index(0)
                                .unwrap()
                                .unwrap();
                            assert_eq!(
                                endpoint
                                    .record_field("address")
                                    .unwrap()
                                    .unwrap()
                                    .primitive_bytes("value/u8")
                                    .unwrap(),
                                [129]
                            );
                        }
                        if expected == "device" {
                            let device =
                                validate_canonical_structured_value(&storage[index].encoded)
                                    .unwrap()
                                    .variant_payload("device")
                                    .unwrap()
                                    .unwrap();
                            for (field, expected) in
                                [("vendor_id", 1575_u64), ("product_id", 1_u64)]
                            {
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
                        sequences[index] += 1;
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
            for (index, port) in outputs.iter().enumerate() {
                assert_eq!(
                    seen[index],
                    port.port_id.as_str() == "observed" || expected_decoded.is_some()
                );
            }
        }
        assert!(complete);
        assert_eq!(transfers, calls);
        for (index, port) in outputs.iter().enumerate() {
            assert_eq!(
                probe
                    .kernel
                    .output_terminal_into(&port.port_id, &mut storage[index])
                    .unwrap(),
                Some(KernelCompositeTerminal::Normal)
            );
        }
    });
    assert_eq!(
        allocations, 0,
        "descriptor Play must reuse all admitted storage"
    );
}
