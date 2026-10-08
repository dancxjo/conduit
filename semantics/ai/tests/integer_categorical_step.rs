#![cfg(all(feature = "kernel-operation-owners", target_has_atomic = "ptr"))]
use conduit_ai::integer_categorical_step::*;
use conduit_composite::KernelOperationFactory;
use conduit_core::*;
use conduit_kernel::{
    scheduler::FixedScheduler, FixedRoutes, HostedSignLog, HostedValueStore, KernelEvent,
    ValueStorage,
};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    PortId as KPort, ValueRef,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};
const PORTS: usize = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
#[allow(dead_code)]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

#[path = "common/integer_categorical_fixture.rs"]
mod categorical_fixture;
use categorical_fixture::*;
fn fixture_kind(name: &str, ty: &StructuredInfoType, source: bool, flow: bool) -> Kind {
    let port = PortDescriptor {
        port_id: port_id("value"),
        value_kind: ty.profile().unwrap().value_kind().clone(),
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
    let bound = conduit_plot::maximum_prepared_canonical_value_bytes(ty).unwrap();
    Kind {
        kind_id: name.into(),
        kind_contract_revision: "test/fixture@1".into(),
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
            max_queue_bytes: 16_384,
        },
    }
}
fn plan(
    profile: &PreparedCategoricalStep,
    flow: bool,
    resource_present: bool,
) -> Result<Plan, String> {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    profile.install(&mut startup, &mut profiles, flow)?;
    let mut offers = vec![profile.offer(flow)?];
    for (name, ty, source) in [
        ("categorical-test/source", profile.indices_type(), true),
        ("categorical-test/sink", profile.scores_type(), false),
    ] {
        let kind = fixture_kind(name, ty, source, flow);
        startup.insert(conduit_plot::KindSignature {
            kind: name.into(),
            startup_parameters: vec![],
        })?;
        profiles
            .insert_kind(kind.clone())
            .map_err(|e| format!("{e:?}"))?;
        offers.push(
            BackOfferBuilder::new(
                kind,
                Back {
                    capability_id: name.into(),
                    execution_profile_id: "test/fixture@1".into(),
                    implementation_id: name.into(),
                    artifact_id: name.into(),
                    host_calls: vec![],
                    resource_requirements: vec![],
                    authority_requirements: vec![],
                },
            )
            .build(),
        );
    }
    let source = format!("plot categorical-proof {{\n source: categorical-test/source\n model: {}\n sink: categorical-test/sink\n source.value >> model.indices\n model.scores >> sink.value\n}}\n", profile.kind_identity(flow));
    let plot = conduit_plot::parse_with_startup(&source, &startup, &profiles)
        .map_err(|e| format!("{e:?}"))?;
    let hosts = [HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "categorical-test".into(),
        boot_id: "categorical-test-boot".into(),
        offer_generation: OfferGeneration(1),
        profile: "test/host@1".into(),
        bases: vec![],
        resources: if resource_present {
            vec![profile.resource_offer().clone()]
        } else {
            vec![]
        },
        planner_capabilities: vec![],
        capabilities: offers,
    }];
    let placements =
        conduit_planner::default_placements(&plot, &hosts).map_err(|e| format!("{e:?}"))?;
    conduit_planner::plan_with_connection_limits(
        &plot,
        &hosts,
        &placements,
        &["conduit.base/local@1".into()],
        1,
        conduit_plot::maximum_prepared_canonical_value_bytes(profile.indices_type())
            .unwrap()
            .max(
                conduit_plot::maximum_prepared_canonical_value_bytes(profile.scores_type())
                    .unwrap(),
            ),
    )
    .map_err(|e| format!("{e:?}"))
}
fn gear(plan: &Plan) -> &PlannedGear {
    plan.fragments[0]
        .placements
        .iter()
        .find(|g| g.implementation_id.as_str() == CATEGORICAL_STEP_IMPLEMENTATION)
        .unwrap()
}

#[test]
fn exact_resource_planning_and_model_adoption_refuse_missing_foreign_or_unsealed_selection() {
    let profile = profile();
    assert_eq!(profile.maximum_score_magnitude(), 10);
    assert!(plan(&profile, true, false).is_err());
    let plan = plan(&profile, true, true).unwrap();
    assert!(verify_plan(&plan));
    assert_eq!(gear(&plan).resources.len(), 1);
    owner::CategoricalOperationFactory::for_plan(&plan, std::slice::from_ref(&profile)).unwrap();
    assert!(owner::CategoricalOperationFactory::for_plan(&plan, &[]).is_err());
    assert!(owner::CategoricalOperationFactory::for_plan(
        &plan,
        &[profile.clone(), profile.clone()]
    )
    .is_err());
    let mut foreign = plan.clone();
    foreign.fragments[0].placements[0].artifact_id = "foreign".into();
    assert!(
        owner::CategoricalOperationFactory::for_plan(&foreign, std::slice::from_ref(&profile))
            .is_err()
    );
    let mut wrong = gear(&plan).clone();
    wrong.resources[0].pool_id = "foreign".into();
    assert!(CategoricalStepBack::prepare_planned::<4>(&wrong, 2, profile.clone(), true).is_err());
    let (model, mut residence) = fixture();
    residence.owner_host = "foreign-host".into();
    let different = Arc::new(
        PreparedCategoricalStep::prepare(model, "test/model-pool".into(), residence).unwrap(),
    );
    assert!(owner::CategoricalOperationFactory::for_plan(&plan, &[different]).is_err());
    assert!(CategoricalStepBack::prepare_planned::<4>(gear(&plan), 2, profile, false).is_err());
}

