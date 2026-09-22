//! Bounded production-kernel execution for `state/latest > flow/tee`.

use alloc::vec::Vec;
use conduit_core::Scalar;
use conduit_kernel::scheduler::{CordSpec, FixedScheduler, HostCallRequest, SchedulerStatus};
use conduit_kernel::{
    FixedHostCallBindings, FixedRoutes, FixedSignLog, FixedValueStore, HostCallDisposition,
    HostCallOutcome, NodeId, ValueStorage,
};
use conduit_plan_lowering::lowering::{FIXED_KERNEL_STORAGE_PORTS_PER_NODE, lower_plan_fragment};

use super::{
    flow_state_operation::FlowStateBack,
    flow_state_plan::{LEFT_SINK_KIND, PreparedFlowState, RIGHT_SINK_KIND, SOURCE_KIND},
};

const PORTS: usize = FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const NODES: usize = 5;
const CORDS: usize = 4;
const ROUTES: usize = NODES * PORTS;
const HOST_BINDINGS: usize = NODES * NODES;
const VALUES: usize = 4;
const MAX_VALUE_BYTES: usize = conduit_core::SCALAR_ENCODED_LEN;
const VALUE_BYTES: usize = VALUES * MAX_VALUE_BYTES;
const SIGNS: usize = 96;

type Kernel = FixedScheduler<
    FlowStateBack,
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
pub enum FlowStateError {
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
pub struct FlowStateProof {
    pub plan_id: conduit_core::PlanId,
    pub left: Scalar,
    pub right: Scalar,
}

struct Scheduler {
    kernel: Kernel,
    left: NodeId,
    right: NodeId,
}

pub fn run_flow_state(prepared: &PreparedFlowState) -> Result<FlowStateProof, FlowStateError> {
    let fragment = prepared
        .plan
        .fragments
        .first()
        .ok_or(FlowStateError::Shape)?;
    let lowered = lower_plan_fragment(fragment).map_err(|_| FlowStateError::Lowering)?;
    if lowered.nodes.len() != NODES
        || lowered.cords.len() != CORDS
        || !lowered.remote_endpoints.is_empty()
    {
        return Err(FlowStateError::Shape);
    }
    let mut scheduler = scheduler(fragment, &lowered, prepared.value)?;
    let mut left = None;
    let mut right = None;
    loop {
        if let Some(request) = scheduler.kernel.next_host_request() {
            capture(&mut scheduler, request, &mut left, &mut right)?;
            continue;
        }
        match scheduler
            .kernel
            .step()
            .map_err(|_| FlowStateError::Kernel)?
        {
            SchedulerStatus::Progress { .. } => {}
            SchedulerStatus::Drained => break,
            SchedulerStatus::Idle | SchedulerStatus::Cancelled => {
                return Err(FlowStateError::Kernel);
            }
        }
    }
    let (Some(left), Some(right)) = (left, right) else {
        return Err(FlowStateError::Shape);
    };
    if left != prepared.value || right != prepared.value {
        return Err(FlowStateError::Shape);
    }
    Ok(FlowStateProof {
        plan_id: prepared.plan.plan_id.clone(),
        left,
        right,
    })
}

fn capture(
    scheduler: &mut Scheduler,
    request: HostCallRequest,
    left: &mut Option<Scalar>,
    right: &mut Option<Scalar>,
) -> Result<(), FlowStateError> {
    let value = Scalar::decode(
        scheduler
            .kernel
            .host_value(request.input.value)
            .map_err(|_| FlowStateError::Value)?,
    )
    .map_err(|_| FlowStateError::Value)?;
    let target = if request.node == scheduler.left {
        left
    } else if request.node == scheduler.right {
        right
    } else {
        return Err(FlowStateError::Shape);
    };
    if target.replace(value).is_some() {
        return Err(FlowStateError::Shape);
    }
    scheduler
        .kernel
        .complete_host_call(
            request.node,
            request.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: None,
                failure: None,
            },
        )
        .map_err(|_| FlowStateError::Kernel)
}

fn scheduler(
    fragment: &conduit_core::PlanFragment,
    lowered: &conduit_plan_lowering::lowering::LoweredPlanFragment,
    value: Scalar,
) -> Result<Scheduler, FlowStateError> {
    let nodes = lowered
        .node_specs
        .as_slice()
        .try_into()
        .map_err(|_| FlowStateError::Shape)?;
    let cords: [CordSpec; CORDS] = lowered
        .cords
        .iter()
        .map(|cord| cord.spec)
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| FlowStateError::Shape)?;
    let mut routes = FixedRoutes::<ROUTES, CORDS>::new(PORTS as u16);
    for route in &lowered.routes {
        routes
            .install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )
            .map_err(|_| FlowStateError::Kernel)?;
    }
    routes.seal().map_err(|_| FlowStateError::Kernel)?;
    let mut bindings = FixedHostCallBindings::<HOST_BINDINGS>::new(NODES as u16);
    for operation in &lowered.host_calls {
        bindings
            .install(operation.node, operation.binding)
            .map_err(|_| FlowStateError::Kernel)?;
    }
    bindings.seal().map_err(|_| FlowStateError::Kernel)?;
    let mut values = FixedValueStore::<VALUES, MAX_VALUE_BYTES>::new(VALUE_BYTES as u32)
        .map_err(|_| FlowStateError::Value)?;
    let mut left = None;
    let mut right = None;
    let backs = fragment
        .placements
        .iter()
        .enumerate()
        .map(|(index, placement)| match placement.kind_id.as_str() {
            SOURCE_KIND => Ok(FlowStateBack::Source {
                value: values
                    .store(&value.encode())
                    .map_err(|_| FlowStateError::Value)?,
                emitted: false,
            }),
            conduit_semantic_catalog::LATEST_KIND => Ok(FlowStateBack::Latest { held: None }),
            conduit_semantic_catalog::TEE_KIND => Ok(FlowStateBack::Tee),
            LEFT_SINK_KIND | RIGHT_SINK_KIND => {
                if placement.kind_id.as_str() == LEFT_SINK_KIND {
                    left = Some(NodeId(index as u16));
                } else {
                    right = Some(NodeId(index as u16));
                }
                Ok(FlowStateBack::Sink {
                    pending: false,
                    complete: false,
                })
            }
            _ => Err(FlowStateError::Shape),
        })
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| FlowStateError::Shape)?;
    let signs = FixedSignLog::<SIGNS>::new(
        (SIGNS * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32,
    )
    .map_err(|_| FlowStateError::Kernel)?;
    let kernel =
        FixedScheduler::new_with_host_calls(nodes, cords, routes, bindings, backs, values, signs)
            .map_err(|_| FlowStateError::Kernel)?;
    Ok(Scheduler {
        kernel,
        left: left.ok_or(FlowStateError::Shape)?,
        right: right.ok_or(FlowStateError::Shape)?,
    })
}
