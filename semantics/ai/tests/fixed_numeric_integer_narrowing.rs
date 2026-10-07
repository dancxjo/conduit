#![cfg(feature = "kernel-step")]
use conduit_ai::{fixed_numeric_catalog::*, fixed_numeric_integer_narrowing::*};
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
const PORTS: usize = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
fn fixture(name: &str, source: bool, flow: bool) -> Kind {
    let (identity, size) = if source {
        ("value/u64", 8)
    } else {
        ("value/u16", 2)
    };
    let port = PortDescriptor {
        port_id: port_id("value"),
        value_kind: kind_id(identity),
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
            contract: CheckedValueContract::new(port.value_kind, size, vec![]).unwrap(),
        }])],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: 8,
        },
    }
}
fn offer(kind: Kind) -> CapabilityOffer {
    let id = String::from(kind.kind_id.as_str());
    BackOfferBuilder::new(
        kind,
        Back {
            capability_id: CapabilityId::from(id.clone()),
            execution_profile_id: ExecutionProfileId::from("integer-fixture@1"),
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
    install_fixed_numeric_catalogs(&mut startup, &mut profiles).unwrap();
    if flow {
        install_checked_integer_flow_catalogs(&mut startup, &mut profiles).unwrap();
    }
    let mut offers = vec![];
    for (name, source) in [
        ("integer-fixture/source", true),
        ("integer-fixture/sink", false),
    ] {
        let kind = fixture(name, source, flow);
        startup
            .insert(conduit_plot::KindSignature {
                kind: name.into(),
                startup_parameters: vec![],
            })
            .unwrap();
        profiles.insert_kind(kind.clone()).unwrap();
        offers.push(offer(kind));
    }
    let numeric = checked_integer_narrowing_offer(flow).unwrap();
    let identity = numeric.kind_id.as_str().to_owned();
    offers.push(numeric);
    let plot=conduit_plot::parse_with_startup(&format!("plot integer-proof {{\n input: integer-fixture/source\n operation: {identity}\n sink: integer-fixture/sink\n input.value >> operation.value\n operation.result >> sink.value\n}}"),&startup,&profiles).unwrap();
    let hosts = [HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "integer-proof".into(),
        boot_id: "boot".into(),
        offer_generation: OfferGeneration(1),
        profile: "integer-proof@1".into(),
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
        8,
    )
    .unwrap()
}
enum Driver {
    Source(Vec<ValueRef>, usize, bool),
    Operation(Box<dyn StepBack<PORTS>>, Rc<Cell<u64>>, bool),
    Sink(Rc<Cell<bool>>, Rc<RefCell<Vec<u16>>>, Option<u16>),
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
                *staged = Some(u16::from_le_bytes(input.try_into().unwrap()));
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
    input: &[u64],
    capacity: usize,
    blocked: Rc<Cell<bool>>,
    seen: Rc<RefCell<Vec<u16>>>,
    count: Rc<Cell<u64>>,
) -> Scheduler {
    let plan = plan(true);
    let fragment = &plan.fragments[0];
    let (lowered, _) = conduit_plan_lowering::lowering::lower_plan_fragment_from_plan(
        &plan,
        &fragment.fragment_id,
    )
    .unwrap();
    let mut values = HostedValueStore::new(capacity as u16, 8, capacity as u32 * 8).unwrap();
    let inputs: Vec<_> = input
        .iter()
        .map(|x| values.store(&x.to_le_bytes()).unwrap())
        .collect();
    let owners = conduit_std_host::fixed_numeric::FixedNumericOperationFactory::for_plan(
        &plan,
        &BTreeMap::new(),
    )
    .unwrap();
    let drivers: Vec<_> = fragment
        .placements
        .iter()
        .map(|gear| match gear.kind_id.as_str() {
            "integer-fixture/source" => Driver::Source(inputs.clone(), 0, false),
            "integer-fixture/sink" => Driver::Sink(blocked.clone(), seen.clone(), None),
            _ => {
                let owner = owners
                    .iter()
                    .find(|owner| owner.implementation_id() == &gear.implementation_id)
                    .unwrap();
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
    drop(owners);
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
#[test]
fn ordinary_closing_flow_preserves_integer_boundaries_and_closes_without_surplus() {
    let seen = Rc::new(RefCell::new(vec![]));
    let count = Rc::new(Cell::new(0));
    let mut scheduler = scheduler(
        &[0, 32, 65535],
        16,
        Rc::new(Cell::new(false)),
        seen.clone(),
        count.clone(),
    );
    for _ in 0..80 {
        scheduler.step().unwrap();
    }
    assert_eq!(*seen.borrow(), [0, 32, 65535]);
    assert_eq!(count.get(), 3);
    let flow_plan = plan(true);
    let gear = flow_plan.fragments[0]
        .placements
        .iter()
        .find(|g| g.kind_id.as_str().starts_with("numeric/"))
        .unwrap();
    assert!(CheckedU64ToU16Back::prepare_planned::<PORTS>(gear, 2, false).is_err());
    assert!(CheckedU64ToU16Back::prepare_planned::<PORTS>(gear, 1, true).is_err());
    let value_plan = plan(false);
    let gear = value_plan.fragments[0]
        .placements
        .iter()
        .find(|g| g.kind_id.as_str().starts_with("numeric/"))
        .unwrap();
    assert!(CheckedU64ToU16Back::prepare_planned::<PORTS>(gear, 2, false).is_ok());
}
#[test]
fn overflowing_input_refuses_before_numeric_consumption_or_output() {
    let seen = Rc::new(RefCell::new(vec![]));
    let count = Rc::new(Cell::new(0));
    let mut scheduler = scheduler(
        &[65536, u64::MAX],
        16,
        Rc::new(Cell::new(false)),
        seen.clone(),
        count.clone(),
    );
    let mut refused = false;
    for _ in 0..24 {
        if scheduler.step().is_err() {
            refused = true;
            break;
        }
    }
    assert!(refused);
    assert_eq!(count.get(), 0);
    assert!(seen.borrow().is_empty());
}
#[test]
fn queue_storage_pressure_and_cancellation_preserve_last_committed_conversion() {
    let blocked = Rc::new(Cell::new(true));
    let seen = Rc::new(RefCell::new(vec![]));
    let count = Rc::new(Cell::new(0));
    let mut run = scheduler(&[32, 80, 255], 16, blocked, seen.clone(), count.clone());
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
        &[32, 80, 255],
        3,
        Rc::new(Cell::new(false)),
        seen.clone(),
        count.clone(),
    );
    let mut refused = false;
    for _ in 0..24 {
        if run.step().is_err() {
            refused = true;
            break;
        }
    }
    assert!(refused);
    assert_eq!(count.get(), 0);
    assert!(seen.borrow().is_empty());
    run.cancel().unwrap();
    run.step().unwrap();
    assert_eq!(count.get(), 0);
}

#[test]
fn exact_primitive_extent_and_one_shot_value_commit_are_preserved() {
    let plan = plan(false);
    let gear = plan.fragments[0]
        .placements
        .iter()
        .find(|gear| gear.kind_id.as_str() == "numeric/u64-to-u16")
        .unwrap();
    let mut back = CheckedU64ToU16Back::prepare_planned::<4>(gear, 2, false).unwrap();
    for bytes in [
        &[0u8; 7][..],
        &65536u64.to_le_bytes()[..],
        &u64::MAX.to_le_bytes()[..],
    ] {
        let reference = ValueRef {
            slot: 0,
            generation: 1,
            byte_len: bytes.len() as u32,
        };
        let mut io = StepIo::test_frame(
            [Some(reference), None, None, None],
            [false; 4],
            [Some(2), None, None, None],
            None,
            2,
        );
        let inputs = StepInputBytes::test_frame([Some(bytes), None, None, None], None);
        assert!(matches!(back.step(&mut io, &inputs), StepOutcome::Fail(_)));
        assert!(!io.test_consumed(KPort(0)));
        assert!(<CheckedU64ToU16Back as StepBack<4>>::prepared_output(&back, KPort(0)).is_none());
        assert_eq!(back.committed_frames(), 0);
    }
    let bytes = 65535u64.to_le_bytes();
    let reference = ValueRef {
        slot: 0,
        generation: 1,
        byte_len: 8,
    };
    let mut io = StepIo::test_frame(
        [Some(reference), None, None, None],
        [false; 4],
        [Some(2), None, None, None],
        None,
        2,
    );
    let inputs = StepInputBytes::test_frame([Some(&bytes), None, None, None], None);
    assert_eq!(back.step(&mut io, &inputs), StepOutcome::Progress);
    assert_eq!(back.committed_frames(), 0);
    <CheckedU64ToU16Back as StepBack<4>>::step_committed(&mut back);
    assert_eq!(back.committed_frames(), 1);
    let mut next = StepIo::test_frame(
        [Some(reference), None, None, None],
        [false; 4],
        [Some(2), None, None, None],
        None,
        2,
    );
    assert_eq!(back.step(&mut next, &inputs), StepOutcome::Complete);
    assert!(!next.test_consumed(KPort(0)));
    assert_eq!(back.committed_frames(), 1);
}
