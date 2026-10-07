#![cfg(feature = "kernel-step")]
use conduit_ai::{
    fixed_numeric_binding::*, fixed_numeric_catalog::*, fixed_numeric_codec::*,
    fixed_numeric_linear_back::*, fixed_numeric_preparation::TENSOR_READ_AUTHORITY,
};
use conduit_core::*;
use conduit_data::*;
use conduit_kernel::{
    scheduler::{FixedScheduler, StepBack, StepInputBytes, StepIo, StepOutcome},
    FixedRoutes, HostedSignLog, HostedValueStore, KernelEvent, PortId as KPort, ValueRef,
    ValueStorage,
};
use conduit_plot::rust_binding::BoundedSequence;
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
fn resource_tensor(dimensions: &[u64], bytes: &[u8]) -> TensorValue {
    let digest = tensor_content_digest(bytes);
    TensorValue {
        element: TensorElement::F32,
        dimensions: BoundedSequence::try_from_iter(dimensions.iter().copied()).unwrap(),
        axes: BoundedSequence::try_from_iter(dimensions.iter().map(|_| TensorAxis {
            role: TensorAxisRole::Feature,
            identity: None,
            unit: None,
        }))
        .unwrap(),
        content_digest: digest,
        backing: TensorBacking::Resource(BoundedResourceRef {
            identity: ResourceSemanticIdentity::from_digest(digest),
            content_profile: kind_id("tensor/elements-ieee754-f32-le@1"),
            access_class: ResourceClassId::from("test/read@1"),
            extent: ResourceExtent {
                bytes: bytes.len() as u64,
                items: Some(dimensions.iter().product()),
            },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest([1; 32]),
                expires_at: None,
            },
        }),
    }
}
fn bytes(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

fn access(tensor: &TensorValue) -> ResourceReferenceBinding {
    let TensorBacking::Resource(reference) = &tensor.backing else {
        panic!("resource fixture")
    };
    ResourceReferenceBinding {
        identity: reference.identity,
        version: reference.lifetime.version,
        content_profile: reference.content_profile.clone(),
        access_class: reference.access_class.clone(),
        handle: ResourceHandleId::from("numeric-test-immutable"),
        authority_contract: AuthorityContractId::from(TENSOR_READ_AUTHORITY),
        authority_grant: AuthorityGrantId::from("numeric-test-read"),
        maximum_bytes: reference.extent.bytes,
        maximum_items: reference.extent.items,
        availability: ResourceReferenceAvailability::Available,
    }
}

enum Driver<'a> {
    Source(ValueRef, bool),
    Linear(Box<FixedLinearBack<'a, 80, 1>>),
    Sink(Rc<Cell<bool>>, Vec<u8>),
}
impl StepBack<PORTS> for Driver<'_> {
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
            Self::Linear(back) => back.step(io, inputs),
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
        if let Self::Linear(back) = self {
            <FixedLinearBack<80, 1> as StepBack<PORTS>>::step_committed(back);
        }
    }
    fn prepared_output(&self, port: KPort) -> Option<&[u8]> {
        if let Self::Linear(back) = self {
            <FixedLinearBack<80, 1> as StepBack<PORTS>>::prepared_output(back, port)
        } else {
            None
        }
    }
    fn cancel(&mut self) {
        if let Self::Linear(back) = self {
            <FixedLinearBack<80, 1> as StepBack<PORTS>>::cancel(back);
        }
    }
}
#[test]
fn ordinary_linear_plan_reads_exact_resource_without_bias() {
    let packed = bytes(&[0.5; 80]);
    let tensor = resource_tensor(&[80, 1], &packed);
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    install_fixed_numeric_catalogs(&mut startup, &mut profile).unwrap();
    let mut offers = Vec::new();
    for (name, type_name, source) in [
        ("numeric-test/input", "NumericF32Vector80", true),
        ("numeric-test/weights", "NumericF32MatrixRef80x1", true),
        ("numeric-test/sink", "NumericF32Vector1", false),
    ] {
        let ty = fixed_numeric_type(type_name).unwrap();
        let offer = fixture(name, &ty, source);
        startup
            .insert(conduit_plot::KindSignature {
                kind: name.into(),
                startup_parameters: vec![],
            })
            .unwrap();
        profile
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
        offers.push(offer);
    }
    offers.push(fixed_linear_offer::<80, 1>().unwrap());
    let source="plot linear-proof {\n input: numeric-test/input\n weights: numeric-test/weights\n operation: numeric/linear80x1\n sink: numeric-test/sink\n input.value >> operation.value\n weights.value >> operation.weights\n operation.result >> sink.value\n}\n";
    let plot = conduit_plot::parse_with_startup(source, &startup, &profile).unwrap();
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
    assert!(verify_plan(&plan));
    let fragment = &plan.fragments[0];
    let (lowered, _) = conduit_plan_lowering::lowering::lower_plan_fragment_from_plan(
        &plan,
        &fragment.fragment_id,
    )
    .unwrap();
    let mut values = HostedValueStore::new(8, 16384, 8 * 16384).unwrap();
    let mut input =
        FixedF32VectorCodec::<80>::prepare(&fixed_numeric_type("NumericF32Vector80").unwrap())
            .unwrap();
    let input = values.store(input.encode(&[2.; 80]).unwrap()).unwrap();
    let weights = values
        .store(
            FixedTensorPortBinding::prepare(
                &fixed_numeric_type("NumericF32MatrixRef80x1").unwrap(),
                &tensor,
            )
            .unwrap()
            .encoded(),
        )
        .unwrap();
    let expected =
        FixedF32VectorCodec::<1>::prepare(&fixed_numeric_type("NumericF32Vector1").unwrap())
            .unwrap()
            .encode(&[80.])
            .unwrap()
            .to_vec();
    let seen = Rc::new(Cell::new(false));
    let drivers: Vec<_> = fragment
        .placements
        .iter()
        .enumerate()
        .map(|(i, gear)| match gear.kind_id.as_str() {
            "numeric-test/input" => Driver::Source(input, false),
            "numeric-test/weights" => Driver::Source(weights, false),
            "numeric-test/sink" => Driver::Sink(seen.clone(), expected.clone()),
            _ => {
                let mut wrong = gear.clone();
                wrong.implementation_id = ImplementationId::from("wrong");
                assert!(FixedLinearBack::<80, 1>::prepare_planned::<PORTS>(
                    &wrong,
                    4,
                    &tensor,
                    &packed,
                    &access(&tensor)
                )
                .is_err());
                let mut unavailable = access(&tensor);
                unavailable.availability = ResourceReferenceAvailability::Lost;
                assert!(FixedLinearBack::<80, 1>::prepare_planned::<PORTS>(
                    gear,
                    4,
                    &tensor,
                    &packed,
                    &unavailable
                )
                .is_err());
                Driver::Linear(Box::new(
                    FixedLinearBack::prepare_planned::<PORTS>(
                        gear,
                        lowered.node_specs[i].maximum_step_fuel,
                        &tensor,
                        &packed,
                        &access(&tensor),
                    )
                    .unwrap(),
                ))
            }
        })
        .collect();
    let mut routes = FixedRoutes::<4, 3>::new(1);
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
    let mut scheduler = FixedScheduler::<_, _, _, 4, 3, PORTS, 3, 4, 3>::new(
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
    for _ in 0..64 {
        scheduler.step().unwrap();
        if seen.get() {
            break;
        }
    }
    assert!(seen.get());
}