#[test]
fn prepared_exact_integer_inference_is_allocation_free_and_commits_receipts_only_after_ack() {
    let profile = profile();
    let plan = plan(&profile, true, true).unwrap();
    let mut back =
        CategoricalStepBack::prepare_planned::<4>(gear(&plan), 2, profile.clone(), true).unwrap();
    for (invocation, (values, expected)) in [([0, 2], [3, 8]), ([1, 1], [4, -4]), ([2, 0], [3, 8])]
        .into_iter()
        .enumerate()
    {
        let bytes = indices(&profile, &values);
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
        let (outcome, allocations) = allocation_probe::observe(|| back.step(&mut io, &inputs));
        assert_eq!(allocations.allocations, 0);
        assert_eq!(allocations.reallocations, 0);
        assert_eq!(outcome, StepOutcome::Progress);
        assert_eq!(back.committed_invocations(), invocation as u64);
        assert_eq!(
            back.last_receipt().map(|r| r.invocation),
            (invocation > 0).then_some(invocation as u64)
        );
        let output =
            <CategoricalStepBack as StepBack<4>>::prepared_output(&back, KPort(0)).unwrap();
        assert_eq!(scores(output), expected);
        let output_identity = semantic_digest("numeric/categorical-scores@1", output);
        <CategoricalStepBack as StepBack<4>>::step_committed(&mut back);
        assert_eq!(back.committed_invocations(), invocation as u64 + 1);
        let receipt = back.last_receipt().unwrap();
        assert_eq!(
            receipt.model_content,
            profile.resource().artifact().content_identity()
        );
        assert_eq!(
            receipt.input,
            semantic_digest("numeric/categorical-indices@1", &bytes)
        );
        assert_eq!(receipt.output, output_identity);
        assert_eq!(receipt.work_units, 4);
    }
}

#[test]
fn bad_indices_malformed_frame_pressure_and_cancel_do_not_consume_or_advance_model_progress() {
    let profile = profile();
    let plan = plan(&profile, true, true).unwrap();
    let mut back =
        CategoricalStepBack::prepare_planned::<4>(gear(&plan), 2, profile.clone(), true).unwrap();
    let good = indices(&profile, &[0, 2]);
    let mut tail = good.clone();
    tail.push(0);
    let mut truncated = good.clone();
    truncated.pop();
    for bytes in [
        indices(&profile, &[0, 3]),
        indices(&profile, &[u64::MAX, 0]),
        tail,
        truncated,
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
        let inputs = StepInputBytes::test_frame([Some(bytes.as_slice()), None, None, None], None);
        let (outcome, allocations) = allocation_probe::observe(|| back.step(&mut io, &inputs));
        assert_eq!(allocations.allocations, 0);
        assert_eq!(allocations.reallocations, 0);
        assert!(matches!(outcome, StepOutcome::Fail(_)));
        assert!(!io.test_consumed(KPort(0)));
        assert_eq!(back.committed_invocations(), 0);
        assert_eq!(back.last_receipt(), None);
        assert!(<CategoricalStepBack as StepBack<4>>::prepared_output(&back, KPort(0)).is_none());
    }
    let reference = ValueRef {
        slot: 0,
        generation: 1,
        byte_len: good.len() as u32,
    };
    let mut io = StepIo::test_frame(
        [Some(reference), None, None, None],
        [false; 4],
        [None; 4],
        None,
        2,
    );
    let inputs = StepInputBytes::test_frame([Some(good.as_slice()), None, None, None], None);
    assert_eq!(back.step(&mut io, &inputs), StepOutcome::Await);
    assert!(!io.test_consumed(KPort(0)));
    assert_eq!(back.committed_invocations(), 0);
    <CategoricalStepBack as StepBack<4>>::cancel(&mut back);
    assert!(matches!(back.step(&mut io, &inputs), StepOutcome::Fail(_)));
    assert_eq!(back.last_receipt(), None);
}

