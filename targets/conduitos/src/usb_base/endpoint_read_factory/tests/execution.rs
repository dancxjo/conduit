//! Actual kernel execution with a scripted, explicitly inert transfer result.
use super::*;
use alloc::vec::Vec;
use conduit_composite::*;
use conduit_plot::CompositeFrontTerminal;

fn prepared() -> (KernelCompositeHost, PortDescriptor, PortDescriptor) {
    prepared_with_sign_storage(KernelCompositeSignStorage {
        additional_local_items: 4096,
        additional_remote_items: 4096,
    })
    .unwrap()
}

fn prepared_with_sign_storage(
    sign_storage: KernelCompositeSignStorage,
) -> Result<(KernelCompositeHost, PortDescriptor, PortDescriptor), KernelCompositeError> {
    let plan = planned();
    let fragment = &plan.fragments[0];
    let gear = &fragment.placements[0];
    let mut boundary = KernelCompositeBoundary {
        input_fronts: vec![],
        output_fronts: vec![],
    };
    for fore in &fragment.fore_ports {
        let ports = if fore.direction == PortDirection::Input {
            &gear.inputs
        } else {
            &gear.outputs
        };
        let mut port = ports
            .iter()
            .find(|port| port.port_id == fore.gear_port_id)
            .unwrap()
            .clone();
        port.port_id = fore.front_port_id.clone();
        let binding = KernelCompositeFrontBinding {
            external_port: port,
            internal_child: fragment.host_id.clone(),
            internal_placement_id: fore.placement_id.clone(),
            internal_port_id: fore.gear_port_id.clone(),
            terminal: CompositeFrontTerminal::Independent,
        };
        if fore.direction == PortDirection::Input {
            boundary.input_fronts.push(binding)
        } else {
            boundary.output_fronts.push(binding)
        }
    }
    let input = boundary.input_fronts[0].external_port.clone();
    let output = boundary.output_fronts[0].external_port.clone();
    let mut semantic = gear.semantic_contract.clone();
    for law in &mut semantic.laws {
        if let KindSemanticLaw::ValueContracts(contracts) = law {
            for contract in contracts {
                contract.location = match contract.location {
                    FrontValueLocation::Input(_) => {
                        FrontValueLocation::Input(input.port_id.clone())
                    }
                    FrontValueLocation::Output(_) => {
                        FrontValueLocation::Output(output.port_id.clone())
                    }
                    _ => panic!("control has ordinary input/output contracts"),
                };
            }
        }
    }
    let external = conduit_core::capability_offer_from_parts! {
        semantic_contract:semantic, startup_parameters:vec![],shorthand:None,
        capability_id:"fixture/usb-endpoint-read-wrapper".into(), kind_id:gear.kind_id.clone(),kind_contract_revision:gear.kind_contract_revision.clone(),
        implementation:ImplementationOffer {execution_profile_id:"fixture/usb-wrapper".into(),implementation_id:"fixture/usb-wrapper".into(),artifact_id:"fixture/usb-wrapper".into()},
        inputs:vec![input.clone()],outputs:vec![output.clone()],host_calls:vec![],resource_requirements:vec![],authority_requirements:vec![],limits:gear.limits.clone(),
    };
    let definition = KernelCompositeDefinition {
        host_id: fragment.host_id.clone(),
        boot_id: fragment.boot_id.clone(),
        offer_generation: fragment.offer_generation,
        profile: "fixture/usb-endpoint-read".into(),
        external_capability: external,
        internal_plan: plan,
        boundary,
        failure_translation: FailureReason::CompositeCapabilityFailed,
    };
    let mut registry = KernelOperationRegistry::new();
    registry
        .install(EndpointReadOperationFactory::prepare_contract().unwrap())
        .unwrap();
    Ok((
        KernelCompositeHost::prepare_with_sign_storage(definition, &registry, sign_storage)?,
        input,
        output,
    ))
}

