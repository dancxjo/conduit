//! Bounded production-kernel execution for `logic/compare > logic/select`.

use alloc::vec::Vec;
use conduit_core::{InfoBool, Scalar};
use conduit_kernel::scheduler::{CordSpec, FixedScheduler, HostCallRequest, SchedulerStatus};
use conduit_kernel::{
    BoundedValueRef, FixedHostCallBindings, FixedRoutes, FixedSignLog, FixedValueStore,
    HostCallDisposition, HostCallOutcome, NodeId, ValueStorage,
};
use conduit_plan_lowering::lowering::{FIXED_KERNEL_STORAGE_PORTS_PER_NODE, lower_plan_fragment};

use super::{
    back::PresentationBack,
    logic_multi_plan::{
        FALSE_KIND, LEFT_KIND, PreparedLogicMulti, RIGHT_KIND, SINK_KIND, TRUE_KIND,
    },
};

const PORTS: usize = FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const NODES: usize = 7;
const CORDS: usize = 6;
const ROUTES: usize = NODES * PORTS;
const HOST_BINDINGS: usize = NODES * NODES;
const VALUES: usize = 8;
const MAX_VALUE_BYTES: usize = 128;
const VALUE_BYTES: usize = VALUES * MAX_VALUE_BYTES;
const SIGNS: usize = 96;

type Kernel = FixedScheduler<
    PresentationBack,
    FixedValueStore<VALUES, MAX_VALUE_BYTES>,
    FixedSignLog<SIGNS>,
    NODES,
    CORDS,
    PORTS,
    CORDS,
    ROUTES,
    CORDS,
    HOST_BINDINGS,
    NODES,
