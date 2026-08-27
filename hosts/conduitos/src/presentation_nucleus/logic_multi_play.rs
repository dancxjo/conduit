//! Bounded production-kernel execution for `logic/compare > logic/select`.

use alloc::vec::Vec;
use conduit_core::{InfoBool, Scalar};
use conduit_kernel::scheduler::{
    CordSpec, FixedScheduler, HostOperationRequest, OperationDriver, SchedulerStatus,
};
use conduit_kernel::{
    BoundedValueRef, FixedHostOperationBindings, FixedRoutes, FixedSignLog, FixedValueStore,
    HostOperationDisposition, HostOperationOutcome, NodeId, ValueStorage,
};
use conduit_plan_lowering::lowering::{FIXED_KERNEL_STORAGE_PORTS_PER_NODE, lower_plan_fragment};

use super::{
    logic_multi_plan::{
        FALSE_KIND, LEFT_KIND, PreparedLogicMulti, RIGHT_KIND, SINK_KIND, TRUE_KIND,
    },
    operation::PresentationOperation,
};

const PORTS: usize = FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const NODES: usize = 7;
const CORDS: usize = 6;
const ROUTES: usize = NODES * PORTS;
const HOST_BINDINGS: usize = NODES * NODES;
const VALUES: usize = 8;
const MAX_VALUE_BYTES: usize = conduit_core::SCALAR_ENCODED_LEN;
const VALUE_BYTES: usize = VALUES * MAX_VALUE_BYTES;
const SIGNS: usize = 96;

type Kernel = FixedScheduler<
    OperationDriver<PresentationOperation, PORTS>,
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
    Form,
    Placement,
    Plan,
    Lowering,
    Shape,
    Kernel,
    Value,
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

#[derive(Default)]
struct HostState {
    compare: [Option<Scalar>; 2],
    selector: Option<InfoBool>,
    candidates: [Option<Scalar>; 2],
    decision: Option<InfoBool>,
    captured: Option<Scalar>,
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
    loop {
        if let Some(request) = scheduler.kernel.next_host_request() {
            service(&mut scheduler, request, &mut state, prepared.comparison)?;
            continue;
        }
        match scheduler
            .kernel
            .step()
            .map_err(|_| LogicMultiError::Kernel)?
        {
            SchedulerStatus::Progress { .. } => {}
            SchedulerStatus::Complete => break,
            SchedulerStatus::Idle | SchedulerStatus::Cancelled => {
                return Err(LogicMultiError::Kernel);
            }
        }
    }
    let decision = state.decision.ok_or(LogicMultiError::Shape)?;
    let output = state.captured.ok_or(LogicMultiError::Shape)?;
    let expected =
        conduit_std_catalog::select_scalar(decision.get(), prepared.when_false, prepared.when_true);
    if output != expected {
        return Err(LogicMultiError::Shape);
    }
    Ok(LogicMultiProof {
        plan_id: prepared.plan.plan_id.clone(),
        decision,
        output,
    })
}

fn service(
    scheduler: &mut Scheduler,
    request: HostOperationRequest,
    state: &mut HostState,
    comparison: conduit_std_catalog::ScalarComparison,
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
        state.compare[index] = Some(Scalar::decode(bytes).map_err(|_| LogicMultiError::Value)?);
        let output = match state.compare {
            [Some(left), Some(right)] => {
                let decision = InfoBool::new(comparison.evaluate(left, right));
                state.decision = Some(decision);
                Some(decision.encode().to_vec())
            }
            _ => None,
        };
        complete(&mut scheduler.kernel, request, output.as_deref())
    } else if request.node == scheduler.select {
        match request.request.0 % 4 {
            0 if state.selector.is_none() => {
                state.selector = Some(InfoBool::decode(bytes).map_err(|_| LogicMultiError::Value)?);
            }
            port @ (1 | 2) if state.candidates[(port - 1) as usize].is_none() => {
                state.candidates[(port - 1) as usize] =
                    Some(Scalar::decode(bytes).map_err(|_| LogicMultiError::Value)?);
            }
            _ => return Err(LogicMultiError::Shape),
        }
        let output = match (state.selector, state.candidates) {
            (Some(selector), [Some(when_false), Some(when_true)]) => Some(
                conduit_std_catalog::select_scalar(selector.get(), when_false, when_true).encode(),
            ),
            _ => None,
        };
        complete(
            &mut scheduler.kernel,
            request,
            output.as_ref().map(|bytes| bytes.as_slice()),
        )
    } else if request.node == scheduler.sink && state.captured.is_none() {
        state.captured = Some(Scalar::decode(bytes).map_err(|_| LogicMultiError::Value)?);
        complete(&mut scheduler.kernel, request, None)
    } else {
        Err(LogicMultiError::Shape)
    }
}

fn complete(
    kernel: &mut Kernel,
    request: HostOperationRequest,
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
        .complete_host_operation(
            request.node,
            request.request,
            HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
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
    let mut bindings = FixedHostOperationBindings::<HOST_BINDINGS>::new(NODES as u16);
    for operation in &lowered.host_operations {
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
                LEFT_KIND => source(&mut values, prepared.left)?,
                RIGHT_KIND => source(&mut values, prepared.right)?,
                FALSE_KIND => source(&mut values, prepared.when_false)?,
                TRUE_KIND => source(&mut values, prepared.when_true)?,
                conduit_std_catalog::LOGIC_COMPARE_KIND => {
                    compare = Some(NodeId(index as u16));
                    PresentationOperation::LogicInputs {
                        input_count: 2,
                        seen: 0,
                        next_request: 0,
                        pending: false,
                        emitted: false,
                    }
                }
                conduit_std_catalog::LOGIC_SELECT_KIND => {
                    select = Some(NodeId(index as u16));
                    PresentationOperation::LogicInputs {
                        input_count: 3,
                        seen: 0,
                        next_request: 0,
                        pending: false,
                        emitted: false,
                    }
                }
                SINK_KIND => {
                    sink = Some(NodeId(index as u16));
                    PresentationOperation::Sink {
                        maximum_input_bytes: conduit_core::SCALAR_ENCODED_LEN as u32,
                        pending: false,
                        complete: false,
                    }
                }
                _ => return Err(LogicMultiError::Shape),
            };
            OperationDriver::new(operation).map_err(|_| LogicMultiError::Kernel)
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
    let kernel = FixedScheduler::new_with_host_operations(
        nodes, cords, routes, bindings, drivers, values, signs,
    )
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
) -> Result<PresentationOperation, LogicMultiError> {
    Ok(PresentationOperation::Source {
        value: values
            .store(&value.encode())
            .map_err(|_| LogicMultiError::Value)?,
        emitted: false,
    })
}
