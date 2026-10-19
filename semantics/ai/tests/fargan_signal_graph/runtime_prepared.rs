//! One-time exact Plan, owner, resource and storage preparation for the fixture.
use super::*;
pub(super) type Kernel =
    FixedScheduler<Driver, ObservedValues, HostedSignLog, N, C, PORTS, C, C, C, N, 256>;
type SinkCapture = Rc<std::cell::RefCell<Option<Vec<u8>>>>;
pub(super) struct PreparedExecution {
    pub(super) plan: Plan,
    pub(super) scheduler: super::execution_kernel::ExecutionKernel,
    pub(super) sources: Vec<(usize, String, Option<conduit_kernel::ValueRef>)>,
    pub(super) timing: Rc<std::cell::RefCell<WorkTiming>>,
    pub(super) owners: Vec<Option<conduitos::expression_host_call::ExpressionHostCall>>,
    pub(super) received: BTreeMap<String, SinkCapture>,
    pub(super) host_output: Vec<u8>,
    pub(super) output_limits: [u32; N],
    pub(super) preparation: Duration,
    pub(super) nodes: usize,
    pub(super) cords: usize,
}
pub(super) fn prepare(
    plan: Plan,
    resources: &Resources,
    input_values: BTreeMap<String, Vec<u8>>,
    mode: ExecutionMode,
    reuse: bool,
    sequence: u64,
) -> PreparedExecution {
    let start = Instant::now();
    let timing = Rc::new(std::cell::RefCell::new(WorkTiming::default()));
    assert!(verify_plan(&plan));
    let fragment = &plan.fragments[0];
    let (lowered, _) = conduit_plan_lowering::lowering::lower_plan_fragment_from_plan(
        &plan,
        &fragment.fragment_id,
    )
    .unwrap();
    assert!(lowered.node_specs.len() <= N && lowered.cords.len() <= C);
    let dags = if fragment.execution_fusions.is_empty() {
        Vec::new()
    } else {
        conduit_plan_lowering::lowering::lower_pure_value_fusion_dags(&plan, &fragment.fragment_id)
            .unwrap()
    };
    let compact_nodes: std::collections::BTreeSet<_> = dags
        .iter()
        .filter(|dag| {
            dag.execution_profile_id.as_str()
                == conduit_ai::fixed_numeric_compact::COMPACT_DAG_PROFILE
        })
        .flat_map(|dag| dag.nodes.iter().copied())
        .collect();
    let active = bind_active_play(
        &fragment.plan_id,
        &fragment.host_id,
        &fragment.boot_id,
        sequence,
    );
    let slots = match mode {
        ExecutionMode::StoragePressure => resources.len() + input_values.len(),
        _ => 1024,
    };
    let mut store = ObservedValues::new(
        slots.try_into().unwrap(),
        16384,
        (slots * 16384).try_into().unwrap(),
    )
    .unwrap();
    if !compact_nodes.is_empty() {
        store.prepare_compact_f32();
    }
    let mut fixtures: BTreeMap<_, _> = input_values
        .iter()
        .map(|(name, encoded)| (name.clone(), store.store(encoded).unwrap()))
        .collect();
    for (name, resource) in resources {
        let binding =
            FixedTensorPortBinding::prepare(&resource.value_type, &resource.tensor).unwrap();
        let reference = store.store(binding.encoded()).unwrap();
        if reuse {
            store.retain(reference).unwrap();
        }
        fixtures.insert(name.clone(), reference);
    }
    let mut adopted = BTreeMap::new();
    for gear in &fragment.placements {
        if let Some(name) = gear.kind_id.as_str().strip_prefix("subframe-fixture/") {
            if let Some(resource) = resources.get(name) {
                adopted.insert(gear.placement_id.clone(), resource.adopt());
            }
        }
    }
    let factories =
        conduit_ai::operation_owners::fixed_numeric::FixedNumericOperationFactory::for_plan_capacity64(
            &plan, &adopted,
        )
        .unwrap();
    let mut received = BTreeMap::new();
    let mut owners: Vec<Option<conduitos::expression_host_call::ExpressionHostCall>> =
        (0..N).map(|_| None).collect();
    let mut drivers = Vec::new();
    let mut sources = Vec::new();
    let expressions =
        conduitos::expression_host_call::ExpressionPreparation::new(fragment, &lowered, &active)
            .unwrap();
    for (index, gear) in fragment.placements.iter().enumerate() {
        if gear.kind_id.as_str().starts_with("numeric/") {
            let factory = factories
                .iter()
                .find(|f| f.implementation_id() == &gear.implementation_id)
                .unwrap();
            factory.budget(gear).unwrap();
            if compact_nodes.contains(&conduit_kernel::NodeId(index as u16)) {
                drivers.push(Driver::ReusableOperation(Box::new(TimedNumeric {
                    back: factory.prepare_compact_reusable(gear).unwrap(),
                    timing: timing.clone(),
                })));
                continue;
            }
            drivers.push(if reuse {
                Driver::ReusableOperation(Box::new(TimedNumeric {
                    back: factory.prepare_reusable(gear).unwrap(),
                    timing: timing.clone(),
                }))
            } else {
                Driver::Operation(Box::new(TimedNumeric {
                    back: factory.prepare(gear, &mut store).unwrap(),
                    timing: timing.clone(),
                }))
            });
        } else if gear.implementation_id.as_str() == conduitos::expression_host_call::IMPLEMENTATION
        {
            let owner = expressions.prepare(&gear.placement_id).unwrap();
            owners[index] = Some(owner);
            let back = conduit_kernel::scheduler::HostCallBack::new(
                gear.host_calls[0].maximum_input_bytes,
            );
            drivers.push(if reuse {
                Driver::ReusableOperation(Box::new(back))
            } else {
                Driver::Operation(Box::new(back))
            });
        } else {
            let name = gear
                .kind_id
                .as_str()
                .strip_prefix("subframe-fixture/")
                .unwrap();
            if let Some(reference) = fixtures.get(name) {
                sources.push((
                    index,
                    name.to_owned(),
                    resources.contains_key(name).then_some(*reference),
                ));
                drivers.push(Driver::Source {
                    reference: *reference,
                    staged: false,
                    sent: false,
                });
            } else {
                let value = Rc::new(std::cell::RefCell::new(None));
                received.insert(name.to_owned(), value.clone());
                drivers.push(Driver::Sink {
                    received: value,
                    staged: Some(Vec::with_capacity(16384)),
                });
            }
        }
    }
    drop(factories);
    drop(adopted); // drivers retain immutable admitted resource custody.
    let nodes = drivers.len();
    while drivers.len() < N {
        drivers.push(Driver::Inactive);
    }
    let cords = lowered.cords.len();
    let mut routes = FixedRoutes::<C, C>::new(1);
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
    let mut bindings = conduit_kernel::FixedHostCallBindings::<N>::new(1);
    for b in &lowered.host_calls {
        bindings.install(b.node, b.binding).unwrap();
    }
    bindings.seal().unwrap();
    let mut specs = lowered.node_specs.clone();
    specs.resize(
        N,
        conduit_kernel::scheduler::NodeSpec {
            input_cords: [None; PORTS],
            maximum_step_fuel: 0,
        },
    );
    let mut cord_specs: Vec<_> = lowered.cords.iter().map(|c| c.spec).collect();
    cord_specs.resize(C, conduit_kernel::scheduler::CordSpec::inactive());
    let mut scheduler=FixedScheduler::<_,_,_,N,C,PORTS,C,C,C,N,256>::new_boxed_with_active_counts_and_host_calls(nodes,cords,specs.try_into().unwrap(),cord_specs.try_into().unwrap(),routes,bindings,drivers.try_into().unwrap_or_else(|_|panic!("capacity")),store,HostedSignLog::new(32768,32768*core::mem::size_of::<KernelEvent>()as u32).unwrap()).unwrap();
    for dag in &dags {
        use conduit_ai::fixed_numeric_compact::*;
        assert_eq!(dag.artifact_id.as_str(), DAG_ARTIFACT);
        if dag.execution_profile_id.as_str() == COMPACT_DAG_PROFILE {
            assert_eq!(dag.implementation_id.as_str(), COMPACT_DAG_IMPLEMENTATION);
            scheduler.bind_compact_f32_dag(&dag.nodes).unwrap();
        } else {
            assert_eq!(dag.execution_profile_id.as_str(), CANONICAL_DAG_PROFILE);
            assert_eq!(dag.implementation_id.as_str(), CANONICAL_DAG_IMPLEMENTATION);
            scheduler.bind_execution_dag(&dag.nodes).unwrap();
        }
    }
    timing.borrow_mut().scheduler_bytes = core::mem::size_of_val(&*scheduler);
    let host_output = Vec::with_capacity(16384);
    let mut output_limits = [u32::MAX; N];
    for route in &lowered.routes {
        for target in &route.targets {
            output_limits[usize::from(route.source_node.0)] =
                output_limits[usize::from(route.source_node.0)].min(
                    lowered.cords[usize::from(target.cord.0)]
                        .spec
                        .maximum_value_bytes,
                );
        }
    }
    let scheduler = if reuse {
        super::execution_kernel::ExecutionKernel::Reusable(Box::new(
            conduit_kernel::scheduler::prepared_session::PreparedKernelSession::new(scheduler)
                .unwrap_or_else(|_| panic!("fresh local kernel")),
        ))
    } else {
        super::execution_kernel::ExecutionKernel::Fresh(scheduler)
    };
    let preparation = start.elapsed();
    PreparedExecution {
        plan,
        scheduler,
        sources,
        timing,
        owners,
        received,
        host_output,
        output_limits,
        preparation,
        nodes,
        cords,
    }
}