>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LogicMultiError {
    Catalog,
    Plot,
    Placement,
    Plan,
    Lowering,
    Shape,
    Kernel,
    Value,
    Numerical(conduit_core::NumericRefusal),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogicMultiProof {
    pub plan_id: conduit_core::PlanId,
    pub decision: InfoBool,
    pub output: Scalar,
}

struct Scheduler {
    kernel: Kernel,
    compare: NodeId,
    select: NodeId,
    sink: NodeId,
}

#[derive(Clone, Copy)]
struct ScalarBytes {
    bytes: [u8; MAX_VALUE_BYTES],
    length: usize,
}
impl ScalarBytes {
    fn new(value: &[u8]) -> Result<Self, LogicMultiError> {
        if value.len() > MAX_VALUE_BYTES {
            return Err(LogicMultiError::Value);
        }
        conduit_core::MathValueRef::validate(value).map_err(|_| LogicMultiError::Value)?;
        let mut bytes = [0; MAX_VALUE_BYTES];
        bytes[..value.len()].copy_from_slice(value);
        Ok(Self {
            bytes,
            length: value.len(),
        })
    }
    fn encoded(&self) -> &[u8] {
        &self.bytes[..self.length]
    }
}
#[derive(Default)]
struct HostState {
    compare: [Option<ScalarBytes>; 2],
    selector: Option<InfoBool>,
    candidates: [Option<ScalarBytes>; 2],
    decision: Option<InfoBool>,
    refusal: Option<conduit_core::NumericRefusal>,
    captured: Option<ScalarBytes>,
}

pub fn run_logic_multi(prepared: &PreparedLogicMulti) -> Result<LogicMultiProof, LogicMultiError> {
    let fragment = prepared
        .plan
        .fragments
        .first()
        .ok_or(LogicMultiError::Shape)?;
    let lowered = lower_plan_fragment(fragment).map_err(|_| LogicMultiError::Lowering)?;
    if lowered.nodes.len() != NODES
        || lowered.cords.len() != CORDS
        || !lowered.remote_endpoints.is_empty()
    {
        return Err(LogicMultiError::Shape);
    }
    let mut scheduler = scheduler(fragment, &lowered, prepared)?;
    let mut state = HostState::default();
    #[cfg(test)]
    {
        let mut result = Ok(());
        let allocations = crate::test_allocations::allocations(|| {
            result = execute(&mut scheduler, &mut state, prepared.comparison);
        });
        assert_eq!(
            allocations, 0,
            "prepared exact logic kernel loop allocates no memory"
        );
        result?;
    }
    #[cfg(not(test))]
    execute(&mut scheduler, &mut state, prepared.comparison)?;
    if let Some(refusal) = state.refusal {
        return Err(LogicMultiError::Numerical(refusal));
    }
    let decision = state.decision.ok_or(LogicMultiError::Shape)?;
    let captured = state.captured.ok_or(LogicMultiError::Shape)?;
    let output = Scalar::decode(captured.encoded()).map_err(|_| LogicMultiError::Value)?;
    let expected = conduit_semantic_catalog::select_scalar(
        decision.get(),
        prepared.when_false.clone(),
        prepared.when_true.clone(),
    );
    if output != expected {
        return Err(LogicMultiError::Shape);
    }
    Ok(LogicMultiProof {
        plan_id: prepared.plan.plan_id.clone(),
        decision,
        output,
    })
}

fn execute(
    scheduler: &mut Scheduler,
    state: &mut HostState,
    comparison: conduit_semantic_catalog::ScalarComparison,
) -> Result<(), LogicMultiError> {
    loop {
        if let Some(request) = scheduler.kernel.next_host_request() {
            service(scheduler, request, state, comparison)?;
            continue;
        }
        match scheduler
            .kernel
            .step()
            .map_err(|error| match (error, state.refusal) {
                (
                    conduit_kernel::scheduler::SchedulerError::SemanticAbnormal {
                        node,
                        port,
                        terminal,
                    },
                    Some(refusal),
                ) if node == scheduler.compare && port == conduit_kernel::PortId(0) => {
                    let _ = terminal;
                    LogicMultiError::Numerical(refusal)
                }
                _ => LogicMultiError::Kernel,
            })? {
            SchedulerStatus::Progress { .. } => {}
            SchedulerStatus::Drained => break,
            SchedulerStatus::Idle | SchedulerStatus::Cancelled => {
                return Err(LogicMultiError::Kernel);
            }
        }
    }
    Ok(())
}

fn service(
    scheduler: &mut Scheduler,
    request: HostCallRequest,
    state: &mut HostState,
    comparison: conduit_semantic_catalog::ScalarComparison,
) -> Result<(), LogicMultiError> {
    let bytes = scheduler
        .kernel
        .host_value(request.input.value)
        .map_err(|_| LogicMultiError::Value)?;
    if request.node == scheduler.compare {
        let index = (request.request.0 % 4) as usize;
        if index >= 2 || state.compare[index].is_some() {
            return Err(LogicMultiError::Shape);
        }
        state.compare[index] = Some(ScalarBytes::new(bytes)?);
        let output = match state.compare {
            [Some(left), Some(right)] => {
                let relation = conduit_core::MathValueRef::validate(left.encoded())
                    .and_then(|left| {
                        conduit_core::MathValueRef::validate(right.encoded())
                            .and_then(|right| left.compare(right))
                    })
                    .and_then(|relation| comparison.evaluate_order(relation));
                let decision = match relation {
                    Ok(decision) => InfoBool::new(decision),
                    Err(error) => {
                        state.refusal = Some(error);
                        scheduler
                            .kernel
                            .complete_host_call(
                                request.node,
                                request.request,
                                HostCallOutcome {
                                    disposition: HostCallDisposition::Failed,
                                    output: None,
                                    failure: Some(conduit_kernel::Failure {
                                        code: conduit_kernel::FailureCode::HostCallFailed,
                                        detail: refusal_index(error),
                                    }),
                                },
                            )
                            .map_err(|_| LogicMultiError::Kernel)?;
                        return Ok(());
                    }
                };
                state.decision = Some(decision);
                Some(decision.encode())
            }
            _ => None,
        };
        complete(
            &mut scheduler.kernel,
            request,
            output.as_ref().map(|bytes| bytes.as_slice()),
        )
    } else if request.node == scheduler.select {
        match request.request.0 % 4 {
            0 if state.selector.is_none() => {
                state.selector = Some(InfoBool::decode(bytes).map_err(|_| LogicMultiError::Value)?);
            }
            port @ (1 | 2) if state.candidates[(port - 1) as usize].is_none() => {
                state.candidates[(port - 1) as usize] = Some(ScalarBytes::new(bytes)?);
            }
            _ => return Err(LogicMultiError::Shape),
        }
        let output = match (state.selector, state.candidates) {
            (Some(selector), [Some(when_false), Some(when_true)]) => Some(if selector.get() {
                when_true
            } else {
                when_false
            }),
            _ => None,
        };
        complete(
            &mut scheduler.kernel,
            request,
            output.as_ref().map(ScalarBytes::encoded),
        )
    } else if request.node == scheduler.sink && state.captured.is_none() {
        state.captured = Some(ScalarBytes::new(bytes)?);
        complete(&mut scheduler.kernel, request, None)
    } else {
        Err(LogicMultiError::Shape)
    }
}

fn complete(
    kernel: &mut Kernel,
    request: HostCallRequest,
    output: Option<&[u8]>,
) -> Result<(), LogicMultiError> {
    let output = output
        .map(|bytes| {
            let value = kernel
                .store_host_value(bytes)
                .map_err(|_| LogicMultiError::Value)?;
            BoundedValueRef::new(value, bytes.len() as u32).map_err(|_| LogicMultiError::Value)
        })
        .transpose()?;
    kernel
        .complete_host_call(
            request.node,
            request.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output,
                failure: None,
            },
        )
        .map_err(|_| LogicMultiError::Kernel)
}

