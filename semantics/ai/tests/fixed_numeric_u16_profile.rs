#![cfg(feature = "kernel-step")]
use conduit_ai::fixed_numeric_u16_profile::*;
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
const DEFINITION: &str = "type Allowed = U16 in 32..=255 where . % 2 == 0\n";
fn fixture(name: &str, source: bool, profile: &PreparedU16Profile, flow: bool) -> Kind {
    let view = profile.value_type().profile().unwrap();
    let (identity, bound) = if source {
        (kind_id("value/u16"), 2)
    } else {
        (
            view.value_kind().clone(),
            conduit_plot::maximum_prepared_transport_value_bytes(profile.value_type()).unwrap(),
        )
    };
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
            contract: CheckedValueContract::new(port.value_kind, bound, vec![]).unwrap(),
        }])],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: conduit_plot::maximum_prepared_transport_value_bytes(
                profile.value_type(),
            )
            .unwrap(),
        },
    }
}
fn fixture_offer(kind: Kind) -> CapabilityOffer {
    let id = String::from(kind.kind_id.as_str());
    BackOfferBuilder::new(
        kind,
        Back {
            capability_id: CapabilityId::from(id.clone()),
            execution_profile_id: ExecutionProfileId::from("profile-fixture@1"),
            implementation_id: ImplementationId::from(id.clone()),
            artifact_id: ArtifactId::from(id),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}
fn plan(profile: &PreparedU16Profile, flow: bool) -> Plan {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    profile.install(&mut startup, &mut profiles, flow).unwrap();
    let mut offers = vec![profile.offer(flow).unwrap()];
    for (name, source) in [
        ("profile-fixture/source", true),
        ("profile-fixture/sink", false),
    ] {
        let kind = fixture(name, source, profile, flow);
        startup
            .insert(conduit_plot::KindSignature {
                kind: name.into(),
                startup_parameters: vec![],
            })
            .unwrap();
        profiles.insert_kind(kind.clone()).unwrap();
        offers.push(fixture_offer(kind));
    }
    let plot=conduit_plot::parse_with_startup(&format!("plot profile-proof {{\n input: profile-fixture/source\n operation: {}\n sink: profile-fixture/sink\n input.value >> operation.value\n operation.result >> sink.value\n}}",profile.kind_identity(flow)),&startup,&profiles).unwrap();
    let hosts = [HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "profile-proof".into(),
        boot_id: "boot".into(),
        offer_generation: OfferGeneration(1),
        profile: "profile-proof@1".into(),
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
        conduit_plot::maximum_prepared_transport_value_bytes(profile.value_type()).unwrap(),
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
    input: &[u16],
    capacity: usize,
    blocked: Rc<Cell<bool>>,
    seen: Rc<RefCell<Vec<Vec<u8>>>>,
    count: Rc<Cell<u64>>,
) -> Scheduler {
    let profile = std::sync::Arc::new(PreparedU16Profile::check_definition(DEFINITION).unwrap());
    let plan = plan(&profile, true);
    let fragment = &plan.fragments[0];
    let (lowered, _) = conduit_plan_lowering::lowering::lower_plan_fragment_from_plan(
        &plan,
        &fragment.fragment_id,
    )
    .unwrap();
    let mut values = HostedValueStore::new(capacity as u16, 4096, capacity as u32 * 4096).unwrap();
    let inputs: Vec<_> = input
        .iter()
        .map(|x| values.store(&x.to_le_bytes()).unwrap())
        .collect();
    let owners = conduit_std_host::fixed_numeric::FixedNumericOperationFactory::for_plan(
        &plan,
        &BTreeMap::new(),
    )
    .unwrap();
    assert!(owners.is_empty());
    let owner = conduit_std_host::fixed_numeric_u16_profile::U16ProfileOperationFactory::for_plan(
        &plan,
        &[profile],
    )
    .unwrap();
    let drivers: Vec<_> = fragment
        .placements
        .iter()
        .map(|gear| match gear.kind_id.as_str() {
            "profile-fixture/source" => Driver::Source(inputs.clone(), 0, false),
            "profile-fixture/sink" => Driver::Sink(blocked.clone(), seen.clone(), None),
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

#[test]
fn ordinary_profile_flow_validates_source_range_and_boolean_law() {
    let seen = Rc::new(RefCell::new(vec![]));
    let count = Rc::new(Cell::new(0));
    let mut run = scheduler(
        &[32, 80, 254],
        16,
        Rc::new(Cell::new(false)),
        seen.clone(),
        count.clone(),
    );
    for _ in 0..80 {
        run.step().unwrap();
    }
    assert_eq!(count.get(), 3);
    let profile = PreparedU16Profile::check_definition(DEFINITION).unwrap();
    let actual: Vec<_> = seen
        .borrow()
        .iter()
        .map(|bytes| {
            let value = StructuredInfoValue::from_canonical_bytes(bytes).unwrap();
            assert_eq!(value.value_type(), profile.value_type());
            let StructuredInfoValueShape::Leaf(raw) = value.shape() else {
                panic!("scalar")
            };
            u16::from_le_bytes(raw.try_into().unwrap())
        })
        .collect();
    assert_eq!(actual, [32, 80, 254]);
    for value in [0, 31, 33, 255, 256, u16::MAX] {
        let seen = Rc::new(RefCell::new(vec![]));
        let count = Rc::new(Cell::new(0));
        let mut run = scheduler(
            &[value],
            16,
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
        assert!(refused, "{value}");
        assert_eq!(count.get(), 0);
        assert!(seen.borrow().is_empty());
    }
}
#[test]
fn pressure_storage_rollback_and_cancellation_preserve_profile_progress() {
    let seen = Rc::new(RefCell::new(vec![]));
    let count = Rc::new(Cell::new(0));
    let mut run = scheduler(
        &[32, 80, 254],
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
        &[32, 80, 254],
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
fn exact_profile_signature_and_source_definition_cannot_be_substituted() {
    let profile = PreparedU16Profile::check_definition(DEFINITION).unwrap();
    let plan = plan(&profile, false);
    let gear = plan.fragments[0]
        .placements
        .iter()
        .find(|gear| gear.implementation_id.as_str() == U16_PROFILE_IMPLEMENTATION)
        .unwrap();
    let other = PreparedU16Profile::check_definition("type Allowed = U16 in 0..=65535\n").unwrap();
    assert!(U16ProfileBack::prepare_planned::<4>(gear, 2, &other, false).is_err());
    assert!(U16ProfileBack::prepare_planned::<4>(gear, 2, &profile, true).is_err());
    for source in [
        "type Wrong = U32\n",
        "type Wrong = F32\n",
        "type One = U16\ntype Two = U16\n",
        "type Wrong = U16\nplot untrusted (>> value: U16 result: U16 >>) = (.)\n",
    ] {
        assert!(PreparedU16Profile::check_definition(source).is_err());
    }
    let mut wrong = gear.clone();
    wrong.kind_id = kind_id("numeric/not-supported");
    wrong.implementation_id =
        conduit_ai::fixed_numeric_integer_narrowing::INTEGER_NARROWING_IMPLEMENTATION.into();
    let mut malformed = plan.clone();
    let selected = malformed.fragments[0]
        .placements
        .iter_mut()
        .find(|g| g.placement_id == gear.placement_id)
        .unwrap();
    *selected = wrong;
    assert!(
        conduit_std_host::fixed_numeric::FixedNumericOperationFactory::for_plan(
            &malformed,
            &BTreeMap::new()
        )
        .is_err()
    );
}
#[test]
fn prepared_profile_step_is_allocation_free_and_refuses_before_consumption() {
    let profile = PreparedU16Profile::check_definition(DEFINITION).unwrap();
    let plan = plan(&profile, true);
    let gear = plan.fragments[0]
        .placements
        .iter()
        .find(|gear| gear.implementation_id.as_str() == U16_PROFILE_IMPLEMENTATION)
        .unwrap();
    let mut back = U16ProfileBack::prepare_planned::<4>(gear, 2, &profile, true).unwrap();
    for (value, valid) in [
        (31u16, false),
        (33, false),
        (80, true),
        (255, false),
        (254, true),
    ] {
        let bytes = value.to_le_bytes();
        let reference = ValueRef {
            slot: 0,
            generation: 1,
            byte_len: 2,
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
            <U16ProfileBack as StepBack<4>>::step_committed(&mut back);
            assert_eq!(back.committed_frames(), before + 1);
        } else {
            assert!(matches!(out, StepOutcome::Fail(_)));
            assert!(!io.test_consumed(KPort(0)));
            assert!(<U16ProfileBack as StepBack<4>>::prepared_output(&back, KPort(0)).is_none());
            assert_eq!(back.committed_frames(), before);
        }
    }
}

#[test]
fn authored_source_profile_fore_matches_exact_output_type_and_refuses_changed_law() {
    let profile = PreparedU16Profile::check_definition(DEFINITION).unwrap();
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    profile.install(&mut startup, &mut profiles, false).unwrap();
    let wrapper=format!("{DEFINITION}plot admit-profile (\n >> value: U16\n result: Allowed >>\n) {{\n admit: {}\n value >> admit.value\n admit.result >> result\n}}\n",profile.kind_identity(false));
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(&wrapper),
        &startup,
    )
    .unwrap();
    let expanded =
        conduit_plot::expand_canonical_plot_for_authoring(&checked, "admit-profile", &profiles)
            .unwrap();
    assert_eq!(expanded.expanded.gears.len(), 1);
    let wrong = wrapper.replace("in 32..=255 where . % 2 == 0", "in 0..=65535");
    let checked =
        conduit_plot::check_syntax_document(&conduit_plot::parse_syntax_document(&wrong), &startup)
            .unwrap();
    assert!(conduit_plot::expand_canonical_plot_for_authoring(
        &checked,
        "admit-profile",
        &profiles
    )
    .is_err());
}
