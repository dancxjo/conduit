use super::{plan::*, shared::*, state::*};
use conduit_ai::fixed_numeric_binding::*;
use conduit_composite::KernelOperationFactory;
use conduit_core::*;
use conduit_kernel::{
    scheduler::FixedScheduler, BoundedValueRef, FixedRoutes, HostedSignLog, HostedValueStore,
    KernelEvent, ValueStorage,
};
use std::{
    collections::BTreeMap,
    rc::Rc,
    time::{Duration, Instant},
};
const PORTS: usize = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const N: usize = 1024;
const C: usize = 2048;
#[path = "runtime_backs.rs"]
mod backs;
pub use backs::WorkTiming;
use backs::{Driver, TimedNumeric};

pub struct ResultAndTiming {
    pub work: WorkTiming,
    pub pcm: [f32; 40],
    pub state: State,
    pub preparation: Duration,
    pub execution: Duration,
    pub nodes: usize,
    pub cords: usize,
}
#[derive(Clone, Copy)]
pub enum ExecutionMode {
    Normal,
    StoragePressure,
    CancelFirstExpression,
}
pub fn try_run_graph(
    schema: &SourceSchema,
    resources: &Resources,
    condition: [f32; 80],
    period: u16,
    state: &State,
    mode: ExecutionMode,
) -> Option<ResultAndTiming> {
    let planning = Instant::now();
    let plan = prepare_plan(schema);
    let planning = planning.elapsed();
    let fixtures = encode_inputs(schema, condition, period, state);
    let result = run_entry_plan(plan, resources, fixtures, mode)?;
    Some(ResultAndTiming {
        work: result.work,
        pcm: floats(field(&result.value, "pcm_f32")).try_into().unwrap(),
        state: State::from_result(&result.value),
        preparation: planning + result.preparation,
        execution: result.execution,
        nodes: result.nodes,
        cords: result.cords,
    })
}
pub fn encode_inputs(
    schema: &SourceSchema,
    condition: [f32; 80],
    period: u16,
    state: &State,
) -> BTreeMap<String, Vec<u8>> {
    let mut fixtures = BTreeMap::new();
    fixtures.insert(
        "condition".to_owned(),
        vector(schema.ty("condition"), &condition)
            .canonical_bytes()
            .unwrap(),
    );
    fixtures.insert("state".to_owned(), state.encode(schema.ty("state")));
    let primitive = StructuredInfoValue::leaf(
        StructuredInfoType::leaf(kind_id("value/u16")).unwrap(),
        period.to_le_bytes().to_vec(),
    )
    .unwrap();
    let encoded = StructuredInfoValue::nominal(schema.ty("period").clone(), primitive)
        .unwrap()
        .canonical_bytes()
        .unwrap();
    fixtures.insert("period".to_owned(), encoded);
    fixtures
}

