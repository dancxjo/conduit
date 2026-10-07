#![cfg(feature = "kernel-operation-owners")]
use conduit_ai::native_profile::*;
use conduit_composite::KernelOperationFactory;
use conduit_core::*;
use conduit_kernel::{
    scheduler::{FixedScheduler, StepBack, StepInputBytes, StepIo, StepOutcome},
    FixedRoutes, HostedSignLog, HostedValueStore, KernelEvent, PortId as KPort, ValueRef,
    ValueStorage,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
const PORTS: usize = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const DEFINITION: &str = "type Allowed = U16 in 32..=255 where . % 2 == 0\ntype Finite = F32 finite\ntype Nested = {\n period: Allowed\n values: collection Finite = 2\n}\ntype Choice =\n idle\n | active Allowed\ntype Payload = {\n nested: Nested\n choice: Choice\n limit: U16\n where .nested.period + 0 <= .limit\n}\n";
fn fixture(name: &str, source: bool, profile: &PreparedNativeProfile, flow: bool) -> Kind {
    let ty = if source {
        profile.candidate_type()
    } else {
        profile.value_type()
    };
    let identity = ty.profile().unwrap().value_kind().clone();
    let bound = conduit_plot::maximum_prepared_transport_value_bytes(ty).unwrap();
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
fn plan(profile: &PreparedNativeProfile, flow: bool) -> Plan {
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
    let plot=conduit_plot::parse_with_startup(&format!("plot profile-proof {{\n input: profile-fixture/source\n operation: {}\n sink: profile-fixture/sink\n input.value >> operation.candidate\n operation.result >> sink.value\n}}",profile.kind_identity(flow)),&startup,&profiles).unwrap();
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
    input: &[Vec<u8>],
    capacity: usize,
    blocked: Rc<Cell<bool>>,
    seen: Rc<RefCell<Vec<Vec<u8>>>>,
    count: Rc<Cell<u64>>,
) -> Scheduler {
    let profile = std::sync::Arc::new(
        PreparedNativeProfile::check_definition(DEFINITION, "Payload").unwrap(),
    );
    let plan = plan(&profile, true);
    let fragment = &plan.fragments[0];
    let (lowered, _) = conduit_plan_lowering::lowering::lower_plan_fragment_from_plan(
        &plan,
        &fragment.fragment_id,
    )
    .unwrap();
    let mut values = HostedValueStore::new(capacity as u16, 4096, capacity as u32 * 4096).unwrap();
    let inputs: Vec<_> = input.iter().map(|x| values.store(x).unwrap()).collect();
    let owner =
        conduit_ai::operation_owners::native_profile::NativeProfileOperationFactory::for_plan(
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
fn field(ty: &StructuredInfoType, name: &str) -> StructuredInfoType {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|f| f.name() == name)
        .unwrap()
        .value_type()
        .clone()
}
fn record(ty: StructuredInfoType, items: Vec<(&str, StructuredInfoValue)>) -> StructuredInfoValue {
    StructuredInfoValue::record(
        ty,
        items
            .into_iter()
            .map(|(name, value)| StructuredFieldValue::new(name, value).unwrap())
            .collect(),
    )
    .unwrap()
}
fn candidate(profile: &PreparedNativeProfile, period: u16, limit: u16, active: bool) -> Vec<u8> {
    let ty = profile.candidate_type();
    let nested = field(ty, "nested");
    let values = field(&nested, "values");
    let StructuredInfoTypeShape::Collection { element, .. } = values.shape() else {
        panic!("collection")
    };
    let floats = StructuredInfoValue::collection(
        values.clone(),
        [0.25f32, -0.0]
            .into_iter()
            .map(|v| StructuredInfoValue::leaf(element.clone(), v.to_le_bytes().to_vec()).unwrap())
            .collect(),
    )
    .unwrap();
    let nested_value = record(
        nested.clone(),
        vec![
            (
                "period",
                StructuredInfoValue::leaf(field(&nested, "period"), period.to_le_bytes().to_vec())
                    .unwrap(),
            ),
            ("values", floats),
        ],
    );
    let choice = field(ty, "choice");
    let StructuredInfoTypeShape::Variant { cases, .. } = choice.shape() else {
        panic!("variant")
    };
    let tag = if active { "active" } else { "idle" };
    let payload_type = cases
        .iter()
        .find(|c| c.tag() == tag)
        .unwrap()
        .payload_type()
        .clone();
    let payload = StructuredInfoValue::leaf(
        payload_type,
        if active {
            period.to_le_bytes().to_vec()
        } else {
            vec![]
        },
    )
    .unwrap();
    let choice_value = StructuredInfoValue::variant(choice.clone(), tag, payload).unwrap();
    record(
        ty.clone(),
        vec![
            ("nested", nested_value),
            (
                "limit",
                StructuredInfoValue::leaf(field(ty, "limit"), limit.to_le_bytes().to_vec())
                    .unwrap(),
            ),
            ("choice", choice_value),
        ],
    )
    .canonical_bytes()
    .unwrap()
}
#[test]
fn ordinary_native_flow_reframes_distinct_candidate_and_checks_nested_laws() {
    let profile = PreparedNativeProfile::check_definition(DEFINITION, "Payload").unwrap();
    assert_ne!(profile.candidate_type(), profile.value_type());
    let inputs = [
        candidate(&profile, 32, 80, false),
        candidate(&profile, 80, 80, true),
        candidate(&profile, 254, 255, true),
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
    assert_eq!(seen.borrow().len(), 3);
    for output in seen.borrow().iter() {
        let value = StructuredInfoValue::from_canonical_bytes(output).unwrap();
        assert_eq!(value.value_type(), profile.value_type());
    }
}
#[test]
fn source_leaf_nested_boolean_and_cross_field_laws_refuse_before_consumption_without_allocation() {
    let profile = PreparedNativeProfile::check_definition(DEFINITION, "Payload").unwrap();
    let plan = plan(&profile, true);
    let gear = plan.fragments[0]
        .placements
        .iter()
        .find(|g| g.implementation_id.as_str() == NATIVE_PROFILE_IMPLEMENTATION)
        .unwrap();
    let mut back = NativeProfileBack::prepare_planned::<4>(gear, 2, &profile, true).unwrap();
    let good = candidate(&profile, 80, 80, true);
    let mut malformed = good.clone();
    malformed.pop();
    let mut nonfinite = good.clone();
    let position = nonfinite
        .windows(4)
        .position(|b| b == 0.25f32.to_le_bytes())
        .unwrap();
    nonfinite[position..position + 4].copy_from_slice(&f32::NAN.to_le_bytes());
    let cases = [
        (candidate(&profile, 31, 80, false), false),
        (candidate(&profile, 33, 80, false), false),
        (candidate(&profile, 80, 79, true), false),
        (candidate(&profile, 256, 300, true), false),
        (nonfinite, false),
        (malformed, false),
        (good, true),
        (candidate(&profile, 254, 255, false), true),
    ];
    for (bytes, valid) in cases {
        let before = back.committed_frames();
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
        let (out, observed) = allocation_probe::observe(|| back.step(&mut io, &inputs));
        assert_eq!(observed.allocations, 0);
        assert_eq!(observed.reallocations, 0);
        if valid {
            assert_eq!(out, StepOutcome::Progress);
            assert_eq!(back.committed_frames(), before);
            let output =
                <NativeProfileBack as StepBack<4>>::prepared_output(&back, KPort(0)).unwrap();
            assert_eq!(
                StructuredInfoValue::from_canonical_bytes(output)
                    .unwrap()
                    .value_type(),
                profile.value_type()
            );
            <NativeProfileBack as StepBack<4>>::step_committed(&mut back);
            assert_eq!(back.committed_frames(), before + 1);
        } else {
            assert!(matches!(out, StepOutcome::Fail(_)));
            assert!(!io.test_consumed(KPort(0)));
            assert!(<NativeProfileBack as StepBack<4>>::prepared_output(&back, KPort(0)).is_none());
            assert_eq!(back.committed_frames(), before);
        }
    }
}
#[test]
fn exact_source_and_temporal_profile_selection_refuse_foreign_receipts() {
    let profile = PreparedNativeProfile::check_definition(DEFINITION, "Payload").unwrap();
    let plan = plan(&profile, false);
    let gear = plan.fragments[0]
        .placements
        .iter()
        .find(|g| g.implementation_id.as_str() == NATIVE_PROFILE_IMPLEMENTATION)
        .unwrap();
    let other = PreparedNativeProfile::check_definition(
        &DEFINITION.replace("32..=255", "0..=65535"),
        "Payload",
    )
    .unwrap();
    assert!(NativeProfileBack::prepare_planned::<4>(gear, 2, &other, false).is_err());
    assert!(NativeProfileBack::prepare_planned::<4>(gear, 2, &profile, true).is_err());
    assert!(PreparedNativeProfile::check_definition("type Bad = U16\n", "Bad").is_err());
    assert!(PreparedNativeProfile::check_definition(DEFINITION, "Absent").is_err());
}
#[test]
fn pressure_storage_rollback_and_cancellation_preserve_native_progress() {
    let profile = PreparedNativeProfile::check_definition(DEFINITION, "Payload").unwrap();
    let input = [
        candidate(&profile, 32, 80, false),
        candidate(&profile, 80, 80, true),
        candidate(&profile, 254, 255, true),
    ];
    let seen = Rc::new(RefCell::new(vec![]));
    let count = Rc::new(Cell::new(0));
    let mut run = scheduler(
        &input,
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
    let mut run = scheduler(
        &input,
        3,
        Rc::new(Cell::new(false)),
        seen.clone(),
        Rc::new(Cell::new(0)),
    );
    let mut refused = false;
    for _ in 0..24 {
        if run.step().is_err() {
            refused = true;
            break;
        }
    }
    assert!(refused);
    assert!(seen.borrow().is_empty());
}
#[test]
fn derived_candidate_construction_is_source_checked_without_native_law_claim() {
    let profile = PreparedNativeProfile::check_definition(DEFINITION, "Payload").unwrap();
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    profile.install(&mut startup, &mut profiles, true).unwrap();
    let source=format!("with {}/candidate as Candidate\nplot candidate (\n >> period: U16\n result: Candidate >>\n) = ({{nested: {{period: ., values: [0x3e800000, 0x80000000]}}, choice: idle(unit), limit: 80}})\n",profile.kind_identity(true));
    let syntax = conduit_plot::parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = conduit_plot::check_syntax_document(&syntax, &startup).unwrap();
    let expanded =
        conduit_plot::expand_canonical_plot_for_authoring(&checked, "candidate", &profiles)
            .unwrap();
    assert_eq!(expanded.expanded.gears.len(), 1);
}
