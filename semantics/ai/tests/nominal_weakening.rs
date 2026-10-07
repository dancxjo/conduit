#![cfg(feature = "kernel-operation-owners")]
use conduit_ai::nominal_weakening::*;
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
fn fixture(name: &str, source: bool, profile: &PreparedNominalWeakening, flow: bool) -> Kind {
    let ty = if source {
        profile.input_type()
    } else {
        profile.output_type()
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
                profile.input_type(),
            )
            .unwrap()
            .max(
                conduit_plot::maximum_prepared_transport_value_bytes(profile.output_type())
                    .unwrap(),
            ),
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
fn plan(profile: &PreparedNominalWeakening, flow: bool) -> Plan {
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
        conduit_plot::maximum_prepared_transport_value_bytes(profile.input_type())
            .unwrap()
            .max(
                conduit_plot::maximum_prepared_transport_value_bytes(profile.output_type())
                    .unwrap(),
            ),
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
    let profile = std::sync::Arc::new(profile());
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
        conduit_ai::operation_owners::nominal_weakening::NominalWeakeningOperationFactory::for_plan(
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

fn profile() -> PreparedNominalWeakening {
    let definition = conduit_plot::parse_syntax_document(DEFINITION);
    let checked =
        conduit_plot::check_syntax_document(&definition, &conduit_plot::StartupCatalog::new())
            .unwrap();
    PreparedNominalWeakening::prepare(
        checked
            .native_types
            .iter()
            .find(|t| t.name == "Payload")
            .unwrap()
            .value_type
            .clone(),
    )
    .unwrap()
}
fn input(profile: &PreparedNominalWeakening, period: u16) -> Vec<u8> {
    // Construct the same primitive body under a tiny bare schema, then apply the exact input prefix.
    // This is intentionally shape-only, including period=31 and NaN, to prove no native admission claim.
    let source=conduit_plot::parse_syntax_document("type Nested = {\n period: U16\n values: collection F32 = 2\n}\ntype Choice =\n idle\n | active U16\ntype Payload = {\n nested: Nested\n choice: Choice\n limit: U16\n}\n");
    let checked =
        conduit_plot::check_syntax_document(&source, &conduit_plot::StartupCatalog::new()).unwrap();
    let ty = &checked
        .native_types
        .iter()
        .find(|t| t.name == "Payload")
        .unwrap()
        .value_type;
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
    fn record(
        ty: StructuredInfoType,
        fields: Vec<(&str, StructuredInfoValue)>,
    ) -> StructuredInfoValue {
        StructuredInfoValue::record(
            ty,
            fields
                .into_iter()
                .map(|(n, v)| StructuredFieldValue::new(n, v).unwrap())
                .collect(),
        )
        .unwrap()
    }
    let nested = field(ty, "nested");
    let values = field(&nested, "values");
    let StructuredInfoTypeShape::Collection { element, .. } = values.shape() else {
        panic!("collection")
    };
    let vector = StructuredInfoValue::collection(
        values.clone(),
        [f32::NAN, -0.0]
            .into_iter()
            .map(|f| StructuredInfoValue::leaf(element.clone(), f.to_le_bytes().to_vec()).unwrap())
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
            ("values", vector),
        ],
    );
    let choice = field(ty, "choice");
    let StructuredInfoTypeShape::Variant { cases, .. } = choice.shape() else {
        panic!("variant")
    };
    let payload = cases
        .iter()
        .find(|c| c.tag() == "active")
        .unwrap()
        .payload_type();
    let value = record(
        ty.clone(),
        vec![
            ("nested", nested_value),
            (
                "choice",
                StructuredInfoValue::variant(
                    choice.clone(),
                    "active",
                    StructuredInfoValue::leaf(payload.clone(), period.to_le_bytes().to_vec())
                        .unwrap(),
                )
                .unwrap(),
            ),
            (
                "limit",
                StructuredInfoValue::leaf(field(ty, "limit"), 0u16.to_le_bytes().to_vec()).unwrap(),
            ),
        ],
    );
    let encoded = value.canonical_bytes().unwrap();
    let prefix = ty.canonical_bytes().unwrap();
    let mut bytes = profile.input_type().canonical_bytes().unwrap();
    bytes.extend_from_slice(encoded.strip_prefix(prefix.as_slice()).unwrap());
    bytes
}
#[test]
fn closing_flow_preserves_exact_body_and_repeats_without_admitting_laws() {
    let profile = profile();
    let inputs = [
        input(&profile, 31),
        input(&profile, 33),
        input(&profile, 256),
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
    for (input, output) in inputs.iter().zip(seen.borrow().iter()) {
        assert_eq!(
            input
                .strip_prefix(profile.input_type().canonical_bytes().unwrap().as_slice())
                .unwrap(),
            output
                .strip_prefix(profile.output_type().canonical_bytes().unwrap().as_slice())
                .unwrap()
        );
    }
}
#[test]
fn weakening_has_no_step_allocations_and_refuses_foreign_or_malformed_framing() {
    let profile = profile();
    let plan = plan(&profile, true);
    let gear = plan.fragments[0]
        .placements
        .iter()
        .find(|g| g.implementation_id.as_str() == WEAKENING_IMPLEMENTATION)
        .unwrap();
    let mut back = NominalWeakeningBack::prepare_planned::<4>(gear, 2, &profile, true).unwrap();
    let good = input(&profile, 31);
    let mut bad = good.clone();
    bad.pop();
    let foreign = profile.output_type().canonical_bytes().unwrap();
    for (bytes, valid) in [
        (good.as_slice(), true),
        (bad.as_slice(), false),
        (foreign.as_slice(), false),
    ] {
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
        let inputs = StepInputBytes::test_frame([Some(bytes), None, None, None], None);
        let (out, observed) = allocation_probe::observe(|| back.step(&mut io, &inputs));
        assert_eq!(observed.allocations, 0);
        assert_eq!(observed.reallocations, 0);
        if valid {
            assert_eq!(out, StepOutcome::Progress);
            assert_eq!(back.committed_frames(), 0);
            <NominalWeakeningBack as StepBack<4>>::step_committed(&mut back);
        } else {
            assert!(matches!(out, StepOutcome::Fail(_)));
            assert!(!io.test_consumed(KPort(0)));
            assert!(
                <NominalWeakeningBack as StepBack<4>>::prepared_output(&back, KPort(0)).is_none()
            );
        }
        assert_eq!(back.committed_frames(), 1);
    }
}
#[test]
fn pressure_storage_rollback_and_cancel_preserve_committed_progress() {
    let profile = profile();
    let input = [
        input(&profile, 31),
        input(&profile, 33),
        input(&profile, 256),
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
    let mut run = scheduler(
        &input,
        3,
        Rc::new(Cell::new(false)),
        seen.clone(),
        Rc::new(Cell::new(0)),
    );
    assert!((0..24).any(|_| run.step().is_err()));
    assert!(seen.borrow().is_empty());
}
#[test]
fn value_is_one_shot_and_exact_temporal_and_schema_selection_is_required() {
    let profile = profile();
    let plan = plan(&profile, false);
    let gear = plan.fragments[0]
        .placements
        .iter()
        .find(|g| g.implementation_id.as_str() == WEAKENING_IMPLEMENTATION)
        .unwrap();
    assert!(NominalWeakeningBack::prepare_planned::<4>(gear, 2, &profile, true).is_err());
    let foreign = PreparedNominalWeakening::prepare(
        StructuredInfoType::collection(
            StructuredInfoType::leaf(kind_id("value/u16")).unwrap(),
            Some(2),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(NominalWeakeningBack::prepare_planned::<4>(gear, 2, &foreign, false).is_err());
    let mut back = NominalWeakeningBack::prepare_planned::<4>(gear, 2, &profile, false).unwrap();
    let bytes = input(&profile, 31);
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
    <NominalWeakeningBack as StepBack<4>>::step_committed(&mut back);
    assert_eq!(back.step(&mut io, &inputs), StepOutcome::Complete);
    assert_eq!(back.committed_frames(), 1);
}

#[test]
fn weakening_candidate_matches_separately_checked_native_admission_but_carries_no_laws() {
    let native =
        conduit_ai::native_profile::PreparedNativeProfile::check_definition(DEFINITION, "Payload")
            .unwrap();
    let weak = PreparedNominalWeakening::prepare(native.value_type().clone()).unwrap();
    assert_eq!(weak.output_type(), native.candidate_type());
    assert_ne!(weak.input_type(), weak.output_type());
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    weak.install(&mut startup, &mut profiles, true).unwrap();
    let source = format!("with {}/result as Bare\nplot forward (\n >> value: Bare...|\n result: Bare...| >>\n) = (.)\n", weak.kind_identity(true));
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(&source),
        &startup,
    )
    .unwrap();
    let expanded =
        conduit_plot::expand_canonical_plot_for_authoring(&checked, "forward", &profiles).unwrap();
    assert_eq!(expanded.expanded.gears.len(), 1);
    let mut oversized = weak.input_type().clone();
    for _ in 0..10 {
        oversized = StructuredInfoType::collection(oversized, Some(2)).unwrap();
    }
    assert!(PreparedNominalWeakening::prepare(oversized).is_err());
}

#[test]
fn exact_anonymous_tuple_weakening_preserves_named_members_and_refuses_impostors() {
    let admitted = profile();
    let tuple = tuple_info_type(vec![
        admitted.input_type().clone(),
        admitted.input_type().clone(),
    ])
    .unwrap();
    let bare = PreparedNominalWeakening::prepare(tuple.clone()).unwrap();
    let StructuredInfoTypeShape::Record { fields, schema } = tuple.shape() else {
        panic!("tuple")
    };
    let forged = StructuredInfoType::record(
        schema.clone(),
        vec![StructuredFieldType::new("foreign", admitted.input_type().clone()).unwrap()],
    )
    .unwrap();
    assert!(PreparedNominalWeakening::prepare(forged).is_err());
    let StructuredInfoTypeShape::Record {
        fields: output_fields,
        ..
    } = bare.output_type().shape()
    else {
        panic!("bare tuple")
    };
    assert_ne!(bare.output_type(), &tuple);
    assert_ne!(
        bare.output_type().profile().unwrap().value_kind(),
        tuple.profile().unwrap().value_kind()
    );
    assert!(PreparedNominalWeakening::prepare(
        tuple_info_type(
            output_fields
                .iter()
                .map(|field| field.value_type().clone())
                .collect()
        )
        .unwrap()
    )
    .is_err());
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    bare.install(&mut startup, &mut profiles, true).unwrap();
    startup
        .insert_structured_type("Member", output_fields[0].value_type().clone())
        .unwrap();
    let source = format!("with {}/result as Bare\nplot select (\n >> value: Bare...|\n result: Member...| >>\n) = (.item-00000)\n", bare.kind_identity(true));
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(&source),
        &startup,
    )
    .unwrap();
    assert_eq!(
        conduit_plot::expand_canonical_plot_for_authoring(&checked, "select", &profiles)
            .unwrap()
            .expanded
            .gears
            .len(),
        1
    );
    let impostor = StructuredInfoType::record(kind_id("ordinary-record"), fields.to_vec()).unwrap();
    let impostor = PreparedNominalWeakening::prepare(impostor).unwrap();
    impostor.install(&mut startup, &mut profiles, true).unwrap();
    let source = source
        .replace(&bare.kind_identity(true), &impostor.kind_identity(true))
        .replace(".item-00000", ".0");
    let refused = match conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(&source),
        &startup,
    ) {
        Err(_) => true,
        Ok(checked) => {
            conduit_plot::expand_canonical_plot_for_authoring(&checked, "select", &profiles)
                .is_err()
        }
    };
    assert!(refused);
}

#[test]
fn corrupted_sealed_plan_is_refused_before_owner_preparation() {
    let profile = std::sync::Arc::new(profile());
    let mut plan = plan(&profile, true);
    plan.plan_id = "foreign".into();
    assert!(conduit_ai::operation_owners::nominal_weakening::NominalWeakeningOperationFactory::for_plan(&plan,&[profile]).is_err());
}
