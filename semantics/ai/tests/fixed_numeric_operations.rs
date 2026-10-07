#![cfg(feature = "kernel-step")]
use conduit_ai::{
    fixed_numeric_back::*, fixed_numeric_binding::*, fixed_numeric_catalog::*,
    fixed_numeric_codec::*, fixed_numeric_operations_back::*, fixed_numeric_preparation::*,
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
fn tensor(shape: &[u64], bytes: &[u8]) -> TensorValue {
    let digest = tensor_content_digest(bytes);
    TensorValue {
        element: TensorElement::F32,
        dimensions: BoundedSequence::try_from_iter(shape.iter().copied()).unwrap(),
        axes: BoundedSequence::try_from_iter(shape.iter().map(|_| TensorAxis {
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
                items: Some(shape.iter().product()),
            },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest([1; 32]),
                expires_at: None,
            },
        }),
    }
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
fn packed(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|x| x.to_le_bytes()).collect()
}
fn ty(name: &str) -> StructuredInfoType {
    fixed_numeric_type(name).unwrap()
}
fn fixture_kind(name: &str, value: &StructuredInfoType, source: bool) -> Kind {
    let port = PortDescriptor {
        port_id: port_id("value"),
        value_kind: if name == "op-test/index" {
            kind_id("value/u16")
        } else {
            value.profile().unwrap().value_kind().clone()
        },
        direction: if source {
            PortDirection::Output
        } else {
            PortDirection::Input
        },
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    };
    Kind {
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
                conduit_plot::maximum_prepared_transport_value_bytes(value).unwrap(),
                vec![],
            )
            .unwrap(),
        }])],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: 16_384,
        },
    }
}
fn offer(kind: Kind) -> CapabilityOffer {
    let name = kind.kind_id.as_str().to_string();
    BackOfferBuilder::new(
        kind,
        Back {
            capability_id: CapabilityId::from(name.clone()),
            execution_profile_id: ExecutionProfileId::from("numeric-test@1"),
            implementation_id: ImplementationId::from(name.clone()),
            artifact_id: ArtifactId::from(name),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}

fn plan() -> Plan {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    install_fixed_numeric_catalogs(&mut startup, &mut profile).unwrap();
    let mut offers = vec![];
    let index = StructuredInfoType::leaf(kind_id("value/u16")).unwrap();
    for (name, ty, source) in [
        ("op-test/index", index, true),
        ("op-test/features", ty("NumericF32Vector20"), true),
        ("op-test/table", ty("NumericEmbedding224x12"), true),
        ("op-test/weights", ty("NumericF32MatrixRef32x64"), true),
        ("op-test/bias", ty("NumericF32BiasRef64"), true),
        ("op-test/sink", ty("NumericF32Vector64"), false),
    ] {
        let k = fixture_kind(name, &ty, source);
        startup
            .insert(conduit_plot::KindSignature {
                kind: name.into(),
                startup_parameters: vec![],
            })
            .unwrap();
        profile.insert_kind(k.clone()).unwrap();
        offers.push(offer(k));
    }
    offers.extend([
        fixed_embedding_offer::<224, 12>().unwrap(),
        fixed_concatenate_offer::<20, 12>().unwrap(),
        fixed_affine_offer::<32, 64>().unwrap(),
        fixed_tanh_offer::<64>().unwrap(),
    ]);
    let source="plot numeric-operation-proof {\n index: op-test/index\n features: op-test/features\n table: op-test/table\n weights: op-test/weights\n bias: op-test/bias\n embedding: numeric/embedding224x12\n concatenate: numeric/concatenate20x12\n affine: numeric/dense32x64\n activation: numeric/tanh64\n sink: op-test/sink\n index.value >> embedding.index\n table.value >> embedding.weights\n features.value >> concatenate.left\n embedding.result >> concatenate.right\n concatenate.result >> affine.value\n weights.value >> affine.weights\n bias.value >> affine.bias\n affine.result >> activation.value\n activation.result >> sink.value\n}\n";
    let plot = conduit_plot::parse_with_startup(source, &startup, &profile).unwrap();
    let hosts = [HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("op-test"),
        boot_id: BootId::from("op-boot"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("op-test@1"),
        bases: vec![],
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: offers,
    }];
    let placements = conduit_planner::default_placements(&plot, &hosts).unwrap();
    conduit_planner::plan_with_connection_limits(
        &plot,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        1,
        16384,
    )
    .unwrap()
}
enum Driver<'a> {
    Source {
        reference: ValueRef,
        sent: bool,
    },
    Operation(Box<dyn StepBack<PORTS> + 'a>),
    Sink {
        seen: Rc<Cell<bool>>,
        expected: Vec<f64>,
        codec: Box<FixedF32VectorCodec<64>>,
    },
}
impl StepBack<PORTS> for Driver<'_> {
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        match self {
            Self::Source { reference, sent } => {
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
            Self::Sink {
                seen,
                expected,
                codec,
            } => {
                if io.input(KPort(0)).is_none() {
                    return StepOutcome::Await;
                }
                let mut actual = [0.; 64];
                codec
                    .decode(inputs.input(KPort(0)).unwrap(), &mut actual)
                    .unwrap();
                for (a, b) in actual.iter().zip(expected) {
                    assert!((f64::from(*a) - *b).abs() < 2e-6);
                }
                io.consume(KPort(0)).unwrap();
                seen.set(true);
                StepOutcome::Complete
            }
        }
    }
    fn prepared_output(&self, port: KPort) -> Option<&[u8]> {
        if let Self::Operation(back) = self {
            back.prepared_output(port)
        } else {
            None
        }
    }
    fn step_committed(&mut self) {
        if let Self::Operation(back) = self {
            back.step_committed();
        }
    }
    fn cancel(&mut self) {
        if let Self::Operation(back) = self {
            back.cancel();
        }
    }
}
fn run(slots: u16, index_value: u16) -> bool {
    let table_bytes = packed(&(0..224 * 12).map(|i| i as f32 * 0.01).collect::<Vec<_>>());
    let table = tensor(&[224, 12], &table_bytes);
    let mut weights = vec![0.; 32 * 64];
    for i in 0..32 {
        weights[i * 64 + i] = 1.;
        weights[i * 64 + 32 + i] = -1.;
    }
    let weight_bytes = packed(&weights);
    let weights = tensor(&[32, 64], &weight_bytes);
    let bias_bytes = packed(&[0.; 64]);
    let bias = tensor(&[64], &bias_bytes);
    let features = core::array::from_fn::<_, 20, _>(|i| i as f32 * 0.05 - 0.25);
    let mut joined = features.to_vec();
    joined.extend((7 * 12..8 * 12).map(|i| i as f32 * 0.01));
    let expected: Vec<_> = joined
        .iter()
        .map(|x| f64::from(*x).tanh())
        .chain(joined.iter().map(|x| (-f64::from(*x)).tanh()))
        .collect();
    let plan = plan();
    assert!(verify_plan(&plan));
    let fragment = &plan.fragments[0];
    let (lowered, _) = conduit_plan_lowering::lowering::lower_plan_fragment_from_plan(
        &plan,
        &fragment.fragment_id,
    )
    .unwrap();
    let mut values = HostedValueStore::new(slots, 16384, u32::from(slots) * 16384).unwrap();
    let index = index_value.to_le_bytes().to_vec();
    let mut codec = FixedF32VectorCodec::<20>::prepare(&ty("NumericF32Vector20")).unwrap();
    let fixtures = [
        ("op-test/index", values.store(&index).unwrap()),
        (
            "op-test/features",
            values.store(codec.encode(&features).unwrap()).unwrap(),
        ),
        (
            "op-test/table",
            values
                .store(
                    FixedTensorPortBinding::prepare(&ty("NumericEmbedding224x12"), &table)
                        .unwrap()
                        .encoded(),
                )
                .unwrap(),
        ),
        (
            "op-test/weights",
            values
                .store(
                    FixedTensorPortBinding::prepare(&ty("NumericF32MatrixRef32x64"), &weights)
                        .unwrap()
                        .encoded(),
                )
                .unwrap(),
        ),
        (
            "op-test/bias",
            values
                .store(
                    FixedTensorPortBinding::prepare(&ty("NumericF32BiasRef64"), &bias)
                        .unwrap()
                        .encoded(),
                )
                .unwrap(),
        ),
    ];
    let seen = Rc::new(Cell::new(false));
    let drivers: Vec<_> = fragment
        .placements
        .iter()
        .enumerate()
        .map(|(n, gear)| {
            if let Some((_, reference)) = fixtures
                .iter()
                .find(|(name, _)| *name == gear.kind_id.as_str())
            {
                return Driver::Source {
                    reference: *reference,
                    sent: false,
                };
            }
            let fuel = lowered.node_specs[n].maximum_step_fuel;
            match gear.kind_id.as_str() {
                "numeric/embedding224x12" => Driver::Operation(Box::new(
                    FixedEmbeddingBack::<'_, 224, 12>::prepare_planned::<PORTS>(
                        gear,
                        fuel,
                        &table,
                        &table_bytes,
                        &access(&table),
                    )
                    .unwrap(),
                )),
                "numeric/concatenate20x12" => Driver::Operation(Box::new(
                    FixedConcatenateBack::<20, 12, 32>::prepare_planned::<PORTS>(gear, fuel)
                        .unwrap(),
                )),
                "numeric/dense32x64" => Driver::Operation(Box::new(
                    FixedAffineBack::<32, 64>::prepare_planned::<PORTS>(
                        gear,
                        fuel,
                        &weights,
                        &weight_bytes,
                        &access(&weights),
                        &bias,
                        &bias_bytes,
                        &access(&bias),
                    )
                    .unwrap(),
                )),
                "numeric/tanh64" => Driver::Operation(Box::new(
                    FixedTanhBack::<64>::prepare_planned::<PORTS>(gear, fuel).unwrap(),
                )),
                "op-test/sink" => Driver::Sink {
                    seen: seen.clone(),
                    expected: expected.clone(),
                    codec: Box::new(
                        FixedF32VectorCodec::prepare(&ty("NumericF32Vector64")).unwrap(),
                    ),
                },
                _ => panic!("unexpected operation"),
            }
        })
        .collect();
    let mut routes = FixedRoutes::<10, 9>::new(1);
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
    let mut scheduler = FixedScheduler::<_, _, _, 10, 9, PORTS, 9, 10, 9>::new(
        lowered.node_specs.try_into().unwrap(),
        lowered
            .cords
            .into_iter()
            .map(|c| c.spec)
            .collect::<Vec<_>>()
            .try_into()
            .unwrap(),
        routes,
        drivers.try_into().unwrap_or_else(|_| panic!("10 drivers")),
        values,
        HostedSignLog::new(1024, 1024 * core::mem::size_of::<KernelEvent>() as u32).unwrap(),
    )
    .unwrap();
    let mut pressure = false;
    for _ in 0..128 {
        match scheduler.step() {
            Ok(_) => {}
            Err(error) => {
                if index_value >= 224 {
                    assert!(matches!(
                        error,
                        conduit_kernel::scheduler::SchedulerError::BackFailed(
                            conduit_kernel::Failure {
                                code: conduit_kernel::FailureCode::InvalidInput,
                                detail: 1212
                            }
                        )
                    ));
                    assert!(!seen.get());
                    return false;
                }
                assert!(matches!(
                    error,
                    conduit_kernel::scheduler::SchedulerError::Storage(
                        conduit_kernel::StorageError::ItemCapacityExceeded
                    )
                ));
                pressure = true;
                break;
            }
        }
        if seen.get() {
            break;
        }
    }
    if slots == 5 {
        assert!(pressure);
        assert!(!seen.get());
        scheduler.cancel().unwrap();
        assert_eq!(
            scheduler.step().unwrap(),
            conduit_kernel::scheduler::SchedulerStatus::Cancelled
        );
    }
    seen.get()
}
#[test]
fn generic_operation_chain_executes_owned_plan_against_independent_f64_reference() {
    assert!(run(32, 7));
}
#[test]
fn resource_embedding_pressure_does_not_publish_a_partial_chain() {
    assert!(!run(5, 7));
}
#[test]
fn operation_factories_refuse_wrong_implementation_shape_and_step_budget() {
    let plan = plan();
    let mut p = plan.fragments[0]
        .placements
        .iter()
        .find(|g| g.kind_id.as_str() == "numeric/tanh64")
        .unwrap()
        .clone();
    assert!(FixedTanhBack::<64>::prepare_planned::<PORTS>(&p, 16).is_ok());
    assert!(FixedTanhBack::<64>::prepare_planned::<PORTS>(&p, 1).is_err());
    p.implementation_id = ImplementationId::from("forged");
    assert!(FixedTanhBack::<64>::prepare_planned::<PORTS>(&p, 16).is_err());
    let p = plan.fragments[0]
        .placements
        .iter()
        .find(|g| g.kind_id.as_str() == "numeric/concatenate20x12")
        .unwrap();
    assert!(FixedConcatenateBack::<20, 12, 64>::prepare_planned::<PORTS>(p, 16).is_err());
}

#[test]
fn resource_embedding_refuses_out_of_range_before_publication() {
    assert!(!run(32, 224));
}
