#![cfg(feature = "kernel-step")]
use conduit_ai::{
    fixed_numeric_binding::*, fixed_numeric_catalog::*, fixed_numeric_codec::*,
    fixed_numeric_preparation::*, fixed_tensor_resource::*,
};

use conduit_ai::fixed_numeric_embedding_flow::*;
use conduit_composite::KernelOperationFactory;
use conduit_core::*;
use conduit_data::*;
use conduit_kernel::{
    scheduler::{FixedScheduler, StepBack, StepInputBytes, StepIo, StepOutcome},
    FixedRoutes, HostedSignLog, HostedValueStore, KernelEvent, PortId as KPort, ValueRef,
    ValueStorage,
};
use conduit_plot::rust_binding::BoundedSequence;
use conduit_std_host::fixed_numeric_embedding_flow::FixedEmbeddingFlowOperationFactory;
use std::{cell::Cell, collections::BTreeMap, rc::Rc, sync::Arc};
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
    if name == "U16" {
        StructuredInfoType::leaf(kind_id("value/u16")).unwrap()
    } else {
        fixed_numeric_type(name).unwrap()
    }
}
fn fixture_kind(name: &str, value: &StructuredInfoType, source: bool) -> Kind {
    let port = PortDescriptor {
        port_id: port_id("value"),
        value_kind: if name == "numeric-test/vector" {
            kind_id("value/u16")
        } else {
            value.profile().unwrap().value_kind().clone()
        },
        direction: if source {
            PortDirection::Output
        } else {
            PortDirection::Input
        },
        temporal: if name == "numeric-test/vector" || name == "numeric-test/sink" {
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
fn plan() -> Plan {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    install_fixed_numeric_catalogs(&mut startup, &mut profile).unwrap();
    let mut offers = vec![];
    for (name, t, source) in [
        ("numeric-test/vector", "U16", true),
        ("numeric-test/weights", "NumericEmbedding224x12", true),
        ("numeric-test/sink", "NumericF32Vector12", false),
    ] {
        let kind = fixture_kind(name, &ty(t), source);
        startup
            .insert(conduit_plot::KindSignature {
                kind: name.into(),
                startup_parameters: vec![],
            })
            .unwrap();
        profile.insert_kind(kind.clone()).unwrap();
        offers.push(offer(kind));
    }
    install_embedding_flow_catalogs(&mut startup, &mut profile).unwrap();
    offers.push(embedding_flow_offer::<224, 12>().unwrap());
    let plot=conduit_plot::parse_with_startup("plot numeric-proof {\n vector: numeric-test/vector\n weights: numeric-test/weights\n affine: numeric/flow-embedding224x12\n sink: numeric-test/sink\n vector.value >> affine.index\n weights.value >> affine.weights\n affine.result >> sink.value\n}\n",&startup,&profile).unwrap();
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
    Operation(Box<FixedEmbeddingFlowBack<224, 12>>),
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
                <FixedEmbeddingFlowBack<224, 12> as StepBack<PORTS>>::step_committed(back.as_mut())
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
                <FixedEmbeddingFlowBack<224, 12> as StepBack<PORTS>>::prepared_output(
                    back.as_ref(),
                    port,
                )
            }
            Self::Hosted(back, _, _) => back.prepared_output(port),
            _ => None,
        }
    }
    fn cancel(&mut self) {
        match self {
            Self::Operation(back) => {
                <FixedEmbeddingFlowBack<224, 12> as StepBack<PORTS>>::cancel(back.as_mut())
            }
            Self::Hosted(back, _, staged) => {
                *staged = false;
                back.cancel();
            }
            _ => {}
        }
    }
}
fn adopted(shape: &[u64], values: &[f32]) -> Arc<AdmittedFixedTensorResource> {
    let bytes: Arc<[u8]> = Arc::from(packed(values));
    let tensor = Arc::new(tensor(shape, &bytes));
    Arc::new(AdmittedFixedTensorResource::adopt(tensor.clone(), bytes, &access(&tensor)).unwrap())
}

