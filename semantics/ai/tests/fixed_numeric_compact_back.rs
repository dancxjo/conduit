#![cfg(feature = "kernel-step")]
use conduit_ai::{
    fixed_numeric_binding::*, fixed_numeric_catalog::*, fixed_numeric_codec::*,
    fixed_numeric_preparation::*, fixed_tensor_resource::*,
};

use conduit_ai::fixed_numeric_compact_back::FixedCompactBack;
use conduit_ai::fixed_numeric_compact_catalog::*;
use conduit_composite::KernelOperationFactory;
use conduit_core::*;
use conduit_data::*;
use conduit_kernel::{
    scheduler::{FixedScheduler, StepBack, StepInputBytes, StepIo, StepOutcome},
    FixedRoutes, HostedSignLog, HostedValueStore, KernelEvent, PortId as KPort, ValueRef,
    ValueStorage,
};
use conduit_plot::rust_binding::BoundedSequence;
use conduit_std_host::fixed_numeric_compact::FixedCompactOperationFactory;
use std::{cell::Cell, collections::BTreeMap, rc::Rc, sync::Arc};
const PORTS: usize = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
fn tensor(element: TensorElement, shape: &[u64], bytes: &[u8]) -> TensorValue {
    let digest = tensor_content_digest(bytes);
    TensorValue {
        element,
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
            content_profile: kind_id(if element == TensorElement::I8 {
                "tensor/elements-i8@1"
            } else {
                "tensor/elements-ieee754-f32-le@1"
            }),
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
    fixed_numeric_type(name)
        .or_else(|_| fixed_compact_type(name))
        .unwrap()
}
fn fixture_kind(name: &str, value: &StructuredInfoType, source: bool, flow: bool) -> Kind {
    let port = PortDescriptor {
        port_id: port_id("value"),
        value_kind: value.profile().unwrap().value_kind().clone(),
        direction: if source {
            PortDirection::Output
        } else {
            PortDirection::Input
        },
        temporal: if flow && (name == "numeric-test/vector" || name == "numeric-test/sink") {
            PortTemporal::Flow { closes: true }
        } else {
            PortTemporal::Value
        },
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
fn plan(flow: bool, biased: bool) -> Plan {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    install_fixed_numeric_catalogs(&mut startup, &mut profile).unwrap();
    let mut offers = vec![];
    let mut fixture_types = vec![
        ("numeric-test/vector", "NumericF32Vector4", true),
        ("numeric-test/weights", "NumericI8TiledMatrixRef4x40", true),
        ("numeric-test/sink", "NumericF32Vector40", false),
    ];
    fixture_types.push(("numeric-test/scales", "NumericF32ScaleRef40", true));
    if biased {
        fixture_types.push(("numeric-test/bias", "NumericF32BiasRef40", true));
    }
    for (name, t, source) in fixture_types {
        let kind = fixture_kind(name, &ty(t), source, flow);
        startup
            .insert(conduit_plot::KindSignature {
                kind: name.into(),
                startup_parameters: vec![],
            })
            .unwrap();
        profile.insert_kind(kind.clone()).unwrap();
        offers.push(offer(kind));
    }
    install_compact_catalogs(&mut startup, &mut profile).unwrap();
    offers.push(compact_offer(4, 40, biased, flow).unwrap());
    let operation = compact_contract(4, 40, biased, flow)
        .unwrap()
        .kind_id
        .as_str()
        .to_string();
    let bias_declaration = if biased {
        "bias: numeric-test/bias\n"
    } else {
        ""
    };
    let bias_cord = if biased {
        "bias.value >> affine.bias\n"
    } else {
        ""
    };
    let authored=format!("plot numeric-proof {{\n vector: numeric-test/vector\n weights: numeric-test/weights\n scales: numeric-test/scales\n {bias_declaration} affine: {operation}\n sink: numeric-test/sink\n vector.value >> affine.value\n weights.value >> affine.weights\n scales.value >> affine.scales\n {bias_cord} affine.result >> sink.value\n}}\n");
    let plot = conduit_plot::parse_with_startup(&authored, &startup, &profile).unwrap();
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
    conduit_planner::plan_with_connection_limits(
        &plot,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        1,
        16_384,
    )
    .unwrap()
}

enum Driver {
    Source(Vec<ValueRef>, usize, bool),
    Operation(Box<FixedCompactBack<4, 40>>),
    Hosted(Box<dyn StepBack<PORTS> + Send>, Rc<Cell<u64>>, bool),
    Sink {
        blocked: Rc<Cell<bool>>,
        seen: Rc<Cell<usize>>,
        expected: Vec<Vec<u8>>,
        staged: bool,
    },
}
impl StepBack<PORTS> for Driver {
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        match self {
            Self::Source(values, cursor, staged) => {
                if *cursor == values.len() {
                    return StepOutcome::Complete;
                }
                if !io.output_ready(KPort(0)) {
                    return StepOutcome::Await;
                }
                io.send(KPort(0), values[*cursor]).unwrap();
                *staged = true;
                StepOutcome::Progress
            }
            Self::Operation(back) => back.step(io, inputs),
            Self::Hosted(back, _, staged) => {
                let outcome = back.step(io, inputs);
                *staged = matches!(outcome, StepOutcome::Progress);
                outcome
            }
            Self::Sink {
                blocked,
                seen,
                expected,
                staged,
            } => {
                if io.input_closed(KPort(0)) {
                    assert_eq!(seen.get(), expected.len());
                    return StepOutcome::Complete;
                }
                if blocked.get() || io.input(KPort(0)).is_none() {
                    return StepOutcome::Await;
                }
                assert_eq!(inputs.input(KPort(0)).unwrap(), expected[seen.get()]);
                io.consume(KPort(0)).unwrap();
                *staged = true;
                StepOutcome::Progress
            }
        }
    }
    fn step_committed(&mut self) {
        match self {
            Self::Operation(back) => {
                <FixedCompactBack<4, 40> as StepBack<PORTS>>::step_committed(back.as_mut())
            }
            Self::Hosted(back, count, staged) if *staged => {
                back.step_committed();
                count.set(count.get() + 1);
                *staged = false;
            }
            Self::Source(_, cursor, staged) if *staged => {
                *staged = false;
                *cursor += 1;
            }
            Self::Sink { seen, staged, .. } if *staged => {
                *staged = false;
                seen.set(seen.get() + 1);
            }
            _ => {}
        }
    }
    fn prepared_output(&self, port: KPort) -> Option<&[u8]> {
        match self {
            Self::Operation(back) => {
                <FixedCompactBack<4, 40> as StepBack<PORTS>>::prepared_output(back.as_ref(), port)
            }
            Self::Hosted(back, _, _) => back.prepared_output(port),
            _ => None,
        }
    }
    fn cancel(&mut self) {
        match self {
            Self::Operation(back) => {
                <FixedCompactBack<4, 40> as StepBack<PORTS>>::cancel(back.as_mut())
            }
            Self::Hosted(back, _, staged) => {
                *staged = false;
                back.cancel();
            }
            _ => {}
        }
    }
}
fn adopted(
    element: TensorElement,
    shape: &[u64],
    bytes: Vec<u8>,
) -> Arc<AdmittedFixedTensorResource> {
    let bytes: Arc<[u8]> = Arc::from(bytes);
    let tensor = Arc::new(tensor(element, shape, &bytes));
    Arc::new(AdmittedFixedTensorResource::adopt(tensor.clone(), bytes, &access(&tensor)).unwrap())
}

type Scheduler = FixedScheduler<Driver, HostedValueStore, HostedSignLog, 6, 5, PORTS, 5, 6, 5>;
fn scheduler(
    capacity: usize,
    blocked: Rc<Cell<bool>>,
    seen: Rc<Cell<usize>>,
    hosted: bool,
) -> Scheduler {
    scheduler_profile(capacity, blocked, seen, hosted, true, true, false)
}
#[allow(clippy::too_many_arguments)]
fn scheduler_profile(
    capacity: usize,
    blocked: Rc<Cell<bool>>,
    seen: Rc<Cell<usize>>,
    hosted: bool,
    flow: bool,
    biased: bool,
    invalid: bool,
) -> Scheduler {
    let logical: Vec<i8> = (0..4 * 40).map(|i| (i % 7) as i8 - 3).collect();
    let mut tiles = vec![0u8; 160];
    for i in 0..4 {
        for o in 0..40 {
            tiles[(o / 8) * 32 + (o % 8) * 4 + i] = logical[i * 40 + o] as u8;
        }
    }
    let scale_values: Vec<f32> = (0..40).map(|o| (o + 1) as f32 / 10000.).collect();
    let bias_values: Vec<f32> = (0..40).map(|o| (o % 3) as f32 / 10.).collect();
    let weights = adopted(TensorElement::I8, &[4, 40], tiles);
    let scales = adopted(TensorElement::F32, &[40], packed(&scale_values));
    let bias = biased.then(|| adopted(TensorElement::F32, &[40], packed(&bias_values)));
    let plan = plan(flow, biased);
    let fragment = &plan.fragments[0];
    let (lowered, _) = conduit_plan_lowering::lowering::lower_plan_fragment_from_plan(
        &plan,
        &fragment.fragment_id,
    )
    .unwrap();
    let mut values =
        HostedValueStore::new(capacity.try_into().unwrap(), 16384, capacity as u32 * 16384)
            .unwrap();
    let mut input_codec = FixedF32VectorCodec::<4>::prepare(&ty("NumericF32Vector4")).unwrap();
    let mut frames = vec![[1., -1., 0.5, 0.], [0.; 4], [-1., 0.25, 0.75, 1.]];
    if !flow {
        frames.truncate(1);
    }
    if invalid {
        frames[0][0] = 1.1;
    }
    let vectors: Vec<_> = frames
        .iter()
        .map(|v| values.store(input_codec.encode(v).unwrap()).unwrap())
        .collect();
    let weight = values
        .store(
            FixedTensorPortBinding::prepare(&ty("NumericI8TiledMatrixRef4x40"), weights.tensor())
                .unwrap()
                .encoded(),
        )
        .unwrap();
    let scale = values
        .store(
            FixedTensorPortBinding::prepare(&ty("NumericF32ScaleRef40"), scales.tensor())
                .unwrap()
                .encoded(),
        )
        .unwrap();
    let bias_value = bias.as_ref().map(|b| {
        values
            .store(
                FixedTensorPortBinding::prepare(&ty("NumericF32BiasRef40"), b.tensor())
                    .unwrap()
                    .encoded(),
            )
            .unwrap()
    });
    let mut output_codec = FixedF32VectorCodec::<40>::prepare(&ty("NumericF32Vector40")).unwrap();
    let expected: Vec<_> = frames
        .iter()
        .map(|frame| {
            let output = core::array::from_fn(|o| {
                let sum: i32 = (0..4)
                    .map(|i| {
                        libm::floorf(127. * frame[i] + 0.5) as i32 * i32::from(logical[i * 40 + o])
                    })
                    .sum();
                let mut value = sum as f32 * scale_values[o];
                if biased {
                    value += bias_values[o];
                }
                value
            });
            output_codec.encode(&output).unwrap().to_vec()
        })
        .collect();
    let mut sources = BTreeMap::new();
    for gear in &fragment.placements {
        let resource = match gear.kind_id.as_str() {
            "numeric-test/weights" => Some(weights.clone()),
            "numeric-test/scales" => Some(scales.clone()),
            "numeric-test/bias" => bias.clone(),
            _ => None,
        };
        if let Some(resource) = resource {
            sources.insert(gear.placement_id.clone(), resource);
        }
    }
    let owner = FixedCompactOperationFactory::for_plan(&plan, &sources).unwrap();
    assert!(FixedCompactOperationFactory::for_plan(&plan, &BTreeMap::new()).is_err());
    let drivers: Vec<_> = fragment
        .placements
        .iter()
        .map(|gear| match gear.kind_id.as_str() {
            "numeric-test/vector" => Driver::Source(vectors.clone(), 0, false),
            "numeric-test/weights" => Driver::Source(vec![weight], 0, false),
            "numeric-test/scales" => Driver::Source(vec![scale], 0, false),
            "numeric-test/bias" => Driver::Source(vec![bias_value.unwrap()], 0, false),
            "numeric-test/sink" => Driver::Sink {
                blocked: blocked.clone(),
                seen: seen.clone(),
                expected: expected.clone(),
                staged: false,
            },
            _ if hosted => {
                assert_eq!(owner.budget(gear).unwrap().host_requests, 0);
                let mut wrong = gear.clone();
                wrong.implementation_id = ImplementationId::from("foreign-flow");
                assert!(owner.prepare(&wrong, &mut values).is_err());
                Driver::Hosted(
                    owner.prepare(gear, &mut values).unwrap(),
                    Rc::new(Cell::new(0)),
                    false,
                )
            }
            _ => Driver::Operation(Box::new(
                FixedCompactBack::prepare_planned_owned::<PORTS>(
                    gear,
                    5,
                    flow,
                    weights.clone(),
                    scales.clone(),
                    bias.clone(),
                )
                .unwrap(),
            )),
        })
        .collect();
    drop(owner);
    drop(sources);
    drop(weights);
    let mut routes = FixedRoutes::<6, 5>::new(1);
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
    let node_count = lowered.node_specs.len();
    let cord_count = lowered.cords.len();
    let node_specs = core::array::from_fn(|i| {
        lowered
            .node_specs
            .get(i)
            .copied()
            .unwrap_or(lowered.node_specs[0])
    });
    let cord_specs = core::array::from_fn(|i| {
        lowered
            .cords
            .get(i)
            .map(|c| c.spec)
            .unwrap_or(lowered.cords[0].spec)
    });
    let mut drivers = drivers.into_iter();
    let drivers =
        core::array::from_fn(|_| drivers.next().unwrap_or(Driver::Source(vec![], 0, false)));
    FixedScheduler::new_with_active_counts(
        node_count,
        cord_count,
        node_specs,
        cord_specs,
        routes,
        drivers,
        values,
        HostedSignLog::new(1024, 1024 * core::mem::size_of::<KernelEvent>() as u32).unwrap(),
    )
    .unwrap()
}
fn frame_count(scheduler: &Scheduler) -> u64 {
    scheduler
        .drivers()
        .iter()
        .find_map(|d| {
            if let Driver::Operation(back) = d {
                Some(back.committed_frames())
            } else if let Driver::Hosted(_, count, _) = d {
                Some(count.get())
            } else {
                None
            }
        })
        .unwrap()
}
#[test]
fn closing_flow_reuses_one_prepared_owner_for_three_frames() {
    let blocked = Rc::new(Cell::new(false));
    let seen = Rc::new(Cell::new(0));
    let mut scheduler = scheduler(16, blocked.clone(), seen.clone(), false);
    for _ in 0..80 {
        scheduler.step().unwrap();
        if seen.get() == 3 {
            break;
        }
    }
    assert_eq!(frame_count(&scheduler), 3);
    assert_eq!(seen.get(), 3);
    for _ in 0..12 {
        scheduler.step().unwrap();
    }
    assert_eq!(frame_count(&scheduler), 3);
    let value = compact_offer(4, 40, true, false).unwrap();
    assert!(value
        .inputs
        .iter()
        .all(|p| p.temporal == PortTemporal::Value));
}
#[test]
fn queue_pressure_and_cancellation_preserve_last_committed_flow_frame() {
    let blocked = Rc::new(Cell::new(true));
    let seen = Rc::new(Cell::new(0));
    let mut scheduler = scheduler(16, blocked, seen.clone(), false);
    for _ in 0..40 {
        scheduler.step().unwrap();
    }
    assert_eq!(frame_count(&scheduler), 1);
    assert_eq!(seen.get(), 0);
    scheduler.cancel().unwrap();
    assert_eq!(
        scheduler.step().unwrap(),
        conduit_kernel::scheduler::SchedulerStatus::Cancelled
    );
    assert_eq!(frame_count(&scheduler), 1);
    assert_eq!(seen.get(), 0);
}
#[test]
fn failed_storage_transaction_and_cancellation_do_not_bind_or_advance_flow() {
    let seen = Rc::new(Cell::new(0));
    let mut scheduler = scheduler(6, Rc::new(Cell::new(false)), seen.clone(), false);
    let mut refused = false;
    for _ in 0..32 {
        if let Err(error) = scheduler.step() {
            assert!(matches!(
                error,
                conduit_kernel::scheduler::SchedulerError::Storage(
                    conduit_kernel::StorageError::ItemCapacityExceeded
                )
            ));
            refused = true;
            break;
        }
    }
    assert!(refused);
    assert_eq!(frame_count(&scheduler), 0);
    assert_eq!(seen.get(), 0);
    assert!(scheduler
        .drivers()
        .iter()
        .any(|d| matches!(d, Driver::Operation(back) if !back.resources_bound())));
    scheduler.cancel().unwrap();
    assert_eq!(
        scheduler.step().unwrap(),
        conduit_kernel::scheduler::SchedulerStatus::Cancelled
    );
    assert_eq!(frame_count(&scheduler), 0);
}

#[test]
fn hosted_flow_factory_follows_exact_cords_and_outlives_loader() {
    let seen = Rc::new(Cell::new(0));
    let mut scheduler = scheduler(16, Rc::new(Cell::new(false)), seen.clone(), true);
    for _ in 0..80 {
        scheduler.step().unwrap();
        if seen.get() == 3 {
            break;
        }
    }
    assert_eq!(seen.get(), 3);
    assert_eq!(frame_count(&scheduler), 3);
}

#[test]
fn unbiased_and_value_profiles_use_only_their_declared_resources() {
    for (flow, biased) in [(true, false), (false, false), (false, true)] {
        let seen = Rc::new(Cell::new(0));
        let mut scheduler = scheduler_profile(
            16,
            Rc::new(Cell::new(false)),
            seen.clone(),
            true,
            flow,
            biased,
            false,
        );
        let count = if flow { 3 } else { 1 };
        for _ in 0..100 {
            scheduler.step().unwrap();
        }
        assert_eq!(seen.get(), count);
        assert_eq!(frame_count(&scheduler), count as u64);
    }
}
#[test]
fn outside_quantization_domain_refuses_without_binding_or_publication() {
    let seen = Rc::new(Cell::new(0));
    let mut scheduler = scheduler_profile(
        16,
        Rc::new(Cell::new(false)),
        seen.clone(),
        false,
        true,
        true,
        true,
    );
    let mut refused = false;
    for _ in 0..64 {
        if let Err(error) = scheduler.step() {
            assert!(matches!(
                error,
                conduit_kernel::scheduler::SchedulerError::BackFailed(conduit_kernel::Failure {
                    code: conduit_kernel::FailureCode::InvalidInput,
                    ..
                })
            ));
            refused = true;
            break;
        }
    }
    assert!(refused);
    assert_eq!(seen.get(), 0);
    assert_eq!(frame_count(&scheduler), 0);
    assert!(scheduler
        .drivers()
        .iter()
        .any(|d| matches!(d,Driver::Operation(back) if !back.resources_bound())));
}

#[test]
fn exact_signed_tensor_profiles_refuse_foreign_shape_and_element() {
    let weights = adopted(TensorElement::I8, &[4, 40], vec![0; 160]);
    assert!(FixedTensorPortBinding::prepare(
        &ty("NumericI8TiledMatrixRef128x40"),
        weights.tensor()
    )
    .is_err());
    let float_weights = adopted(TensorElement::F32, &[4, 40], packed(&[0.; 160]));
    assert!(FixedTensorPortBinding::prepare(
        &ty("NumericI8TiledMatrixRef4x40"),
        float_weights.tensor()
    )
    .is_err());
    let scales = adopted(TensorElement::F32, &[40], packed(&[1.; 40]));
    assert!(
        FixedTensorPortBinding::prepare(&ty("NumericF32ScaleRef128"), scales.tensor()).is_err()
    );
}
#[test]
fn nonpositive_or_nonfinite_scale_refuses_during_selected_owner_preparation() {
    let plan = plan(true, false);
    let gear = plan.fragments[0]
        .placements
        .iter()
        .find(|gear| gear.implementation_id.as_str() == COMPACT_IMPLEMENTATION)
        .unwrap();
    for invalid in [0., -1., f32::NAN, f32::INFINITY] {
        let weights = adopted(TensorElement::I8, &[4, 40], vec![0; 160]);
        let scales = adopted(TensorElement::F32, &[40], packed(&[invalid; 40]));
        assert!(matches!(
            FixedCompactBack::<4, 40>::prepare_planned_owned::<PORTS>(
                gear, 5, true, weights, scales, None
            ),
            Err(
                conduit_ai::fixed_numeric_compact_back::CompactPreparationRefusal::Tensor(
                    conduit_ai::fixed_compact::CompactTensorRefusal::Scale
                )
            )
        ));
    }
}
