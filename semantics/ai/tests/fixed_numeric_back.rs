#![cfg(feature = "kernel-step")]
use conduit_ai::{
    fixed_numeric_back::*, fixed_numeric_binding::*, fixed_numeric_catalog::*,
    fixed_numeric_codec::*, fixed_numeric_preparation::*, fixed_tensor::FixedMatrixOrder,
};
use conduit_core::*;
use conduit_data::*;
use conduit_kernel::{
    scheduler::{FixedScheduler, StepBack, StepInputBytes, StepIo, StepOutcome},
    FixedRoutes, HostedSignLog, HostedValueStore, KernelEvent, PortId as KPort, SignQuery,
    ValueRef, ValueStorage,
};
use conduit_plot::rust_binding::BoundedSequence;
const PORTS: usize = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
use std::{cell::Cell, rc::Rc};

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
fn prepare<'a>(
    weights: &'a TensorValue,
    weight_bytes: &'a [u8],
    bias: &'a TensorValue,
    bias_bytes: &'a [u8],
) -> FixedAffineBack<'a, 3, 2> {
    FixedAffineBack::prepare(
        &ty("NumericF32Vector3"),
        &ty("NumericF32Vector2"),
        &ty("NumericF32MatrixRef3x2"),
        &ty("NumericF32BiasRef2"),
        weights,
        weight_bytes,
        bias,
        bias_bytes,
        FixedMatrixOrder::InputMajor,
    )
    .unwrap()
}

