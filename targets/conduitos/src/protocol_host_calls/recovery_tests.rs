//! Dispatcher recovery barrier with genuine selected native owners and original kernel tokens.
use super::*;
use crate::usb_base::{
    endpoint_read_owner::tests::fixture as read, endpoint_write_owner::tests::fixture as write,
};
struct NoI2c;
impl crate::i2c_base::I2cProvider for NoI2c {
    fn transact(
        &mut self,
        _: &crate::i2c_base::I2cTransaction<'_>,
        _: &mut [u8],
    ) -> Result<usize, crate::i2c_base::I2cDisposition> {
        panic!("no I2C effect")
    }
    fn revoke(&mut self) {}
}
use conduit_composite::*;
use conduit_plot::CompositeFrontTerminal;

fn admit_fore(fragment: &mut PlanFragment) {
    for (index, gear) in fragment.placements.iter().enumerate() {
        for (direction, descriptor, name) in [
            (
                PortDirection::Input,
                &gear.inputs[0],
                ["read", "write", "control"][index],
            ),
            (
                PortDirection::Output,
                &gear.outputs[0],
                ["received", "written", "controlled"][index],
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

fn prepare(plan: Plan) -> KernelCompositeHost {
    let fragment = &plan.fragments[0];
    let mut inputs = alloc::vec::Vec::new();
    let mut outputs = alloc::vec::Vec::new();
    let mut input_fronts = alloc::vec::Vec::new();
    let mut output_fronts = alloc::vec::Vec::new();
    for (index, gear) in fragment.placements.iter().enumerate() {
        let mut input = gear.inputs[0].clone();
        input.port_id = port_id(["read", "write", "control"][index]);
        let mut output = gear.outputs[0].clone();
        output.port_id = port_id(["received", "written", "controlled"][index]);
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
    registry
        .install(
            crate::usb_base::control_factory::ControlOperationFactory::prepare_contract().unwrap(),
        )
        .unwrap();
    KernelCompositeHost::prepare(definition, &registry).unwrap()
}

fn plan() -> Plan {
    let (mut fragment, _, _, read_placement) = read::selected();
    let (write_fragment, _, _, write_placement) = write::selected();
    fragment.placements.extend(write_fragment.placements);
    fragment.startup_order.push(write_placement);
    for gear in &mut fragment.placements {
        let (implementation, profile, base, attachment, authority) =
            if gear.placement_id == read_placement {
                use crate::usb_base::endpoint_read_factory::*;
                (
                    ENDPOINT_READ_IMPLEMENTATION,
                    ENDPOINT_READ_PROFILE,
                    ENDPOINT_READ_BASE,
                    ENDPOINT_READ_ATTACHMENT,
                    ENDPOINT_READ_AUTHORITY,
                )
            } else {
                use crate::usb_base::endpoint_write_factory::*;
                (
                    ENDPOINT_WRITE_IMPLEMENTATION,
                    ENDPOINT_WRITE_PROFILE,
                    ENDPOINT_WRITE_BASE,
                    ENDPOINT_WRITE_ATTACHMENT,
                    ENDPOINT_WRITE_AUTHORITY,
                )
            };
        gear.implementation_id = implementation.into();
        gear.execution_profile_id = profile.into();
        gear.base.as_mut().unwrap().implementation_id = base.into();
        gear.base.as_mut().unwrap().mechanism_family = attachment.into();
        gear.resources[0].class_id = attachment.into();
        gear.authority[0].contract_id = authority.into();
    }
    let contract = crate::usb_base::control_contract::ControlContract::prepare().unwrap();
    let control = crate::usb_base::control_proof_plan::plan(
        &contract,
        &crate::usb_base::control_proof_plan::ControlProofSubject {
            host_id: "host/one",
            boot_id: "boot/one",
            controller_base_id: "base/control",
            device_instance_id: "device/control",
            root_port: 1,
            slot: 1,
            attachment_epoch: 1,
        },
    )
    .unwrap();
    for extra in [
        write_fragment.expected_sign,
        control.fragments[0].expected_sign.clone(),
    ] {
        fragment.expected_sign.extend(extra.into_iter().filter(|s| {
            matches!(
                s,
                ExpectedSign::PlacementPrepared(_) | ExpectedSign::PlacementTerminal(_)
            )
        }));
    }
    fragment
        .placements
        .extend(control.fragments[0].placements.clone());
    fragment
        .startup_order
        .extend(control.fragments[0].startup_order.clone());
    fragment.sign_storage_budget =
        mandatory_sign_storage_requirement(&fragment.expected_sign).unwrap();
    admit_fore(&mut fragment);
    let identity = PlotIdentity {
        source_document_id: fragment.source_document_id.clone(),
        checked_plot_id: fragment.checked_plot_id.clone(),
        expanded_plot_id: fragment.expanded_plot_id.clone(),
    };
    seal_plan(identity, alloc::vec![fragment])
}
#[test]
fn duplex_control_dispatch_waits_for_original_bulk_kernel_obligation_retirement() {
    let plan = plan();
    crate::arch::NativeUsbDuplexHost::with_recovery_fixture(&plan, |native| {
        let fragment = &plan.fragments[0];
        let lowered = lower_plan_fragment(fragment).unwrap();
        let bindings = lowered
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| Binding {
                node: n.node,
                resources: fragment.placements[i].resources.clone(),
                authorities: fragment.placements[i].authority.clone(),
                owner: match i {
                    0 => Owner::UsbDuplexRead,
                    1 => Owner::UsbDuplexWrite,
                    _ => Owner::UsbDuplexControl,
                },
            })
            .collect();
        let mut calls = PreparedProtocolCalls::<NoI2c> {
            plan_id: plan.plan_id.clone(),
            storage: None,
            identity: PreparationHostIdentity {
                host_id: fragment.host_id.clone(),
                boot_id: fragment.boot_id.clone(),
                offer_generation: fragment.offer_generation,
            },
            bindings,
            usb_protocol: None,
            usb_duplex: Some(native),
            usb_isochronous: None,
            pending_usb_kind: PendingUsbKind::DuplexRead,
            pending_usb: None,
            pending_usb_failed: false,
            pending_duplex: [None, None],
        };
        let contract = crate::usb_base::control_contract::ControlContract::prepare().unwrap();
        let StructuredInfoTypeShape::Record { fields, .. } = contract.request_type().shape() else {
            panic!("record")
        };
        let ty = |name| {
            fields
                .iter()
                .find(|field| field.name() == name)
                .unwrap()
                .value_type()
                .clone()
        };
        let setup =
            StructuredInfoValue::leaf(ty("setup"), alloc::vec![128, 6, 0, 1, 0, 0, 64, 0]).unwrap();
        let output = StructuredInfoValue::sequence(ty("output"), alloc::vec![]).unwrap();
        let control = StructuredInfoValue::record(
            contract.request_type().clone(),
            alloc::vec![
                StructuredFieldValue::new("setup", setup).unwrap(),
                StructuredFieldValue::new("output", output).unwrap()
            ],
        )
        .unwrap()
        .canonical_bytes()
        .unwrap();
        let values = [
            read::input_length(8),
            write::input_wire(&[1, 2, 3, 4]),
            control,
        ];
        let kinds = fragment
            .placements
            .iter()
            .map(|g| g.inputs[0].value_kind.clone())
            .collect::<Vec<_>>();
        let mut kernel = prepare(plan.clone());
        kernel.start().unwrap();
        for (i, name) in ["read", "write", "control"].into_iter().enumerate() {
            kernel
                .admit_input(
                    &port_id(name),
                    0,
                    &ValuePayload {
                        value_kind: kinds[i].clone(),
                        encoded: values[i].clone(),
                    },
                )
                .unwrap();
            kernel.close_input(&port_id(name)).unwrap();
        }
        let mut tokens = [None, None];
        let mut control_request = None;
        for _ in 0..64 {
            kernel.step().unwrap();
            while let Some(request) = kernel.next_host_request() {
                let node = kernel.host_request_view(&request).unwrap().request.node;
                if node == NodeId(2) {
                    control_request = Some(request);
                } else {
                    let exact = kernel.host_request_obligation(&request).unwrap();
                    tokens[usize::from(node.0)] = Some(
                        kernel
                            .admit_host_request(
                                &request,
                                &exact.host,
                                &exact.resources,
                                &exact.authorities,
                            )
                            .unwrap(),
                    );
                }
            }
            if control_request.is_some() && tokens.iter().all(Option::is_some) {
                break;
            }
        }
        let request = control_request.unwrap();
        assert!(calls.pending_duplex.iter().all(Option::is_none));
        assert!(!calls.can_dispatch(&kernel, &request).unwrap());
        let mut read_encoder =
            crate::usb_base::endpoint_read_result::PreparedEndpointReadResultEncoder::new(
                &crate::usb_base::endpoint_read_contract::EndpointReadContract::prepare().unwrap(),
            )
            .unwrap();
        let mut write_encoder =
            crate::usb_base::endpoint_write_result::PreparedEndpointWriteResultEncoder::new(
                &crate::usb_base::endpoint_write_contract::EndpointWriteContract::prepare()
                    .unwrap(),
            )
            .unwrap();
        kernel
            .complete_host_call_bytes(
                tokens[0].as_ref().unwrap(),
                read_encoder.completed(0, 8, 8, &[0; 8]).unwrap(),
            )
            .unwrap();
        // The original write obligation remains even after read completion.
        assert!(!calls.can_dispatch(&kernel, &request).unwrap());
        kernel
            .complete_host_call_bytes(
                tokens[1].as_ref().unwrap(),
                write_encoder.completed(4, 4).unwrap(),
            )
            .unwrap();
        // Completed-but-unconsumed original scheduler obligations still block control.
        assert!(!calls.can_dispatch(&kernel, &request).unwrap());
        for _ in 0..64 {
            kernel.step().unwrap();
            if calls.can_dispatch(&kernel, &request).unwrap() {
                break;
            }
        }
        assert!(calls.can_dispatch(&kernel, &request).unwrap());
        calls.cancel(&mut kernel).unwrap();
    });
}
