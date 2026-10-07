#![cfg(feature = "kernel-step")]
use conduit_ai::{
    fixed_numeric_catalog::*, fixed_numeric_codec::FixedF32VectorCodec,
    fixed_numeric_signal_back::*,
};
use conduit_core::*;
use conduit_kernel::{
    scheduler::{FixedScheduler, StepBack, StepInputBytes, StepIo, StepOutcome},
    FixedRoutes, HostedSignLog, HostedValueStore, KernelEvent, PortId as KPort, ValueRef,
    ValueStorage,
};
use std::{cell::Cell, rc::Rc};
const PORTS: usize = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
fn fixture(name: &str, ty: &StructuredInfoType, source: bool) -> CapabilityOffer {
    let port = PortDescriptor {
        port_id: port_id("value"),
        value_kind: ty.profile().unwrap().value_kind().clone(),
        direction: if source {
            PortDirection::Output
        } else {
            PortDirection::Input
        },
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    };
    let kind = Kind {
        kind_id: kind_id(name),
        kind_contract_revision: KindIdentity::from(name),
        startup_parameters: vec![],
        shorthand: None,
        configuration: vec![],
        inputs: if source { vec![] } else { vec![port.clone()] },
        outputs: if source { vec![port.clone()] } else { vec![] },
        semantic_laws: vec![KindSemanticLaw::ValueContracts(vec![FrontValueContract {
            location: if source {
                FrontValueLocation::Output(port.port_id)
            } else {
                FrontValueLocation::Input(port.port_id)
            },
            contract: CheckedValueContract::new(port.value_kind, 16384, vec![]).unwrap(),
        }])],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: 16384,
        },
    };
    BackOfferBuilder::new(
        kind,
        Back {
            capability_id: CapabilityId::from(name),
            execution_profile_id: ExecutionProfileId::from("numeric-test@1"),
            implementation_id: ImplementationId::from(name),
            artifact_id: ArtifactId::from(name),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}
enum Driver<const W: usize> {
    Source(ValueRef, bool),
    Operation(Box<FixedElementwiseBack<W>>),
    Sink(Rc<Cell<bool>>, Vec<u8>),
}
impl<const W: usize> StepBack<PORTS> for Driver<W> {
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        match self {
            Self::Source(reference, sent) => {
                if *sent {
                    return StepOutcome::Complete;
                }
                if !io.output_ready(KPort(0)) {
                    return StepOutcome::Await;
                }
                io.send(KPort(0), *reference).unwrap();
                *sent = true;
                StepOutcome::Complete
            }
            Self::Operation(back) => back.step(io, inputs),
            Self::Sink(seen, expected) => {
                if io.input(KPort(0)).is_none() {
                    return StepOutcome::Await;
                }
                assert_eq!(inputs.input(KPort(0)).unwrap(), expected);
                io.consume(KPort(0)).unwrap();
                seen.set(true);
                StepOutcome::Complete
            }
        }
    }
    fn step_committed(&mut self) {
        if let Self::Operation(back) = self {
            <FixedElementwiseBack<W> as StepBack<PORTS>>::step_committed(back);
        }
    }
    fn prepared_output(&self, port: KPort) -> Option<&[u8]> {
        if let Self::Operation(back) = self {
            <FixedElementwiseBack<W> as StepBack<PORTS>>::prepared_output(back, port)
        } else {
            None
        }
    }
    fn cancel(&mut self) {
        if let Self::Operation(back) = self {
            <FixedElementwiseBack<W> as StepBack<PORTS>>::cancel(back);
        }
    }
}
fn exercise<const W: usize, const N: usize, const C: usize>(
    operation: FixedElementwiseOperation,
    input: &[Vec<f32>],
    expected: [f32; W],
    overflow: bool,
    pressure: bool,
) {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    install_fixed_numeric_catalogs(&mut startup, &mut profiles).unwrap();
    let operation_offer = fixed_elementwise_offer::<W>(operation).unwrap();
    assert_eq!(input.len(), operation_offer.inputs.len());
    let owned = fixed_numeric_types().unwrap();
    let mut offers = Vec::new();
    let mut source = String::from("plot numeric-proof {\n");
    let mut values = HostedValueStore::new(
        if pressure { input.len() as u16 } else { 16 },
        16384,
        if pressure {
            input.len() as u32 * 16384
        } else {
            16 * 16384
        },
    )
    .unwrap();
    let mut references = Vec::new();
    for (i, (vector, port)) in input.iter().zip(&operation_offer.inputs).enumerate() {
        let ty = &owned
            .iter()
            .find(|ty| ty.value_type.profile().unwrap().value_kind() == &port.value_kind)
            .unwrap()
            .value_type;
        let name = format!("numeric-test/source{i}");
        offers.push(fixture(&name, ty, true));
        source.push_str(&format!(" source{i}: {name}\n"));
        let bytes = if vector.len() == 1 {
            FixedF32VectorCodec::<1>::prepare(ty)
                .unwrap()
                .encode(&[vector[0]])
                .unwrap()
                .to_vec()
        } else {
            FixedF32VectorCodec::<W>::prepare(ty)
                .unwrap()
                .encode(&vector.clone().try_into().unwrap())
                .unwrap()
                .to_vec()
        };
        references.push(values.store(&bytes).unwrap());
    }
    let output_type = fixed_numeric_type(&format!("NumericF32Vector{W}")).unwrap();
    offers.push(fixture("numeric-test/sink", &output_type, false));
    source.push_str(&format!(
        " operation: {}\n sink: numeric-test/sink\n",
        operation_offer.kind_id.as_str()
    ));
    for (i, port) in operation_offer.inputs.iter().enumerate() {
        source.push_str(&format!(
            " source{i}.value >> operation.{}\n",
            port.port_id.as_str()
        ));
    }
    source.push_str(" operation.result >> sink.value\n}\n");
    offers.push(operation_offer);
    for offer in &offers {
        if offer.kind_id.as_str().starts_with("numeric-test/") {
            startup
                .insert(conduit_plot::KindSignature {
                    kind: offer.kind_id.as_str().into(),
                    startup_parameters: vec![],
                })
                .unwrap();
            let source_kind = fixture(
                offer.kind_id.as_str(),
                if offer.inputs.is_empty() {
                    &owned
                        .iter()
                        .find(|ty| {
                            ty.value_type.profile().unwrap().value_kind()
                                == &offer.outputs[0].value_kind
                        })
                        .unwrap()
                        .value_type
                } else {
                    &output_type
                },
                offer.inputs.is_empty(),
            );
            profiles
                .insert_kind(Kind {
                    kind_id: source_kind.kind_id,
                    kind_contract_revision: source_kind.kind_contract_revision,
                    startup_parameters: vec![],
                    shorthand: None,
                    configuration: vec![],
                    inputs: source_kind.inputs,
                    outputs: source_kind.outputs,
                    semantic_laws: vec![KindSemanticLaw::ValueContracts(
                        source_kind.semantic_contract.value_contracts().to_vec(),
                    )],
                    limits: source_kind.limits,
                })
                .unwrap();
        }
    }
    let plot = conduit_plot::parse_with_startup(&source, &startup, &profiles).unwrap();
    let hosts = [HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("numeric-test"),
        boot_id: BootId::from("numeric-test-boot"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("numeric-test@1"),
        bases: vec![],
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: offers,
    }];
    let placements = conduit_planner::default_placements(&plot, &hosts).unwrap();
    let plan = conduit_planner::plan_with_connection_limits(
        &plot,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        1,
        16384,
    )
    .unwrap();
    let fragment = &plan.fragments[0];
    let (lowered, _) = conduit_plan_lowering::lowering::lower_plan_fragment_from_plan(
        &plan,
        &fragment.fragment_id,
    )
    .unwrap();
    let seen = Rc::new(Cell::new(false));
    let expected = FixedF32VectorCodec::<W>::prepare(&output_type)
        .unwrap()
        .encode(&expected)
        .unwrap()
        .to_vec();
    let drivers: Vec<Driver<W>> = fragment
        .placements
        .iter()
        .enumerate()
        .map(|(i, gear)| {
            if gear.kind_id.as_str() == "numeric-test/sink" {
                Driver::Sink(seen.clone(), expected.clone())
            } else if let Some(index) = gear.kind_id.as_str().strip_prefix("numeric-test/source") {
                Driver::Source(references[index.parse::<usize>().unwrap()], false)
            } else {
                Driver::Operation(Box::new(
                    FixedElementwiseBack::prepare_planned::<PORTS>(
                        gear,
                        lowered.node_specs[i].maximum_step_fuel,
                        operation,
                    )
                    .unwrap(),
                ))
            }
        })
        .collect();
    let mut routes = FixedRoutes::<N, C>::new(1);
    for route in &lowered.routes {
        routes
            .install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )
            .unwrap();
    }
    routes.seal().unwrap();
    let mut scheduler = FixedScheduler::<_, _, _, N, C, PORTS, C, N, C>::new(
        lowered.node_specs.try_into().unwrap(),
        lowered
            .cords
            .into_iter()
            .map(|c| c.spec)
            .collect::<Vec<_>>()
            .try_into()
            .unwrap(),
        routes,
        drivers
            .try_into()
            .unwrap_or_else(|_| panic!("driver count")),
        values,
        HostedSignLog::new(1024, 1024 * core::mem::size_of::<KernelEvent>() as u32).unwrap(),
    )
    .unwrap();
    let mut refused = false;
    for _ in 0..64 {
        if let Err(error) = scheduler.step() {
            assert!(overflow || pressure, "{error:?}");
            refused = true;
            break;
        }
        if seen.get() {
            break;
        }
    }
    assert_eq!(seen.get(), !overflow && !pressure);
    assert_eq!(refused, overflow || pressure);
    if pressure {
        for driver in scheduler.drivers() {
            if let Driver::Operation(back) = driver {
                assert!(!back.has_committed_result());
            }
        }
        scheduler.cancel().unwrap();
    }
}
#[test]
fn ordinary_elementwise_operations_publish_independent_expected_values() {
    exercise::<40, 4, 3>(
        FixedElementwiseOperation::Add,
        &[vec![2.; 40], vec![-0.5; 40]],
        [1.5; 40],
        false,
        false,
    );
    exercise::<40, 4, 3>(
        FixedElementwiseOperation::Multiply,
        &[vec![2.; 40], vec![-0.5; 40]],
        [-1.; 40],
        false,
        false,
    );
    exercise::<40, 3, 2>(
        FixedElementwiseOperation::Sigmoid,
        &[vec![0.; 40]],
        [0.5; 40],
        false,
        false,
    );
    exercise::<40, 3, 2>(
        FixedElementwiseOperation::Complement,
        &[vec![0.25; 40]],
        [0.75; 40],
        false,
        false,
    );
    exercise::<1, 3, 2>(
        FixedElementwiseOperation::Exp,
        &[vec![0.]],
        [1.],
        false,
        false,
    );
    exercise::<40, 4, 3>(
        FixedElementwiseOperation::Scale,
        &[vec![2.; 40], vec![0.25]],
        [0.5; 40],
        false,
        false,
    );
    exercise::<40, 5, 4>(
        FixedElementwiseOperation::Clamp,
        &[vec![2.; 40], vec![-1.], vec![1.]],
        [1.; 40],
        false,
        false,
    );
    exercise::<1, 4, 3>(
        FixedElementwiseOperation::ReciprocalOffset,
        &[vec![1.5], vec![0.5]],
        [0.5],
        false,
        false,
    );
}
#[test]
fn elementwise_overflow_and_pressure_publish_no_result() {
    exercise::<40, 4, 3>(
        FixedElementwiseOperation::Multiply,
        &[vec![f32::MAX; 40], vec![2.; 40]],
        [0.; 40],
        true,
        false,
    );
    exercise::<1, 4, 3>(
        FixedElementwiseOperation::ReciprocalOffset,
        &[vec![1.], vec![-1.]],
        [0.],
        true,
        false,
    );
    exercise::<40, 4, 3>(
        FixedElementwiseOperation::Add,
        &[vec![1.; 40], vec![2.; 40]],
        [3.; 40],
        false,
        true,
    );
}
