#![cfg(feature = "kernel-step")]
use conduit_ai::fixed_numeric_float_integer::*;
use conduit_composite::KernelOperationFactory;
use conduit_core::*;
use conduit_kernel::{
    scheduler::{FixedScheduler, StepBack, StepInputBytes, StepIo, StepOutcome},
    FixedRoutes, HostedSignLog, HostedValueStore, KernelEvent, PortId as KPort, ValueRef,
    ValueStorage,
};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::Rc,
};
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
const PORTS: usize = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
fn bound() -> u32 {
    ["NumericF32Vector160", "NumericI16Vector160"]
        .into_iter()
        .map(|name| {
            conduit_plot::maximum_prepared_transport_value_bytes(
                &conduit_ai::fixed_numeric_catalog::fixed_numeric_type(name).unwrap(),
            )
            .unwrap()
        })
        .max()
        .unwrap()
}
fn fixture(name: &str, source: bool, flow: bool) -> Kind {
    let ty = conduit_ai::fixed_numeric_catalog::fixed_numeric_type(if source {
        "NumericF32Vector160"
    } else {
        "NumericI16Vector160"
    })
    .unwrap();
    let identity = ty.profile().unwrap().value_kind().clone();
    let value_bound = conduit_plot::maximum_prepared_transport_value_bytes(&ty).unwrap();
    let port = PortDescriptor {
        port_id: port_id("value"),
        value_kind: identity,
        direction: if source {
            PortDirection::Output
        } else {
            PortDirection::Input
        },
        temporal: if flow {
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
            contract: CheckedValueContract::new(port.value_kind, value_bound, vec![]).unwrap(),
        }])],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: bound(),
        },
    }
}
fn fixture_offer(kind: Kind) -> CapabilityOffer {
    let id = String::from(kind.kind_id.as_str());
    BackOfferBuilder::new(
        kind,
        Back {
            capability_id: CapabilityId::from(id.clone()),
            execution_profile_id: ExecutionProfileId::from("float-fixture@1"),
            implementation_id: ImplementationId::from(id.clone()),
            artifact_id: ArtifactId::from(id),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}
fn plan(flow: bool) -> Plan {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    conduit_ai::fixed_numeric_catalog::install_fixed_numeric_catalogs(&mut startup, &mut profiles)
        .unwrap();
    install_float_integer_catalogs(&mut startup, &mut profiles).unwrap();
    let mut offers = vec![float_integer_offer(flow).unwrap()];
    for (name, source) in [
        ("float-fixture/source", true),
        ("float-fixture/sink", false),
    ] {
        let kind = fixture(name, source, flow);
        startup
            .insert(conduit_plot::KindSignature {
                kind: name.into(),
                startup_parameters: vec![],
            })
            .unwrap();
        profiles.insert_kind(kind.clone()).unwrap();
        offers.push(fixture_offer(kind));
    }
    let plot=conduit_plot::parse_with_startup(&format!("plot float-proof {{\n input: float-fixture/source\n operation: {}\n sink: float-fixture/sink\n input.value >> operation.value\n operation.result >> sink.value\n}}",float_integer_identity(flow)),&startup,&profiles).unwrap();
    let hosts = [HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "float-proof".into(),
        boot_id: "boot".into(),
        offer_generation: OfferGeneration(1),
        profile: "float-proof@1".into(),
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
        &["conduit.base/local@1".into()],
        1,
        bound(),
    )
    .unwrap()
}
enum Driver {
    Source(Vec<ValueRef>, usize, bool),
    Operation(Box<dyn StepBack<PORTS>>, Rc<Cell<u64>>, bool),
    Sink(Rc<Cell<bool>>, Rc<RefCell<Vec<Vec<u8>>>>, Option<Vec<u8>>),
}
impl StepBack<PORTS> for Driver {
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
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
            Self::Operation(back, _, staged) => {
                let out = back.step(io, bytes);
                *staged = matches!(out, StepOutcome::Progress);
                out
            }
            Self::Sink(blocked, _, staged) => {
                if blocked.get() {
                    return StepOutcome::Await;
                }
                if io.input_closed(KPort(0)) {
                    return StepOutcome::Complete;
                }
                let Some(input) = bytes.input(KPort(0)) else {
                    return StepOutcome::Await;
                };
                *staged = Some(input.to_vec());
                io.consume(KPort(0)).unwrap();
                StepOutcome::Progress
            }
        }
    }
    fn prepared_output(&self, port: KPort) -> Option<&[u8]> {
        if let Self::Operation(back, _, _) = self {
            back.prepared_output(port)
        } else {
            None
        }
    }
    fn step_committed(&mut self) {
        match self {
            Self::Source(_, cursor, staged) if *staged => {
                *cursor += 1;
                *staged = false;
            }
            Self::Operation(back, count, staged) if *staged => {
                back.step_committed();
                count.set(count.get() + 1);
                *staged = false;
            }
            Self::Sink(_, seen, staged) => {
                if let Some(value) = staged.take() {
                    seen.borrow_mut().push(value);
                }
            }
            _ => {}
        }
    }
    fn cancel(&mut self) {
        if let Self::Operation(back, _, staged) = self {
            *staged = false;
            back.cancel();
        }
    }
}
type Scheduler = FixedScheduler<Driver, HostedValueStore, HostedSignLog, 3, 2, PORTS, 2, 3, 2>;
fn scheduler(
    input: &[Vec<u8>],
    capacity: usize,
    blocked: Rc<Cell<bool>>,
    seen: Rc<RefCell<Vec<Vec<u8>>>>,
    count: Rc<Cell<u64>>,
) -> Scheduler {
    let plan = plan(true);
    let fragment = &plan.fragments[0];
    let (lowered, _) = conduit_plan_lowering::lowering::lower_plan_fragment_from_plan(
        &plan,
        &fragment.fragment_id,
    )
    .unwrap();
    let mut values = HostedValueStore::new(capacity as u16, 4096, capacity as u32 * 4096).unwrap();
    let inputs: Vec<_> = input.iter().map(|x| values.store(x).unwrap()).collect();
    let owners = conduit_std_host::fixed_numeric::FixedNumericOperationFactory::for_plan(
        &plan,
        &BTreeMap::new(),
    )
    .unwrap();
    assert!(owners.is_empty());
    let owner =
        conduit_std_host::fixed_numeric_float_integer::FloatIntegerOperationFactory::for_plan(
            &plan,
        )
        .unwrap();
    let drivers: Vec<_> = fragment
        .placements
        .iter()
        .map(|gear| match gear.kind_id.as_str() {
            "float-fixture/source" => Driver::Source(inputs.clone(), 0, false),
            "float-fixture/sink" => Driver::Sink(blocked.clone(), seen.clone(), None),
            _ => {
                assert_eq!(owner.budget(gear).unwrap().host_requests, 0);
                let mut wrong = gear.clone();
                wrong.kind_contract_revision = "foreign".into();
                assert!(owner.prepare(&wrong, &mut values).is_err());
                Driver::Operation(
                    owner.prepare(gear, &mut values).unwrap(),
                    count.clone(),
                    false,
                )
            }
        })
        .collect();
    drop(owner);
    let mut routes = FixedRoutes::<3, 2>::new(1);
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
        drivers.try_into().unwrap_or_else(|_| panic!("capacity")),
        values,
        HostedSignLog::new(256, 256 * core::mem::size_of::<KernelEvent>() as u32).unwrap(),
    )
    .unwrap()
}
fn encoded(values: [f32; 160]) -> Vec<u8> {
    let ty = conduit_ai::fixed_numeric_catalog::fixed_numeric_type("NumericF32Vector160").unwrap();
    let mut codec =
        conduit_ai::fixed_numeric_codec::FixedF32VectorCodec::<160>::prepare(&ty).unwrap();
    codec.encode(&values).unwrap();
    codec.encoded().to_vec()
}
#[test]
fn explicit_rounding_ties_extrema_and_nonfinite_refusal_are_atomic() {
    let cases = [
        (-2.5, -3),
        (-1.5, -2),
        (-0.5, -1),
        (-0., 0),
        (0.5, 1),
        (1.5, 2),
        (2.5, 3),
        (32767.49, 32767),
        (-32768.49, -32768),
    ];
    for (value, expected) in cases {
        let mut out = [7; 160];
        round_f32_to_i16_160(&[value; 160], &mut out).unwrap();
        assert_eq!(out, [expected; 160]);
    }
    for bad in [
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        32767.5,
        -32768.5,
        f32::MAX,
    ] {
        let mut input = [0.; 160];
        input[159] = bad;
        let mut out = [7; 160];
        assert!(round_f32_to_i16_160(&input, &mut out).is_err());
        assert_eq!(out, [7; 160]);
    }
}
#[test]
fn selected_closing_owner_repeats_frames_with_exact_integer_outputs() {
    let inputs = vec![
        encoded([-0.5; 160]),
        encoded([1.5; 160]),
        encoded([32767.; 160]),
    ];
    let seen = Rc::new(RefCell::new(vec![]));
    let count = Rc::new(Cell::new(0));
    let mut run = scheduler(
        &inputs,
        16,
        Rc::new(Cell::new(false)),
        seen.clone(),
        count.clone(),
    );
    for _ in 0..80 {
        run.step().unwrap();
    }
    assert_eq!(count.get(), 3);
    let ty = conduit_ai::fixed_numeric_catalog::fixed_numeric_type("NumericI16Vector160").unwrap();
    let codec =
        conduit_ai::fixed_numeric_i16_codec::FixedI16VectorCodec::<160>::prepare(&ty).unwrap();
    for (bytes, expected) in seen.borrow().iter().zip([-1, 2, 32767]) {
        let mut out = [0; 160];
        codec.decode(bytes, &mut out).unwrap();
        assert_eq!(out, [expected; 160]);
    }
    assert_eq!(seen.borrow().len(), 3);
}
#[test]
fn refusal_pressure_storage_failure_and_cancel_publish_no_partial_frame() {
    let mut nonfinite = encoded([0.; 160]);
    let end = nonfinite.len();
    nonfinite[end - 4..].copy_from_slice(&f32::NAN.to_le_bytes());
    for bad in [nonfinite, encoded([32767.5; 160]), encoded([-32768.5; 160])] {
        let seen = Rc::new(RefCell::new(vec![]));
        let count = Rc::new(Cell::new(0));
        let mut run = scheduler(
            &[bad],
            16,
            Rc::new(Cell::new(false)),
            seen.clone(),
            count.clone(),
        );
        assert!((0..24).any(|_| run.step().is_err()));
        assert_eq!(count.get(), 0);
        assert!(seen.borrow().is_empty());
    }
    let inputs = vec![encoded([0.; 160]); 3];
    let seen = Rc::new(RefCell::new(vec![]));
    let count = Rc::new(Cell::new(0));
    let mut run = scheduler(
        &inputs,
        16,
        Rc::new(Cell::new(true)),
        seen.clone(),
        count.clone(),
    );
    for _ in 0..24 {
        run.step().unwrap();
    }
    assert_eq!(count.get(), 1);
    assert!(seen.borrow().is_empty());
    run.cancel().unwrap();
    run.step().unwrap();
    assert_eq!(count.get(), 1);
    assert!(seen.borrow().is_empty());
    let seen = Rc::new(RefCell::new(vec![]));
    let count = Rc::new(Cell::new(0));
    let mut run = scheduler(
        &inputs,
        3,
        Rc::new(Cell::new(false)),
        seen.clone(),
        count.clone(),
    );
    assert!((0..24).any(|_| run.step().is_err()));
    assert_eq!(count.get(), 0);
    assert!(seen.borrow().is_empty());
    run.cancel().unwrap();
    run.step().unwrap();
    assert_eq!(count.get(), 0);
}
#[test]
fn admitted_conversion_step_allocates_nothing_and_refuses_before_consumption() {
    let plan = plan(true);
    let gear = plan.fragments[0]
        .placements
        .iter()
        .find(|g| g.implementation_id.as_str() == FLOAT_INTEGER_IMPLEMENTATION)
        .unwrap();
    let mut back = FixedFloatIntegerBack::prepare_planned::<4>(gear, 2, true).unwrap();
    for (value, valid) in [(0.5, true), (32767.5, false), (-1.5, true)] {
        let bytes = encoded([value; 160]);
        let reference = ValueRef {
            slot: 0,
            generation: 1,
            byte_len: bytes.len() as u32,
        };
        let mut io = StepIo::test_frame(
            [Some(reference), None, None, None],
            [false; 4],
            [Some(4096), None, None, None],
            None,
            2,
        );
        let inputs = StepInputBytes::test_frame([Some(bytes.as_slice()), None, None, None], None);
        let before = back.committed_frames();
        let (out, observed) = allocation_probe::observe(|| back.step(&mut io, &inputs));
        assert_eq!(observed.allocations, 0);
        assert_eq!(observed.reallocations, 0);
        if valid {
            assert_eq!(out, StepOutcome::Progress);
            assert_eq!(back.committed_frames(), before);
            <FixedFloatIntegerBack as StepBack<4>>::step_committed(&mut back);
            assert_eq!(back.committed_frames(), before + 1);
        } else {
            assert!(matches!(out, StepOutcome::Fail(_)));
            assert!(!io.test_consumed(KPort(0)));
            assert!(
                <FixedFloatIntegerBack as StepBack<4>>::prepared_output(&back, KPort(0)).is_none()
            );
            assert_eq!(back.committed_frames(), before);
        }
    }
}