pub struct EncodedResultAndTiming {
    pub signs: Vec<KernelEvent>,
    pub outputs: BTreeMap<String, StructuredInfoValue>,
    pub work: WorkTiming,
    pub value: StructuredInfoValue,
    pub preparation: Duration,
    pub execution: Duration,
    pub nodes: usize,
    pub cords: usize,
}
pub fn run_entry_plan(
    plan: Plan,
    resources: &Resources,
    input_values: BTreeMap<String, Vec<u8>>,
    mode: ExecutionMode,
) -> Option<EncodedResultAndTiming> {
    run_entry_plan_observed(plan, resources, input_values, mode, || {}, || {})
}
pub fn run_entry_plan_observed(
    plan: Plan,
    resources: &Resources,
    input_values: BTreeMap<String, Vec<u8>>,
    mode: ExecutionMode,
    before_play: impl FnOnce(),
    after_play: impl FnOnce(),
) -> Option<EncodedResultAndTiming> {
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
    let active = bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let slots = match mode {
        ExecutionMode::StoragePressure => resources.len() + input_values.len(),
        _ => 1024,
    };
    let mut store = HostedValueStore::new(
        slots.try_into().unwrap(),
        16384,
        (slots * 16384).try_into().unwrap(),
    )
    .unwrap();
    let mut fixtures: BTreeMap<_, _> = input_values
        .iter()
        .map(|(name, encoded)| (name.clone(), store.store(encoded).unwrap()))
        .collect();
    for (name, resource) in resources {
        let binding =
            FixedTensorPortBinding::prepare(&resource.value_type, &resource.tensor).unwrap();
        fixtures.insert(name.clone(), store.store(binding.encoded()).unwrap());
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
            drivers.push(Driver::Operation(Box::new(TimedNumeric {
                back: factory.prepare(gear, &mut store).unwrap(),
                timing: timing.clone(),
            })));
        } else if gear.implementation_id.as_str() == conduitos::expression_host_call::IMPLEMENTATION
        {
            let owner = expressions.prepare(&gear.placement_id).unwrap();
            owners[index] = Some(owner);
            drivers.push(Driver::Operation(Box::new(
                conduit_kernel::scheduler::HostCallBack::new(
                    gear.host_calls[0].maximum_input_bytes,
                ),
            )));
        } else {
            let name = gear
                .kind_id
                .as_str()
                .strip_prefix("subframe-fixture/")
                .unwrap();
            if let Some(reference) = fixtures.get(name) {
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
    timing.borrow_mut().scheduler_bytes = core::mem::size_of_val(&*scheduler);
    let mut host_output = Vec::with_capacity(16384);
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
    let preparation = start.elapsed();
    before_play();
    let execute = Instant::now();
    let mut drained = false;
    for _ in 0..8192 {
        let step_start = Instant::now();
        let step_result = scheduler.step();
        let elapsed = step_start.elapsed();
        {
            let mut t = timing.borrow_mut();
            t.steps += 1;
            t.kernel_steps += elapsed;
        }
        if step_result.is_err() {
            break;
        }
        if let Some(call) = scheduler.next_host_request() {
            if matches!(mode, ExecutionMode::CancelFirstExpression) {
                owners[usize::from(call.node.0)]
                    .as_mut()
                    .unwrap()
                    .cancel(call.node, call.call)
                    .unwrap();
                scheduler
                    .complete_host_call(
                        call.node,
                        call.request,
                        conduit_kernel::HostCallOutcome {
                            disposition: conduit_kernel::HostCallDisposition::Cancelled,
                            output: None,
                            failure: None,
                        },
                    )
                    .unwrap();
                // Deliver the ordinary host cancellation to its selected Back.
                for _ in 0..nodes {
                    let _ = scheduler.step();
                }
                break;
            }
            let input = scheduler.values().get(call.input.value).unwrap();
            let expression_start = Instant::now();
            let output = owners[usize::from(call.node.0)]
                .as_mut()
                .unwrap()
                .invoke(call.node, call.call, call.request, input)
                .unwrap();
            assert!(
                output.len() as u32 <= output_limits[usize::from(call.node.0)],
                "expression output exceeds cord: node={:?} bytes={} maximum={} placement={:?}",
                call.node,
                output.len(),
                output_limits[usize::from(call.node.0)],
                fragment.placements[usize::from(call.node.0)].gear_id
            );
            let elapsed = expression_start.elapsed();
            {
                let mut t = timing.borrow_mut();
                t.expressions += elapsed;
                t.expression_calls += 1;
            }
            assert!(output.len() <= host_output.capacity());
            host_output.clear();
            host_output.extend_from_slice(output);
            let output = &host_output;
            let Ok(value) = scheduler.store_host_value(output) else {
                break;
            };
            scheduler
                .complete_host_call(
                    call.node,
                    call.request,
                    conduit_kernel::HostCallOutcome {
                        disposition: conduit_kernel::HostCallDisposition::Completed,
                        output: Some(BoundedValueRef::new(value, output.len() as u32).unwrap()),
                        failure: None,
                    },
                )
                .unwrap();
        }
        if matches!(
            step_result,
            Ok(conduit_kernel::scheduler::SchedulerStatus::Drained)
        ) {
            drained = true;
            break;
        }
    }
    let execution = execute.elapsed();
    after_play();
    if !drained || scheduler.pending_host_call_count() != 0 {
        return None;
    }
    let outputs: BTreeMap<_, _> = received
        .into_iter()
        .map(|(name, value)| {
            let bytes = value.borrow_mut().take()?;
            Some((
                name,
                StructuredInfoValue::from_canonical_bytes(&bytes).unwrap(),
            ))
        })
        .collect::<Option<_>>()?;
    let actual = outputs
        .get("result")
        .or_else(|| outputs.values().next())?
        .clone();
    let signs: Vec<_> = scheduler.signs().events().collect();
    timing.borrow_mut().signs = signs.len();
    let work = core::mem::take(&mut *timing.borrow_mut());
    Some(EncodedResultAndTiming {
        signs,
        outputs,
        work,
        value: actual,
        preparation,
        execution,
        nodes,
        cords,
    })
}
pub fn run_graph(
    schema: &SourceSchema,
    resources: &Resources,
    condition: [f32; 80],
    period: u16,
    state: &State,
) -> ResultAndTiming {
    try_run_graph(
        schema,
        resources,
        condition,
        period,
        state,
        ExecutionMode::Normal,
    )
    .expect("one committed aggregate")
}
