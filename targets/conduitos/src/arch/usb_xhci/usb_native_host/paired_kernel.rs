//! Exact sealed-plan kernel admission for the paired native lease fixture.
use conduit_composite::*;
use conduit_core::*;
use conduit_plot::CompositeFrontTerminal;

pub(super) fn admit_fore(fragment: &mut PlanFragment) {
    for (index, gear) in fragment.placements.iter().enumerate() {
        for (direction, descriptor, name) in [
            (
                PortDirection::Input,
                &gear.inputs[0],
                if index == 0 { "read" } else { "write" },
            ),
            (
                PortDirection::Output,
                &gear.outputs[0],
                if index == 0 { "received" } else { "written" },
            ),
        ] {
            let location = match direction {
                PortDirection::Input => FrontValueLocation::Input(descriptor.port_id.clone()),
                PortDirection::Output => FrontValueLocation::Output(descriptor.port_id.clone()),
            };
            let value_contract = gear
                .semantic_contract
                .value_contracts()
                .iter()
                .find(|value| value.location == location)
                .map(|value| value.contract.clone());
            fragment.fore_ports.push(PlannedForePort {
                front_port_id: port_id(name),
                direction,
                placement_id: gear.placement_id.clone(),
                gear_port_id: descriptor.port_id.clone(),
                value_kind: descriptor.value_kind.clone(),
                value_contract,
                abnormal_kind: descriptor.abnormal_kind.clone(),
                track: ConnectionTrack::Payload,
                temporal: descriptor.temporal,
                pressure_policy: DeliveryPressurePolicy::PreserveOrder,
                item_capacity: 1,
                byte_capacity: 4096,
                selected_line: None,
            });
        }
    }
}

pub(super) fn prepare(plan: Plan) -> KernelCompositeHost {
    let fragment = &plan.fragments[0];
    let mut inputs = alloc::vec::Vec::new();
    let mut outputs = alloc::vec::Vec::new();
    let mut input_fronts = alloc::vec::Vec::new();
    let mut output_fronts = alloc::vec::Vec::new();
    for (index, gear) in fragment.placements.iter().enumerate() {
        let mut input = gear.inputs[0].clone();
        input.port_id = port_id(if index == 0 { "read" } else { "write" });
        let mut output = gear.outputs[0].clone();
        output.port_id = port_id(if index == 0 { "received" } else { "written" });
        input_fronts.push(KernelCompositeFrontBinding {
            external_port: input.clone(),
            internal_child: fragment.host_id.clone(),
            internal_placement_id: gear.placement_id.clone(),
            internal_port_id: gear.inputs[0].port_id.clone(),
            terminal: CompositeFrontTerminal::Independent,
        });
        output_fronts.push(KernelCompositeFrontBinding {
            external_port: output.clone(),
            internal_child: fragment.host_id.clone(),
            internal_placement_id: gear.placement_id.clone(),
            internal_port_id: gear.outputs[0].port_id.clone(),
            terminal: CompositeFrontTerminal::Independent,
        });
        inputs.push(input);
        outputs.push(output);
    }
    let definition = KernelCompositeDefinition {
        host_id: fragment.host_id.clone(),
        boot_id: fragment.boot_id.clone(),
        offer_generation: fragment.offer_generation,
        profile: "fixture/paired-native-kernel@1".into(),
        external_capability: capability_offer_from_parts! {
            semantic_contract:Default::default(), startup_parameters:alloc::vec![], shorthand:None,
            capability_id:"fixture/paired-native-kernel".into(), kind_id:"fixture/paired-native-kernel".into(), kind_contract_revision:"fixture/paired-native-kernel@1".into(),
            implementation:ImplementationOffer { execution_profile_id:"fixture/paired-native-kernel@1".into(), implementation_id:"fixture/paired-native-kernel@1".into(), artifact_id:"fixture/paired-native-kernel@1".into() },
            inputs, outputs, host_calls:alloc::vec![], resource_requirements:alloc::vec![], authority_requirements:alloc::vec![],
            limits:CapabilityLimits { max_active_instances:1, max_queue_items:1, max_queue_bytes:4096 },
        },
        internal_plan: plan,
        boundary: KernelCompositeBoundary {
            input_fronts,
            output_fronts,
        },
        failure_translation: FailureReason::CompositeCapabilityFailed,
    };
    let mut registry = KernelOperationRegistry::new();
    registry
        .install(
            crate::usb_base::endpoint_read_factory::EndpointReadOperationFactory::prepare_contract(
            )
            .unwrap(),
        )
        .unwrap();
    registry.install(crate::usb_base::endpoint_write_factory::EndpointWriteOperationFactory::prepare_contract().unwrap()).unwrap();
    KernelCompositeHost::prepare(definition, &registry).unwrap()
}

pub(super) fn admit_pair(
    kernel: &mut KernelCompositeHost,
    values: [ValuePayload; 2],
) -> [AdmittedKernelCompositeHostRequest; 2] {
    kernel.start().unwrap();
    for (index, port) in ["read", "write"].into_iter().enumerate() {
        kernel
            .admit_input(&port_id(port), 0, &values[index])
            .unwrap();
        kernel.close_input(&port_id(port)).unwrap();
    }
    let mut requests = [None, None];
    for _ in 0..64 {
        kernel.step().unwrap();
        while let Some(request) = kernel.next_host_request() {
            let exact = kernel.host_request_obligation(&request).unwrap();
            let admitted = kernel
                .admit_host_request(&request, &exact.host, &exact.resources, &exact.authorities)
                .unwrap();
            let node = kernel
                .admitted_host_request_view(&admitted)
                .unwrap()
                .request
                .node;
            let position = usize::from(node.0);
            assert!(position < 2 && requests[position].is_none());
            requests[position] = Some(admitted);
        }
        if requests.iter().all(Option::is_some) {
            return requests.map(Option::unwrap);
        }
    }
    panic!("both exact admitted requests must remain pending together");
}