#[test]
fn explicit_value_owner_completes_after_one_committed_frame() {
    let selected = plan(false);
    let gear = selected.fragments[0]
        .placements
        .iter()
        .find(|g| g.implementation_id.as_str() == FLOAT_INTEGER_IMPLEMENTATION)
        .unwrap();
    assert!(FixedFloatIntegerBack::prepare_planned::<4>(gear, 2, true).is_err());
    let mut back = FixedFloatIntegerBack::prepare_planned::<4>(gear, 2, false).unwrap();
    let bytes = encoded([0.5; 160]);
    let reference = ValueRef {
        slot: 0,
        generation: 1,
        byte_len: bytes.len() as u32,
    };
    let mut io = StepIo::test_frame(
        [Some(reference), None, None, None],
        [false; 4],
        [Some(4096), None, None, None],
        None,
        2,
    );
    let inputs = StepInputBytes::test_frame([Some(bytes.as_slice()), None, None, None], None);
    assert_eq!(back.step(&mut io, &inputs), StepOutcome::Progress);
    assert_eq!(back.committed_frames(), 0);
    <FixedFloatIntegerBack as StepBack<4>>::step_committed(&mut back);
    assert_eq!(back.committed_frames(), 1);
    assert_eq!(back.step(&mut io, &inputs), StepOutcome::Complete);
    assert_eq!(back.committed_frames(), 1);
}