#[test]
fn value_operation_finishes_once_while_closing_flow_reports_exact_end_of_input() {
    let profile = profile();
    let value_plan = plan(&profile, false, true).unwrap();
    let mut value =
        CategoricalStepBack::prepare_planned::<4>(gear(&value_plan), 2, profile.clone(), false)
            .unwrap();
    let bytes = indices(&profile, &[0, 2]);
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
    assert_eq!(value.step(&mut io, &inputs), StepOutcome::Progress);
    <CategoricalStepBack as StepBack<4>>::step_committed(&mut value);
    assert_eq!(value.step(&mut io, &inputs), StepOutcome::Complete);
    let flow_plan = plan(&profile, true, true).unwrap();
    let mut flow =
        CategoricalStepBack::prepare_planned::<4>(gear(&flow_plan), 2, profile, true).unwrap();
    let mut io = StepIo::test_frame(
        [None; 4],
        [true, false, false, false],
        [Some(4096), None, None, None],
        None,
        2,
    );
    let inputs = StepInputBytes::test_frame([None; 4], None);
    assert_eq!(flow.step(&mut io, &inputs), StepOutcome::Complete);
    assert_eq!(flow.committed_invocations(), 0);
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
    let profile = profile();
    let plan = plan(&profile, true, true).unwrap();
    let fragment = &plan.fragments[0];
    let (lowered, _) = conduit_plan_lowering::lowering::lower_plan_fragment_from_plan(
        &plan,
        &fragment.fragment_id,
    )
    .unwrap();
    let mut values = HostedValueStore::new(capacity as u16, 4096, capacity as u32 * 4096).unwrap();
    let inputs: Vec<_> = input.iter().map(|x| values.store(x).unwrap()).collect();
    let owner = owner::CategoricalOperationFactory::for_plan(&plan, &[profile]).unwrap();
    let drivers: Vec<_> = fragment
        .placements
        .iter()
        .map(|gear| match gear.kind_id.as_str() {
            "categorical-test/source" => Driver::Source(inputs.clone(), 0, false),
            "categorical-test/sink" => Driver::Sink(blocked.clone(), seen.clone(), None),
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
fn ordinary_scheduler_reuses_adopted_model_for_three_frames_after_provider_drop() {
    let profile = profile();
    let inputs = [[0, 2], [1, 1], [2, 0]].map(|v| indices(&profile, &v));
    drop(profile);
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
    assert_eq!(
        seen.borrow().iter().map(|b| scores(b)).collect::<Vec<_>>(),
        vec![vec![3, 8], vec![4, -4], vec![3, 8]]
    );
}
#[test]
fn scheduler_pressure_cancel_and_storage_failure_do_not_publish_uncommitted_frames() {
    let profile = profile();
    let inputs = [[0, 2], [1, 1], [2, 0]].map(|v| indices(&profile, &v));
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
    let mut run = scheduler(
        &inputs,
        3,
        Rc::new(Cell::new(false)),
        seen.clone(),
        Rc::new(Cell::new(0)),
    );
    assert!((0..24).any(|_| run.step().is_err()));
    assert!(seen.borrow().is_empty());
}

#[test]
fn pinned_learned_v2_parameter_blob_matches_all_76_scores_through_prepared_step() {
    let bytes = include_bytes!("../../language/training/ewt_joint_v2/ewt_joint.i16");
    let (model, residence) = fixture_model(bytes.to_vec(), 25, 76);
    let profile = Arc::new(
        PreparedCategoricalStep::prepare(model, "test/model-pool".into(), residence).unwrap(),
    );
    assert_eq!(profile.maximum_score_magnitude(), 128525);
    let plan = plan(&profile, true, true).unwrap();
    let mut back =
        CategoricalStepBack::prepare_planned::<4>(gear(&plan), 2, profile.clone(), true).unwrap();
    for seed in [0u64, 17, 412] {
        let lookups: Vec<_> = (0..25).map(|i| (seed + 31 * i) % 413).collect();
        let input = indices(&profile, &lookups);
        let reference = ValueRef {
            slot: 0,
            generation: 1,
            byte_len: input.len() as u32,
        };
        let mut io = StepIo::test_frame(
            [Some(reference), None, None, None],
            [false; 4],
            [Some(16384), None, None, None],
            None,
            2,
        );
        let inputs = StepInputBytes::test_frame([Some(input.as_slice()), None, None, None], None);
        let (out, observed) = allocation_probe::observe(|| back.step(&mut io, &inputs));
        assert_eq!(out, StepOutcome::Progress);
        assert_eq!(observed.allocations, 0);
        assert_eq!(observed.reallocations, 0);
        let actual =
            scores(<CategoricalStepBack as StepBack<4>>::prepared_output(&back, KPort(0)).unwrap());
        let expected: Vec<i64> = (0..76)
            .map(|row| {
                lookups
                    .iter()
                    .map(|index| {
                        let offset = 20 + 2 * (row * 413 + *index as usize);
                        i16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap()) as i64
                    })
                    .sum()
            })
            .collect();
        assert_eq!(actual, expected);
        <CategoricalStepBack as StepBack<4>>::step_committed(&mut back);
    }
    assert_eq!(back.committed_invocations(), 3);
}
