//! Actual kernel execution with a scripted, explicitly inert transfer result.
use super::*;
use crate::usb_base::control_owner::NativeControlObservation;
use alloc::vec::Vec;
use conduit_composite::*;
use conduit_plot::CompositeFrontTerminal;

fn prepared() -> (KernelCompositeHost, PortDescriptor, PortDescriptor) {
    prepared_with_sign_storage(KernelCompositeSignStorage::default()).unwrap()
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
        capability_id:"fixture/usb-control-wrapper".into(), kind_id:gear.kind_id.clone(),kind_contract_revision:gear.kind_contract_revision.clone(),
        implementation:ImplementationOffer {execution_profile_id:"fixture/usb-wrapper".into(),implementation_id:"fixture/usb-wrapper".into(),artifact_id:"fixture/usb-wrapper".into()},
        inputs:vec![input.clone()],outputs:vec![output.clone()],host_calls:vec![],resource_requirements:vec![],authority_requirements:vec![],limits:gear.limits.clone(),
    };
    let definition = KernelCompositeDefinition {
        host_id: fragment.host_id.clone(),
        boot_id: fragment.boot_id.clone(),
        offer_generation: fragment.offer_generation,
        profile: "fixture/usb-control".into(),
        external_capability: external,
        internal_plan: plan,
        boundary,
        failure_translation: FailureReason::CompositeCapabilityFailed,
    };
    let mut registry = KernelOperationRegistry::new();
    registry
        .install(ControlOperationFactory::prepare_contract().unwrap())
        .unwrap();
    Ok((
        KernelCompositeHost::prepare_with_sign_storage(definition, &registry, sign_storage)?,
        input,
        output,
    ))
}