#[test]
fn prepared_codec_rejects_nonfinite_wrong_schema_and_truncation_atomically() {
    let mut codec = FixedF32VectorCodec::<3>::prepare(&ty("NumericF32Vector3")).unwrap();
    let encoded = codec.encode(&[2., -1., 0.5]).unwrap().to_vec();
    let mut output = [99.; 3];
    codec.decode(&encoded, &mut output).unwrap();
    assert_eq!(output, [2., -1., 0.5]);
    let before = codec.encoded().to_vec();
    assert_eq!(
        codec.encode(&[1., f32::NAN, 2.]),
        Err(FixedCodecRefusal::Nonfinite)
    );
    assert_eq!(codec.encoded(), before);
    let mut corrupt = encoded.clone();
    corrupt[0] ^= 1;
    for malformed in [&corrupt[..], &encoded[..encoded.len() - 1]] {
        let before = output;
        assert!(codec.decode(malformed, &mut output).is_err());
        assert_eq!(output, before);
    }
    assert!(FixedF32VectorCodec::<2>::prepare(&ty("NumericF32Vector3")).is_err());
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
    offers.push(fixed_affine_offer::<3, 2>().unwrap());
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

enum Driver<'a> {
    Source {
        reference: ValueRef,
        sent: bool,
    },
    Affine(Box<FixedAffineBack<'a, 3, 2>>),
    Sink {
        pause: Rc<Cell<bool>>,
        seen: Rc<Cell<bool>>,
        expected: Vec<u8>,
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
            Self::Affine(back) => back.step(io, inputs),
            Self::Sink {
                pause,
                seen,
                expected,
            } => {
                if pause.get() {
                    io.exhaust_fuel();
                    return StepOutcome::Yield;
                }
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
        if let Self::Affine(back) = self {
            <FixedAffineBack<3, 2> as StepBack<PORTS>>::step_committed(back);
        }
    }
    fn prepared_output(&self, port: KPort) -> Option<&[u8]> {
        if let Self::Affine(back) = self {
            <FixedAffineBack<3, 2> as StepBack<PORTS>>::prepared_output(back, port)
        } else {
            None
        }
    }
    fn cancel(&mut self) {
        if let Self::Affine(back) = self {
            <FixedAffineBack<3, 2> as StepBack<PORTS>>::cancel(back);
        }
    }
}

type Scheduler<'a> =
    FixedScheduler<Driver<'a>, HostedValueStore, HostedSignLog, 5, 4, PORTS, 4, 5, 4>;
fn scheduler<'a>(
    weights: &'a TensorValue,
    weight_bytes: &'a [u8],
    bias: &'a TensorValue,
    bias_bytes: &'a [u8],
    pause: Rc<Cell<bool>>,
    seen: Rc<Cell<bool>>,
    slots: u16,
) -> Scheduler<'a> {
    let plan = plan();
    assert!(verify_plan(&plan));
    let fragment = &plan.fragments[0];
    let (lowered, _) = conduit_plan_lowering::lowering::lower_plan_fragment_from_plan(
        &plan,
        &fragment.fragment_id,
    )
    .unwrap();
    let mut values = HostedValueStore::new(slots, 16_384, u32::from(slots) * 16_384).unwrap();
    let mut codec = FixedF32VectorCodec::<3>::prepare(&ty("NumericF32Vector3")).unwrap();
    let vector = values
        .store(codec.encode(&[2., -1., 0.5]).unwrap())
        .unwrap();
    let weight = values
        .store(
            FixedTensorPortBinding::prepare(&ty("NumericF32MatrixRef3x2"), weights)
                .unwrap()
                .encoded(),
        )
        .unwrap();
    let bias_value = values
        .store(
            FixedTensorPortBinding::prepare(&ty("NumericF32BiasRef2"), bias)
                .unwrap()
                .encoded(),
        )
        .unwrap();
    let mut output = FixedF32VectorCodec::<2>::prepare(&ty("NumericF32Vector2")).unwrap();
    let expected = output.encode(&[2., 5.5]).unwrap().to_vec();
    let drivers: Vec<_> = fragment
        .placements
        .iter()
        .map(|gear| match gear.kind_id.as_str() {
            "numeric-test/vector" => Driver::Source {
                reference: vector,
                sent: false,
            },
            "numeric-test/weights" => Driver::Source {
                reference: weight,
                sent: false,
            },
            "numeric-test/bias" => Driver::Source {
                reference: bias_value,
                sent: false,
            },
            "numeric/dense3x2" => Driver::Affine(Box::new(
                FixedAffineBack::prepare_planned::<PORTS>(
                    gear,
                    lowered.node_specs[fragment
                        .placements
                        .iter()
                        .position(|placement| placement.gear_id == gear.gear_id)
                        .unwrap()]
                    .maximum_step_fuel,
                    weights,
                    weight_bytes,
                    &access(weights),
                    bias,
                    bias_bytes,
                    &access(bias),
                )
                .unwrap(),
            )),
            "numeric-test/sink" => Driver::Sink {
                pause: pause.clone(),
                seen: seen.clone(),
                expected: expected.clone(),
            },
            _ => panic!("unexpected fixture"),
        })
        .collect();
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
    FixedScheduler::new(
        lowered.node_specs.try_into().unwrap(),
        lowered
            .cords
            .into_iter()
            .map(|cord| cord.spec)
            .collect::<Vec<_>>()
            .try_into()
            .unwrap(),
        routes,
        drivers
            .try_into()
            .unwrap_or_else(|_| panic!("five drivers")),
        values,
        HostedSignLog::new(1024, 1024 * core::mem::size_of::<KernelEvent>() as u32).unwrap(),
    )
    .unwrap()
}
#[test]
fn ordinary_plan_play_executes_exact_borrowed_tensor_affine() {
    let bytes = packed(&[1., 4., 2., 5., 3., 6.]);
    let weights = tensor(&[3, 2], &bytes);
    let biases = packed(&[0.5, -0.5]);
    let bias = tensor(&[2], &biases);
    let pause = Rc::new(Cell::new(false));
    let seen = Rc::new(Cell::new(false));
    let mut scheduler = scheduler(&weights, &bytes, &bias, &biases, pause, seen.clone(), 8);
    for _ in 0..64 {
        scheduler.step().unwrap();
        if seen.get() {
            break;
        }
    }
    assert!(seen.get());
    assert!(scheduler
        .signs()
        .contains_kind(conduit_kernel::KernelEventKind::ValueConsumed));
}

#[test]
fn ordinary_scheduler_storage_refusal_does_not_commit_and_cancellation_is_distinct() {
    let bytes = packed(&[1., 4., 2., 5., 3., 6.]);
    let weights = tensor(&[3, 2], &bytes);
    let biases = packed(&[0.5, -0.5]);
    let bias = tensor(&[2], &biases);
    let seen = Rc::new(Cell::new(false));
    // Three source values occupy every store slot. Producing the result refuses
    // before the transaction can consume inputs or commit Back completion.
    let mut scheduler = scheduler(
        &weights,
        &bytes,
        &bias,
        &biases,
        Rc::new(Cell::new(false)),
        seen.clone(),
        3,
    );
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
    assert!(!seen.get());
    assert!(scheduler
        .drivers()
        .iter()
        .any(|driver| matches!(driver,Driver::Affine(back) if !back.has_committed_result())));
    scheduler.cancel().unwrap();
    assert_eq!(
        scheduler.step().unwrap(),
        conduit_kernel::scheduler::SchedulerStatus::Cancelled
    );
    assert!(scheduler
        .signs()
        .contains_kind(conduit_kernel::KernelEventKind::RunCancelled));
    assert!(!seen.get());
}