type Scheduler = FixedScheduler<Driver, HostedValueStore, HostedSignLog, 4, 3, PORTS, 3, 4, 3>;
fn scheduler(
    capacity: usize,
    blocked: Rc<Cell<bool>>,
    seen: Rc<Cell<usize>>,
    hosted: bool,
) -> Scheduler {
    scheduler_indices(capacity, blocked, seen, hosted, &[0, 223, 17])
}
fn scheduler_indices(
    capacity: usize,
    blocked: Rc<Cell<bool>>,
    seen: Rc<Cell<usize>>,
    hosted: bool,
    indices: &[u16],
) -> Scheduler {
    let table: Vec<f32> = (0..224 * 12).map(|i| i as f32 / 16.).collect();
    let weights = adopted(&[224, 12], &table);
    let plan = plan();
    let fragment = &plan.fragments[0];
    let (lowered, _) = conduit_plan_lowering::lowering::lower_plan_fragment_from_plan(
        &plan,
        &fragment.fragment_id,
    )
    .unwrap();
    let mut values =
        HostedValueStore::new(capacity.try_into().unwrap(), 16384, capacity as u32 * 16384)
            .unwrap();
    let vectors: Vec<_> = indices
        .iter()
        .map(|i| values.store(&i.to_le_bytes()).unwrap())
        .collect();
    let weight = values
        .store(
            FixedTensorPortBinding::prepare(&ty("NumericEmbedding224x12"), weights.tensor())
                .unwrap()
                .encoded(),
        )
        .unwrap();
    let mut output_codec = FixedF32VectorCodec::<12>::prepare(&ty("NumericF32Vector12")).unwrap();
    let expected: Vec<_> = indices
        .iter()
        .filter(|row| **row < 224)
        .map(|row| {
            let row = usize::from(*row);
            let slice: [f32; 12] = table[row * 12..row * 12 + 12].try_into().unwrap();
            output_codec.encode(&slice).unwrap().to_vec()
        })
        .collect();
    let mut sources = BTreeMap::new();
    for gear in &fragment.placements {
        if gear.kind_id.as_str() == "numeric-test/weights" {
            sources.insert(gear.placement_id.clone(), weights.clone());
        }
    }
    let owner = FixedEmbeddingFlowOperationFactory::for_plan(&plan, &sources).unwrap();
    assert!(FixedEmbeddingFlowOperationFactory::for_plan(&plan, &BTreeMap::new()).is_err());
    let drivers: Vec<_> = fragment
        .placements
        .iter()
        .map(|gear| match gear.kind_id.as_str() {
            "numeric-test/vector" => Driver::Source(vectors.clone(), 0, false),
            "numeric-test/weights" => Driver::Source(vec![weight], 0, false),
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
                FixedEmbeddingFlowBack::prepare_planned_owned::<PORTS>(gear, 4, weights.clone())
                    .unwrap(),
            )),
        })
        .collect();
    drop(owner);
    drop(sources);
    drop(weights);
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
    FixedScheduler::new(
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
    let value =
        conduit_ai::fixed_numeric_operations_back::fixed_embedding_offer::<224, 12>().unwrap();
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
    let mut scheduler = scheduler(4, Rc::new(Cell::new(false)), seen.clone(), false);
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
fn out_of_range_lookup_refuses_without_binding_or_publishing() {
    for index in [224u16, u16::MAX] {
        let seen = Rc::new(Cell::new(0));
        let mut scheduler =
            scheduler_indices(16, Rc::new(Cell::new(false)), seen.clone(), false, &[index]);
        let mut refused = false;
        for _ in 0..32 {
            if let Err(error) = scheduler.step() {
                assert!(matches!(
                    error,
                    conduit_kernel::scheduler::SchedulerError::BackFailed(
                        conduit_kernel::Failure {
                            code: conduit_kernel::FailureCode::InvalidInput,
                            ..
                        }
                    )
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
            .any(|d| matches!(d, Driver::Operation(back) if !back.resources_bound())));
    }
}
