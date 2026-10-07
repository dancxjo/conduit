#![cfg(feature = "kernel-step")]
use conduit_ai::{
    fixed_numeric_catalog::*, fixed_numeric_codec::FixedF32VectorCodec,
    fixed_numeric_index_back::*, fixed_numeric_index_codec::*,
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
        value_kind: match ty.shape() {
            StructuredInfoTypeShape::Leaf(kind) => kind.clone(),
            _ => ty.profile().unwrap().value_kind().clone(),
        },
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
            contract: CheckedValueContract::new(
                port.value_kind,
                conduit_plot::maximum_prepared_transport_value_bytes(ty).unwrap(),
                vec![],
            )
            .unwrap(),
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
enum Driver<const I: usize, const O: usize> {
    Source(ValueRef, bool),
    Operation(Box<FixedIndexBack<I, O>>),
    Sink(Rc<Cell<bool>>, Vec<u8>),
}
impl<const I: usize, const O: usize> StepBack<PORTS> for Driver<I, O> {
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
            <FixedIndexBack<I, O> as StepBack<PORTS>>::step_committed(back);
        }
    }
    fn prepared_output(&self, port: KPort) -> Option<&[u8]> {
        if let Self::Operation(back) = self {
            <FixedIndexBack<I, O> as StepBack<PORTS>>::prepared_output(back, port)
        } else {
            None
        }
    }
    fn cancel(&mut self) {
        if let Self::Operation(back) = self {
            <FixedIndexBack<I, O> as StepBack<PORTS>>::cancel(back);
        }
    }
}
fn exercise<const I: usize, const O: usize, const N: usize, const C: usize>(
    operation: FixedIndexOperation,
    input: [f32; I],
    indices: &[u16],
    expected: [f32; O],
    overflow: bool,
    pressure: bool,
) {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    install_fixed_numeric_catalogs(&mut startup, &mut profiles).unwrap();
    let operation_offer = fixed_index_offer::<I, O>(operation).unwrap();
    assert_eq!(operation_offer.inputs.len(), 2);
    let owned = fixed_numeric_types().unwrap();
    let mut offers = Vec::new();
    let mut source = String::from("plot numeric-proof {\n");
    let mut values = HostedValueStore::new(
        if pressure { 2 } else { 16 },
        16384,
        if pressure { 2 * 16384 } else { 16 * 16384 },
    )
    .unwrap();
    let mut references = Vec::new();
    for (i, port) in operation_offer.inputs.iter().enumerate() {
        let ty = owned
            .iter()
            .find(|ty| ty.value_type.profile().unwrap().value_kind() == &port.value_kind)
            .map(|ty| ty.value_type.clone())
            .unwrap_or_else(|| StructuredInfoType::leaf(port.value_kind.clone()).unwrap());
        let name = format!("numeric-test/source{i}");
        offers.push(fixture(&name, &ty, true));
        source.push_str(&format!(" source{i}: {name}\n"));
        let bytes = if i == 0 {
            FixedF32VectorCodec::<I>::prepare(&ty)
                .unwrap()
                .encode(&input)
                .unwrap()
                .to_vec()
        } else if indices.len() == 1 {
            FixedU16IndexCodec::<1>::prepare(&ty)
                .unwrap()
                .encode(&[indices[0]])
                .to_vec()
        } else {
            FixedU16IndexCodec::<O>::prepare(&ty)
                .unwrap()
                .encode(&indices.try_into().unwrap())
                .to_vec()
        };
        references.push(values.store(&bytes).unwrap());
    }
    let output_type = fixed_numeric_type(&format!("NumericF32Vector{O}")).unwrap();
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
            profiles
                .insert_kind(Kind {
                    kind_id: offer.kind_id.clone(),
                    kind_contract_revision: offer.kind_contract_revision.clone(),
                    startup_parameters: vec![],
                    shorthand: None,
                    configuration: vec![],
                    inputs: offer.inputs.clone(),
                    outputs: offer.outputs.clone(),
                    semantic_laws: vec![KindSemanticLaw::ValueContracts(
                        offer.semantic_contract.value_contracts().to_vec(),
                    )],
                    limits: offer.limits.clone(),
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
    let expected = FixedF32VectorCodec::<O>::prepare(&output_type)
        .unwrap()
        .encode(&expected)
        .unwrap()
        .to_vec();
    let drivers: Vec<Driver<I, O>> = fragment
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
                    FixedIndexBack::prepare_planned::<PORTS>(
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
fn ordinary_explicit_index_operations_and_bounds() {
    exercise::<4, 1, 4, 3>(
        FixedIndexOperation::Slice,
        [0., 1., 2., 3.],
        &[2],
        [2.],
        false,
        false,
    );
    exercise::<4, 1, 4, 3>(
        FixedIndexOperation::Slice,
        [0., 1., 2., 3.],
        &[4],
        [0.],
        true,
        false,
    );
    let input = core::array::from_fn(|i| i as f32);
    let indices = core::array::from_fn::<_, 44, _>(|i| if i % 2 == 0 { 0 } else { 255 });
    let expected = core::array::from_fn(|i| if i % 2 == 0 { 0. } else { 255. });
    exercise::<256, 44, 4, 3>(
        FixedIndexOperation::Gather,
        input,
        &indices,
        expected,
        false,
        false,
    );
    let mut bad = indices;
    bad[43] = 256;
    exercise::<256, 44, 4, 3>(
        FixedIndexOperation::Gather,
        input,
        &bad,
        [0.; 44],
        true,
        false,
    );
    exercise::<256, 44, 4, 3>(
        FixedIndexOperation::Gather,
        input,
        &indices,
        expected,
        false,
        true,
    );
}
#[test]
fn exact_index_codec_preserves_output_on_malformed_envelope() {
    let ty = fixed_numeric_type("NumericU16Indices44").unwrap();
    let mut codec = FixedU16IndexCodec::<44>::prepare(&ty).unwrap();
    let bytes = codec.encode(&[7; 44]).to_vec();
    let mut out = [99; 44];
    codec.decode(&bytes, &mut out).unwrap();
    assert_eq!(out, [7; 44]);
    let mut corrupt = bytes;
    corrupt[0] ^= 1;
    assert!(codec.decode(&corrupt, &mut out).is_err());
    assert_eq!(out, [7; 44]);
    assert!(FixedU16IndexCodec::<1>::prepare(&ty).is_err());
}