#[test]
fn planned_factory_refuses_wrong_identity_and_lost_or_stale_resource() {
    let bytes = packed(&[1., 4., 2., 5., 3., 6.]);
    let weights = tensor(&[3, 2], &bytes);
    let biases = packed(&[0.5, -0.5]);
    let bias = tensor(&[2], &biases);
    let plan = plan();
    let mut placement = plan.fragments[0]
        .placements
        .iter()
        .find(|gear| gear.kind_id.as_str() == "numeric/dense3x2")
        .unwrap()
        .clone();
    let ready = |placement: &PlannedGear, grant: &ResourceReferenceBinding| {
        FixedAffineBack::<3, 2>::prepare_planned::<PORTS>(
            placement,
            16,
            &weights,
            &bytes,
            grant,
            &bias,
            &biases,
            &access(&bias),
        )
    };
    assert!(ready(&placement, &access(&weights)).is_ok());
    placement.artifact_id = ArtifactId::from("wrong");
    assert!(matches!(
        ready(&placement, &access(&weights)),
        Err(FixedAffineBackPreparationRefusal::Planned(
            FixedPlannedRefusal::Identity
        ))
    ));
    let placement = plan.fragments[0]
        .placements
        .iter()
        .find(|gear| gear.kind_id.as_str() == "numeric/dense3x2")
        .unwrap();
    for availability in [
        ResourceReferenceAvailability::Lost,
        ResourceReferenceAvailability::Stale,
    ] {
        let mut grant = access(&weights);
        grant.availability = availability;
        assert!(matches!(
            ready(placement, &grant),
            Err(FixedAffineBackPreparationRefusal::Planned(
                FixedPlannedRefusal::Resource(_)
            ))
        ));
    }
}

#[test]
fn blocked_output_does_not_consume_inputs_or_commit_private_completion() {
    let bytes = packed(&[1., 4., 2., 5., 3., 6.]);
    let weights = tensor(&[3, 2], &bytes);
    let biases = packed(&[0.5, -0.5]);
    let bias = tensor(&[2], &biases);
    let mut back = prepare(&weights, &bytes, &bias, &biases);
    let mut codec = FixedF32VectorCodec::<3>::prepare(&ty("NumericF32Vector3")).unwrap();
    let vector = codec.encode(&[2., -1., 0.5]).unwrap().to_vec();
    let weights_port =
        FixedTensorPortBinding::prepare(&ty("NumericF32MatrixRef3x2"), &weights).unwrap();
    let bias_port = FixedTensorPortBinding::prepare(&ty("NumericF32BiasRef2"), &bias).unwrap();
    let bytes = [&vector[..], weights_port.encoded(), bias_port.encoded()];
    let refs = core::array::from_fn(|p| {
        Some(ValueRef {
            slot: p as u16,
            generation: 1,
            byte_len: bytes[p].len() as u32,
        })
    });
    let mut io = StepIo::<3>::test_frame(refs, [false; 3], [None; 3], None, 16);
    let inputs = StepInputBytes::test_frame(bytes.map(Some), None);
    assert_eq!(back.step(&mut io, &inputs), StepOutcome::Await);
    assert!(!back.has_committed_result());
    assert!((0..3).all(|p| !io.test_consumed(KPort(p))));
}

#[test]
fn every_owned_conditioning_vector_fits_selected_envelope() {
    fn check<const N: usize>() {
        let ty =
            conduit_ai::fixed_numeric_catalog::fixed_numeric_type(&format!("NumericF32Vector{N}"))
                .unwrap();
        let codec =
            conduit_ai::fixed_numeric_codec::FixedF32VectorCodec::<N>::prepare(&ty).unwrap();
        assert!(codec.maximum_bytes() <= 16_384);
        println!("vector {N}: {} canonical bytes", codec.maximum_bytes());
    }
    check::<12>();
    check::<20>();
    check::<32>();
    check::<64>();
    check::<128>();
    check::<192>();
    check::<320>();
}