fn scheduler(
    fragment: &conduit_core::PlanFragment,
    lowered: &conduit_plan_lowering::lowering::LoweredPlanFragment,
    prepared: &PreparedLogicMulti,
) -> Result<Scheduler, LogicMultiError> {
    let nodes = lowered
        .node_specs
        .as_slice()
        .try_into()
        .map_err(|_| LogicMultiError::Shape)?;
    let cords: [CordSpec; CORDS] = lowered
        .cords
        .iter()
        .map(|cord| cord.spec)
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| LogicMultiError::Shape)?;
    let mut routes = FixedRoutes::<ROUTES, CORDS>::new(PORTS as u16);
    for route in &lowered.routes {
        routes
            .install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )
            .map_err(|_| LogicMultiError::Kernel)?;
    }
    routes.seal().map_err(|_| LogicMultiError::Kernel)?;
    let mut bindings = FixedHostCallBindings::<HOST_BINDINGS>::new(NODES as u16);
    for operation in &lowered.host_calls {
        bindings
            .install(operation.node, operation.binding)
            .map_err(|_| LogicMultiError::Kernel)?;
    }
    bindings.seal().map_err(|_| LogicMultiError::Kernel)?;
    let mut values = FixedValueStore::<VALUES, MAX_VALUE_BYTES>::new(VALUE_BYTES as u32)
        .map_err(|_| LogicMultiError::Value)?;
    let mut compare = None;
    let mut select = None;
    let mut sink = None;
    let drivers = fragment
        .placements
        .iter()
        .enumerate()
        .map(|(index, placement)| {
            let operation = match placement.kind_id.as_str() {
                LEFT_KIND => source(&mut values, prepared.left.clone())?,
                RIGHT_KIND => source(&mut values, prepared.right.clone())?,
                FALSE_KIND => source(&mut values, prepared.when_false.clone())?,
                TRUE_KIND => source(&mut values, prepared.when_true.clone())?,
                conduit_semantic_catalog::LOGIC_COMPARE_KIND => {
                    compare = Some(NodeId(index as u16));
                    PresentationBack::LogicInputs {
                        input_count: 2,
                        refusals: Some([
                            conduit_kernel::CanonicalValue::new(
                                &conduit_semantic_catalog::scalar_comparison_refusal_bytes(
                                    conduit_core::NumericRefusal::IndeterminateDomain,
                                ),
                            )
                            .map_err(|_| LogicMultiError::Value)?,
                            conduit_kernel::CanonicalValue::new(
                                &conduit_semantic_catalog::scalar_comparison_refusal_bytes(
                                    conduit_core::NumericRefusal::WorkBudget,
                                ),
                            )
                            .map_err(|_| LogicMultiError::Value)?,
                            conduit_kernel::CanonicalValue::new(
                                &conduit_semantic_catalog::scalar_comparison_refusal_bytes(
                                    conduit_core::NumericRefusal::Domain,
                                )
                                .as_slice(),
                            )
                            .map_err(|_| LogicMultiError::Value)?,
                        ]),
                        seen: 0,
                        next_request: 0,
                        pending: false,
                        emitted: false,
                    }
                }
                conduit_semantic_catalog::LOGIC_SELECT_KIND => {
                    select = Some(NodeId(index as u16));
                    PresentationBack::LogicInputs {
                        input_count: 3,
                        refusals: None,
                        seen: 0,
                        next_request: 0,
                        pending: false,
                        emitted: false,
                    }
                }
                SINK_KIND => {
                    sink = Some(NodeId(index as u16));
                    PresentationBack::Sink {
                        maximum_input_bytes: 128 as u32,
                        pending: false,
                        complete: false,
                    }
                }
                _ => return Err(LogicMultiError::Shape),
            };
            Ok(operation)
        })
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| LogicMultiError::Shape)?;
    let signs = FixedSignLog::<SIGNS>::new(
        lowered
            .sign_bytes
            .max((SIGNS * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32),
    )
    .map_err(|_| LogicMultiError::Kernel)?;
    let kernel =
        FixedScheduler::new_with_host_calls(nodes, cords, routes, bindings, drivers, values, signs)
            .map_err(|_| LogicMultiError::Kernel)?;
    Ok(Scheduler {
        kernel,
        compare: compare.ok_or(LogicMultiError::Shape)?,
        select: select.ok_or(LogicMultiError::Shape)?,
        sink: sink.ok_or(LogicMultiError::Shape)?,
    })
}

fn source(
    values: &mut FixedValueStore<VALUES, MAX_VALUE_BYTES>,
    value: Scalar,
) -> Result<PresentationBack, LogicMultiError> {
    Ok(PresentationBack::Source {
        value: values
            .store(
                &value
                    .canonical_bytes()
                    .map_err(|_| LogicMultiError::Value)?,
            )
            .map_err(|_| LogicMultiError::Value)?,
        emitted: false,
    })
}

fn refusal_index(error: conduit_core::NumericRefusal) -> u16 {
    use conduit_core::NumericRefusal;
    match error {
        NumericRefusal::IndeterminateDomain => 0,
        NumericRefusal::IntegerBudget
        | NumericRefusal::NodeBudget
        | NumericRefusal::DepthBudget
        | NumericRefusal::StorageBudget
        | NumericRefusal::WorkBudget => 1,
        _ => 2,
    }
}
