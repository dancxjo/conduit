//! Cooperative scripted transfers; actual Source and one production kernel.
use conduit_composite::{
    KernelCompositeHost, KernelCompositeStatus, KernelCompositeTerminal, KernelOperationRegistry,
};
use conduit_core::{
    PortDescriptor, ValuePayload, bind_active_play, validate_canonical_structured_value,
};
use conduit_kernel::{HostCallDisposition, HostCallOutcome, NodeId};
use conduit_plan_lowering::lowering::lower_plan_fragment;
use conduitos::{
    expression_host_call::{self, ExpressionHostCall, ExpressionOperationFactory},
    structured_selector_host_call::{self, SelectorHostCall, SelectorOperationFactory},
    usb_base::{
        control_contract::ControlContract,
        control_factory::ControlOperationFactory,
        control_proof_plan::ControlProofSubject,
        control_request::ControlTransferRequest,
        control_result::{ControlTransferDisposition, PreparedControlResultEncoder},
        device_probe_proof_plan,
    },
};

enum PureOwner {
    Expression(ExpressionHostCall),
    Selector(SelectorHostCall),
}

fn run(actual: Option<u16>, expected_observed: &str, expected_decoded: Option<&str>) {
    let subject = ControlProofSubject {
        host_id: "host/device-script",
        boot_id: "boot/device-script",
        controller_base_id: "base/device-script",
        device_instance_id: "device/device-script",
        root_port: 1,
        slot: 1,
        attachment_epoch: 1,
    };
    let artifact = device_probe_proof_plan::prepare(&subject).unwrap();
    let definition = artifact.artifact().definition().clone();
    let fragment = &definition.internal_plan.fragments[0];
    let lowered = lower_plan_fragment(fragment).unwrap();
    let active = bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let mut pure = Vec::<(NodeId, PureOwner)>::new();
    for placement in &fragment.placements {
        let node = lowered
            .identity
            .placements
            .iter()
            .find(|(_, id)| id == &placement.placement_id)
            .unwrap()
            .0;
        let owner = match placement.implementation_id.as_str() {
            expression_host_call::IMPLEMENTATION => Some(PureOwner::Expression(
                ExpressionHostCall::prepare(fragment, &lowered, &active, &placement.placement_id)
                    .unwrap(),
            )),
            structured_selector_host_call::IMPLEMENTATION => Some(PureOwner::Selector(
                SelectorHostCall::prepare(fragment, &lowered, &active, &placement.placement_id)
                    .unwrap(),
            )),
            _ => None,
        };
        if let Some(owner) = owner {
            pure.push((node, owner));
        }
    }
    let input = definition.boundary.input_fronts[0].external_port.clone();
    let outputs: Vec<PortDescriptor> = definition
        .boundary
        .output_fronts
        .iter()
        .map(|front| front.external_port.clone())
        .collect();
    let mut registry = KernelOperationRegistry::new();
    registry
        .install(ExpressionOperationFactory::default())
        .unwrap();
    registry
        .install(SelectorOperationFactory::default())
        .unwrap();
    registry
        .install(ControlOperationFactory::prepare_contract().unwrap())
        .unwrap();
    let mut kernel = KernelCompositeHost::prepare(definition, &registry).unwrap();
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
    kernel.start().unwrap();
    kernel
        .admit_input(
            &input.port_id,
            0,
            &ValuePayload {
                value_kind: input.value_kind,
                encoded: vec![],
            },
        )
        .unwrap();
    kernel.close_input(&input.port_id).unwrap();
    for _ in 0..128 {
        let status = kernel.step().unwrap();
        if let Some(request) = kernel.next_host_request() {
            let obligation = kernel.host_request_obligation(&request).unwrap();
            let admitted = kernel
                .admit_host_request(
                    &request,
                    &obligation.host,
                    &obligation.resources,
                    &obligation.authorities,
                )
                .unwrap();
            let call = *kernel
                .admitted_host_request_view(&admitted)
                .unwrap()
                .request;
            let bytes = kernel.host_request_input(&admitted).unwrap();
            if let Some((_, owner)) = pure.iter_mut().find(|(node, _)| *node == call.node) {
                let result = match owner {
                    PureOwner::Expression(owner) => Some(
                        owner
                            .invoke(call.node, call.call, call.request, bytes)
                            .unwrap(),
                    ),
                    PureOwner::Selector(owner) => owner
                        .invoke(call.node, call.call, call.request, bytes)
                        .unwrap(),
                };
                match result {
                    Some(bytes) => kernel.complete_host_call_bytes(&admitted, bytes).unwrap(),
                    None => kernel
                        .complete_host_call(
                            &admitted,
                            HostCallOutcome {
                                disposition: HostCallDisposition::Completed,
                                output: None,
                                failure: None,
                            },
                        )
                        .unwrap(),
                }
            } else {
                assert_eq!(
                    obligation.requirement.contract_id.as_str(),
                    conduitos::usb_base::control_contract::CONTROL_CALL
                );
                transfers += 1;
                assert_eq!(transfers, 1, "hidden transfer or retry");
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
                kernel.complete_host_call_bytes(&admitted, &reply).unwrap();
            }
        }
        for (index, port) in outputs.iter().enumerate() {
            if let Some(sequence) = kernel
                .output_into(&port.port_id, &mut storage[index])
                .unwrap()
            {
                assert_eq!(sequence, 0);
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
                kernel.complete_output(&port.port_id, sequence).unwrap();
            }
        }
        if status == KernelCompositeStatus::Complete {
            complete = true;
            break;
        }
    }
    assert!(complete);
    assert_eq!(transfers, 1);
    for (index, port) in outputs.iter().enumerate() {
        assert_eq!(
            seen[index],
            port.port_id.as_str() == "observed" || expected_decoded.is_some()
        );
        assert_eq!(
            kernel
                .output_terminal_into(&port.port_id, &mut storage[index])
                .unwrap(),
            Some(KernelCompositeTerminal::Normal)
        );
    }
}

#[test]
fn checked_probe_constructs_request_decodes_reply_and_closes_in_one_kernel() {
    run(Some(18), "frame", Some("device"));
}

#[test]
fn short_or_stalled_transfer_remains_observed_and_never_decodes_or_retries() {
    run(Some(0), "short", None);
    run(Some(17), "short", None);
    run(None, "stalled", None);
}