fn input(contract: &ControlContract) -> Vec<u8> {
    let StructuredInfoTypeShape::Record { fields, .. } = contract.request_type().shape() else {
        panic!("request")
    };
    let ty = |name| {
        fields
            .iter()
            .find(|f| f.name() == name)
            .unwrap()
            .value_type()
            .clone()
    };
    StructuredInfoValue::record(
        contract.request_type().clone(),
        vec![
            StructuredFieldValue::new(
                "setup",
                StructuredInfoValue::leaf(ty("setup"), vec![128, 6, 0, 1, 0, 0, 8, 0]).unwrap(),
            )
            .unwrap(),
            StructuredFieldValue::new(
                "output",
                StructuredInfoValue::sequence(ty("output"), vec![]).unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}

#[test]
fn checked_control_plot_reuses_bounded_output_without_replaying_completed_calls() {
    let contract = ControlContract::prepare().unwrap();
    let mut owner = super::possession::owner(&planned());
    let (mut kernel, input_port, output_port) = prepared();
    kernel.start().unwrap();
    let bytes = input(&contract);
    kernel
        .admit_input(
            &input_port.port_id,
            0,
            &ValuePayload {
                value_kind: input_port.value_kind.clone(),
                encoded: bytes.clone(),
            },
        )
        .unwrap();
    let mut request = None;
    for _ in 0..16 {
        kernel.step().unwrap();
        if let Some(next) = kernel.next_host_request() {
            request = Some(next);
            break;
        }
    }
    let request = request.expect("control Host Call");
    assert_eq!(
        kernel
            .host_request_obligation(&request)
            .unwrap()
            .requirement
            .contract_id
            .as_str(),
        CONTROL_CALL
    );
    let obligation = kernel.host_request_obligation(&request).unwrap().clone();
    let mut stale = obligation.host.clone();
    stale.boot_id = "boot/stale".into();
    assert!(
        kernel
            .admit_host_request(
                &request,
                &stale,
                &obligation.resources,
                &obligation.authorities
            )
            .is_err()
    );
    assert!(
        kernel
            .admit_host_request(&request, &obligation.host, &[], &obligation.authorities)
            .is_err()
    );
    assert!(
        kernel
            .admit_host_request(&request, &obligation.host, &obligation.resources, &[])
            .is_err()
    );
    let admitted = kernel
        .admit_host_request(
            &request,
            &obligation.host,
            &obligation.resources,
            &obligation.authorities,
        )
        .unwrap();
    assert_eq!(kernel.host_request_input(&admitted).unwrap(), bytes);
    let forged = KernelCompositeHostRequest {
        dispatch_token: request.dispatch_token + 1,
    };
    assert!(kernel.host_request_view(&forged).is_err());
    let call = *kernel
        .admitted_host_request_view(&admitted)
        .unwrap()
        .request;
    let submission = owner
        .begin(
            call.node,
            call.call,
            call.request,
            kernel.host_request_input(&admitted).unwrap(),
        )
        .unwrap();
    assert_eq!(submission.request().unwrap().length(), 8);
    // SAFETY: scripted acknowledgement, no physical transfer is outstanding.
    let result = unsafe {
        owner.finish_quiesced(
            &submission,
            NativeControlObservation::Completed {
                actual: 8,
                input: &[18, 1, 0, 2, 0, 0, 0, 64],
            },
        )
    }
    .unwrap()
    .to_vec();
    kernel.complete_host_call_bytes(&admitted, &result).unwrap();
    assert!(kernel.complete_host_call_bytes(&admitted, &result).is_err());
    let mut output = ValuePayload {
        value_kind: output_port.value_kind,
        encoded: Vec::with_capacity(CONTROL_MAXIMUM_BYTES as usize),
    };
    let capacity = output.encoded.capacity();
    let mut sequence = None;
    for _ in 0..16 {
        kernel.step().unwrap();
        sequence = kernel
            .output_into(&output_port.port_id, &mut output)
            .unwrap();
        if sequence.is_some() {
            break;
        }
    }
    assert_eq!(sequence, Some(0));
    assert_eq!(output.encoded, result);
    assert_eq!(output.encoded.capacity(), capacity);
    for _ in 0..32 {
        kernel.step().unwrap();
        assert!(kernel.next_host_request().is_none());
    }
    kernel.complete_output(&output_port.port_id, 0).unwrap();
    kernel
        .admit_input(
            &input_port.port_id,
            1,
            &ValuePayload {
                value_kind: input_port.value_kind.clone(),
                encoded: bytes,
            },
        )
        .unwrap();
    kernel.close_input(&input_port.port_id).unwrap();
    let mut next = None;
    for _ in 0..16 {
        kernel.step().unwrap();
        if let Some(request) = kernel.next_host_request() {
            next = Some(request);
            break;
        }
    }
    let next = next.unwrap();
    let admitted_next = kernel
        .admit_host_request(
            &next,
            &obligation.host,
            &obligation.resources,
            &obligation.authorities,
        )
        .unwrap();
    let next_call = *kernel
        .admitted_host_request_view(&admitted_next)
        .unwrap()
        .request;
    assert_ne!(next_call.request, call.request);
    let submission = owner
        .begin(
            next_call.node,
            next_call.call,
            next_call.request,
            kernel.host_request_input(&admitted_next).unwrap(),
        )
        .unwrap();
    // SAFETY: inert acknowledgement of a short completed transfer.
    let result = unsafe {
        owner.finish_quiesced(
            &submission,
            NativeControlObservation::Completed {
                actual: 0,
                input: &[],
            },
        )
    }
    .unwrap();
    kernel
        .complete_host_call_bytes(&admitted_next, result)
        .unwrap();
    let expected = result.to_vec();
    let mut sequence = None;
    for _ in 0..16 {
        kernel.step().unwrap();
        sequence = kernel
            .output_into(&output_port.port_id, &mut output)
            .unwrap();
        if sequence.is_some() {
            break;
        }
    }
    assert_eq!(sequence, Some(1));
    assert_eq!(output.encoded, expected);
    assert_eq!(output.encoded.capacity(), capacity);
    kernel.complete_output(&output_port.port_id, 1).unwrap();
}

#[test]
fn cancellation_invalidates_a_pending_kernel_dispatch() {
    let contract = ControlContract::prepare().unwrap();
    let mut owner = super::possession::owner(&planned());
    let (mut kernel, input_port, _) = prepared();
    kernel.start().unwrap();
    kernel
        .admit_input(
            &input_port.port_id,
            0,
            &ValuePayload {
                value_kind: input_port.value_kind,
                encoded: input(&contract),
            },
        )
        .unwrap();
    let mut request = None;
    for _ in 0..16 {
        kernel.step().unwrap();
        if let Some(next) = kernel.next_host_request() {
            request = Some(next);
            break;
        }
    }
    let request = request.unwrap();
    let obligation = kernel.host_request_obligation(&request).unwrap().clone();
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
    let submission = owner
        .begin(
            call.node,
            call.call,
            call.request,
            kernel.host_request_input(&admitted).unwrap(),
        )
        .unwrap();
    owner.revoke(call.node, call.call).unwrap();
    kernel.cancel().unwrap();
    // SAFETY: inert completion acknowledges quiescence after software revocation.
    assert!(
        unsafe {
            owner.finish_quiesced(
                &submission,
                NativeControlObservation::Completed {
                    actual: 0,
                    input: &[],
                },
            )
        }
        .is_err()
    );
    assert!(kernel.complete_host_call_bytes(&admitted, &[]).is_err());
    assert!(kernel.next_host_request().is_none());
}

#[test]
fn checked_control_plot_refuses_input_when_remote_sign_budget_is_exhausted() {
    let contract = ControlContract::prepare().unwrap();
    let (mut kernel, input_port, output_port) = prepared();
    let input = ValuePayload {
        value_kind: input_port.value_kind.clone(),
        encoded: input(&contract),
    };
    let raw = crate::usb_base::control_request::ControlTransferRequest::new(
        [128, 6, 0, 1, 0, 0, 8, 0],
        &[],
        256,
    )
    .unwrap();
    let mut encoder =
        crate::usb_base::control_result::PreparedControlResultEncoder::new(&contract).unwrap();
    let result = encoder.completed(&raw, 0, &[]).unwrap().to_vec();
    let mut output = ValuePayload {
        value_kind: output_port.value_kind.clone(),
        encoded: Vec::with_capacity(CONTROL_MAXIMUM_BYTES as usize),
    };
    kernel.start().unwrap();
    for sequence in 0..4 {
        kernel
            .admit_input(&input_port.port_id, sequence, &input)
            .unwrap();
        let mut delivered = false;
        for _ in 0..16 {
            kernel.step().unwrap();
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
                kernel.complete_host_call_bytes(&admitted, &result).unwrap();
            }
            if let Some(actual) = kernel
                .output_into(&output_port.port_id, &mut output)
                .unwrap()
            {
                assert_eq!(actual, sequence);
                assert_eq!(output.encoded, result);
                kernel
                    .complete_output(&output_port.port_id, actual)
                    .unwrap();
                delivered = true;
                break;
            }
        }
        assert!(delivered, "output {sequence}");
    }
    assert!(matches!(
        kernel.admit_input(&input_port.port_id, 4, &input),
        Err(KernelCompositeError::Execution {
            reason: conduit_composite::ChildExecutionError::Scheduler(
                conduit_kernel::scheduler::SchedulerError::Sign(
                    conduit_kernel::SignError::RemoteItemCapacityExceeded
                )
            ),
            ..
        })
    ));
    assert!(kernel.next_host_request().is_none());
}

#[path = "execution/sustained.rs"]
mod sustained;