fn input(contract: &EndpointReadContract) -> Vec<u8> {
    StructuredInfoValue::record(
        contract.request_type().clone(),
        vec![
            StructuredFieldValue::new(
                "length",
                StructuredInfoValue::leaf(
                    StructuredInfoType::leaf(kind_id("value/u64")).unwrap(),
                    2048_u64.to_le_bytes().to_vec(),
                )
                .unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}

#[test]
fn packed_endpoint_results_cross_the_production_kernel_without_growth_or_replay() {
    use crate::usb_base::endpoint_read_result::PreparedEndpointReadResultEncoder;
    let contract = EndpointReadContract::prepare().unwrap();
    let (mut kernel, input_port, output_port) = prepared();
    let input = ValuePayload {
        value_kind: input_port.value_kind.clone(),
        encoded: input(&contract),
    };
    let mut encoder = PreparedEndpointReadResultEncoder::new(&contract).unwrap();
    let wire = core::array::from_fn::<_, 2048, _>(|n| n as u8);
    let result = encoder.completed(2048, 2048, &wire).unwrap().to_vec();
    let mut output = ValuePayload {
        value_kind: output_port.value_kind.clone(),
        encoded: Vec::with_capacity(4096),
    };
    let capacity = output.encoded.capacity();
    let allocations = crate::test_allocations::allocations(|| {
        kernel.start().unwrap();
        for sequence in 0..64 {
            kernel
                .admit_input(&input_port.port_id, sequence, &input)
                .unwrap();
            if sequence == 63 {
                kernel.close_input(&input_port.port_id).unwrap();
            }
            let request = (0..16)
                .find_map(|_| {
                    kernel.step().unwrap();
                    kernel.next_host_request()
                })
                .expect("one exact endpoint Host Call");
            let obligation = kernel.host_request_obligation(&request).unwrap();
            assert_eq!(
                obligation.requirement.contract_id.as_str(),
                ENDPOINT_READ_CALL
            );
            let admitted = kernel
                .admit_host_request(
                    &request,
                    &obligation.host,
                    &obligation.resources,
                    &obligation.authorities,
                )
                .unwrap();
            assert_eq!(kernel.host_request_input(&admitted).unwrap(), input.encoded);
            kernel.complete_host_call_bytes(&admitted, &result).unwrap();
            assert!(kernel.complete_host_call_bytes(&admitted, &result).is_err());
            let delivered = (0..16).find_map(|_| {
                kernel.step().unwrap();
                kernel
                    .output_into(&output_port.port_id, &mut output)
                    .unwrap()
            });
            assert_eq!(delivered, Some(sequence));
            assert_eq!(output.encoded, result);
            assert_eq!(output.encoded.capacity(), capacity);
            for _ in 0..8 {
                kernel.step().unwrap();
                assert!(kernel.next_host_request().is_none());
                assert_eq!(
                    kernel
                        .output_into(&output_port.port_id, &mut output)
                        .unwrap(),
                    Some(sequence)
                );
                assert_eq!(output.encoded, result);
            }
            kernel
                .complete_output(&output_port.port_id, sequence)
                .unwrap();
        }
        assert!((0..32).any(|_| kernel.step().unwrap() == KernelCompositeStatus::Complete));
    });
    assert_eq!(allocations, 0);
}

#[test]
fn cancellation_retires_an_outstanding_endpoint_call_and_rejects_late_results() {
    use crate::usb_base::endpoint_read_result::{
        EndpointReadDisposition, PreparedEndpointReadResultEncoder,
    };
    let contract = EndpointReadContract::prepare().unwrap();
    let (mut kernel, input_port, output_port) = prepared();
    let input = ValuePayload {
        value_kind: input_port.value_kind.clone(),
        encoded: input(&contract),
    };
    let mut encoder = PreparedEndpointReadResultEncoder::new(&contract).unwrap();
    kernel.start().unwrap();
    kernel.admit_input(&input_port.port_id, 0, &input).unwrap();
    let request = (0..16)
        .find_map(|_| {
            kernel.step().unwrap();
            kernel.next_host_request()
        })
        .unwrap();
    let obligation = kernel.host_request_obligation(&request).unwrap();
    let admitted = kernel
        .admit_host_request(
            &request,
            &obligation.host,
            &obligation.resources,
            &obligation.authorities,
        )
        .unwrap();
    kernel.cancel().unwrap();
    assert!(
        kernel
            .complete_host_call_bytes(
                &admitted,
                encoder
                    .disposition(EndpointReadDisposition::ProviderLost)
                    .unwrap()
            )
            .is_err()
    );
    assert!(kernel.admit_input(&input_port.port_id, 1, &input).is_err());
    assert!(kernel.next_host_request().is_none());
    assert!(kernel.complete_output(&output_port.port_id, 0).is_err());
}
