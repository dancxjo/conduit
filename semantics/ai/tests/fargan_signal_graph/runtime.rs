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
#[path = "runtime_execution_kernel.rs"]
mod execution_kernel;
#[path = "runtime_prepared.rs"]
mod prepared;
#[path = "runtime_session.rs"]
pub mod session;

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
    let mut prepared = prepared::prepare(plan, resources, input_values, mode, false, 0);
    execute_prepared(&mut prepared, mode, before_play, after_play)
}
pub fn run_entry_plan_at_sequence(
    plan: Plan,
    resources: &Resources,
    input_values: BTreeMap<String, Vec<u8>>,
    sequence: u64,
) -> Option<EncodedResultAndTiming> {
    let mut prepared = prepared::prepare(
        plan,
        resources,
        input_values,
        ExecutionMode::Normal,
        false,
        sequence,
    );
    execute_prepared(&mut prepared, ExecutionMode::Normal, || {}, || {})
}
fn execute_prepared(
    prepared: &mut prepared::PreparedExecution,
    mode: ExecutionMode,
    before_play: impl FnOnce(),
    after_play: impl FnOnce(),
) -> Option<EncodedResultAndTiming> {
    let prepared::PreparedExecution {
        plan,
        scheduler,
        timing,
        owners,
        received,
        host_output,
        output_limits,
        preparation,
        nodes,
        cords,
        ..
    } = prepared;
    let fragment = &plan.fragments[0];
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
                for _ in 0..*nodes {
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
            let output = &*host_output;
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
        .iter()
        .map(|(name, value)| {
            let guard = value.borrow();
            let bytes = guard.as_ref()?;
            Some((
                name.clone(),
                StructuredInfoValue::from_canonical_bytes(bytes).unwrap(),
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
        preparation: *preparation,
        execution,
        nodes: *nodes,
        cords: *cords,
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
