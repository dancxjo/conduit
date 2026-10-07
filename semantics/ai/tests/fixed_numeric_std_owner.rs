#![cfg(feature = "kernel-step")]
use conduit_ai::{
    fixed_numeric_binding::*, fixed_numeric_catalog::*, fixed_numeric_codec::*,
    fixed_numeric_preparation::*, fixed_tensor_resource::*,
};
use conduit_composite::KernelOperationFactory;
use conduit_core::*;
use conduit_data::*;
use conduit_kernel::{
    scheduler::{FixedScheduler, StepBack, StepInputBytes, StepIo, StepOutcome},
    FixedRoutes, HostedSignLog, HostedValueStore, KernelEvent, PortId as KPort, ValueRef,
    ValueStorage,
};
use conduit_plot::rust_binding::BoundedSequence;
use conduit_std_host::fixed_numeric::*;
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
    fixed_numeric_type(name).unwrap()
}
fn fixture_kind(name: &str, value: &StructuredInfoType, source: bool) -> Kind {
    let port = PortDescriptor {
        port_id: port_id("value"),
        value_kind: value.profile().unwrap().value_kind().clone(),
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
    for (name, t, source) in [
        ("numeric-test/vector", "NumericF32Vector3", true),
        ("numeric-test/weights", "NumericF32MatrixRef3x2", true),
        ("numeric-test/bias", "NumericF32BiasRef2", true),
        ("numeric-test/sink", "NumericF32Vector2", false),
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
    offers.push(fixed_numeric_offer("numeric/dense3x2").unwrap());
    let plot=conduit_plot::parse_with_startup("plot numeric-proof {\n vector: numeric-test/vector\n weights: numeric-test/weights\n bias: numeric-test/bias\n affine: numeric/dense3x2\n sink: numeric-test/sink\n vector.value >> affine.value\n weights.value >> affine.weights\n bias.value >> affine.bias\n affine.result >> sink.value\n}\n",&startup,&profile).unwrap();
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
    Source(ValueRef, bool, bool),
    Operation(Box<dyn StepBack<PORTS> + Send>),
    Sink(Rc<Cell<bool>>, Vec<u8>),
}
impl StepBack<PORTS> for Driver {
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        match self {
            Self::Source(reference, staged, sent) => {
                if *sent {
                    return StepOutcome::Complete;
                }
                if !io.output_ready(KPort(0)) {
                    return StepOutcome::Await;
                }
                io.send(KPort(0), *reference).unwrap();
                *staged = true;
                StepOutcome::Progress
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
        match self {
            Self::Operation(back) => back.step_committed(),
            Self::Source(_, staged, sent) if *staged => {
                *staged = false;
                *sent = true;
            }
            _ => {}
        }
    }
    fn prepared_output(&self, port: KPort) -> Option<&[u8]> {
        if let Self::Operation(back) = self {
            back.prepared_output(port)
        } else {
            None
        }
    }
    fn cancel(&mut self) {
        if let Self::Operation(back) = self {
            back.cancel();
        }
    }
}
fn adopted(shape: &[u64], values: &[f32]) -> Arc<AdmittedFixedTensorResource> {
    let bytes: Arc<[u8]> = Arc::from(packed(values));
    let tensor = Arc::new(tensor(shape, &bytes));
    Arc::new(AdmittedFixedTensorResource::adopt(tensor.clone(), bytes, &access(&tensor)).unwrap())
}
#[test]
fn std_numeric_factory_follows_planned_tensor_cords_and_retains_custody() {
    let weights = adopted(&[3, 2], &[1., 4., 2., 5., 3., 6.]);
    let bias = adopted(&[2], &[0.5, -0.5]);
    let plan = plan();
    let fragment = &plan.fragments[0];
    let (lowered, _) = conduit_plan_lowering::lowering::lower_plan_fragment_from_plan(
        &plan,
        &fragment.fragment_id,
    )
    .unwrap();
    let mut sources = BTreeMap::new();
    for gear in &fragment.placements {
        match gear.kind_id.as_str() {
            "numeric-test/weights" => {
                sources.insert(gear.placement_id.clone(), weights.clone());
            }
            "numeric-test/bias" => {
                sources.insert(gear.placement_id.clone(), bias.clone());
            }
            _ => {}
        }
    }
    let owners = FixedNumericOperationFactory::for_plan(&plan, &sources).unwrap();
    assert_eq!(owners.len(), 1);
    let owner = &owners[0];
    let mut values = HostedValueStore::new(8, 16384, 8 * 16384).unwrap();
    let vector = values
        .store(
            FixedF32VectorCodec::<3>::prepare(&ty("NumericF32Vector3"))
                .unwrap()
                .encode(&[2., -1., 0.5])
                .unwrap(),
        )
        .unwrap();
    let weight = values
        .store(
            FixedTensorPortBinding::prepare(&ty("NumericF32MatrixRef3x2"), weights.tensor())
                .unwrap()
                .encoded(),
        )
        .unwrap();
    let bias_value = values
        .store(
            FixedTensorPortBinding::prepare(&ty("NumericF32BiasRef2"), bias.tensor())
                .unwrap()
                .encoded(),
        )
        .unwrap();
    let expected = FixedF32VectorCodec::<2>::prepare(&ty("NumericF32Vector2"))
        .unwrap()
        .encode(&[2., 5.5])
        .unwrap()
        .to_vec();
    let seen = Rc::new(Cell::new(false));
    let drivers: Vec<_> = fragment
        .placements
        .iter()
        .map(|gear| match gear.kind_id.as_str() {
            "numeric-test/vector" => Driver::Source(vector, false, false),
            "numeric-test/weights" => Driver::Source(weight, false, false),
            "numeric-test/bias" => Driver::Source(bias_value, false, false),
            "numeric-test/sink" => Driver::Sink(seen.clone(), expected.clone()),
            _ => {
                let budget = owner.budget(gear).unwrap();
                assert_eq!(budget.host_requests, 0);
                let mut wrong = gear.clone();
                wrong.implementation_id = ImplementationId::from("wrong");
                assert!(owner.prepare(&wrong, &mut values).is_err());
                Driver::Operation(owner.prepare(gear, &mut values).unwrap())
            }
        })
        .collect();
    drop(owners);
    drop(sources);
    drop(weights);
    drop(bias);
    let mut routes = FixedRoutes::<5, 4>::new(1);
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
    let mut scheduler = FixedScheduler::<_, _, _, 5, 4, PORTS, 4, 5, 4>::new(
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
#[test]
fn std_numeric_owner_covers_exact_catalog_and_refuses_missing_custody() {
    for contract in fixed_numeric_contracts().unwrap() {
        let offer = fixed_numeric_offer(contract.kind_id.as_str()).unwrap();
        assert_eq!(offer.kind_id, contract.kind_id);
    }
    for (_, _, contract) in
        conduit_ai::fixed_numeric_pair_catalog::fixed_numeric_pair_contracts().unwrap()
    {
        assert_eq!(
            fixed_numeric_offer(contract.kind_id.as_str())
                .unwrap()
                .kind_id,
            contract.kind_id
        );
    }
    assert!(FixedNumericOperationFactory::for_plan(&plan(), &BTreeMap::new()).is_err());
    assert!(fixed_numeric_offer("numeric/hidden-model-infer").is_err());
}
