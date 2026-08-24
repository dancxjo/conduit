//! Fixed-capacity production-kernel execution for portable `state/select`.

use alloc::vec::Vec;
use conduit_core::{InfoBool, SCALAR_ENCODED_LEN, Scalar};
use conduit_kernel::scheduler::{
    CordSpec, FixedScheduler, HostOperationRequest, OperationDriver, SchedulerStatus,
};
use conduit_kernel::{
    FixedHostOperationBindings, FixedRoutes, FixedSignLog, FixedValueStore,
    HostOperationDisposition, HostOperationOutcome, NodeId, SignSink, ValueStorage,
};
use conduit_runtime::lowering::{MAXIMUM_KERNEL_PORTS_PER_NODE, lower_plan_fragment};

use super::{
    state_select_operation::StateSelectOperation,
    state_select_plan::{
        FALSE_SOURCE_KIND, PreparedStateSelect, SELECTOR_SOURCE_KIND, SINK_KIND, TRUE_SOURCE_KIND,
    },
};

const PORTS: usize = MAXIMUM_KERNEL_PORTS_PER_NODE;
const NODES: usize = 5;
const CORDS: usize = 4;
const ROUTES: usize = NODES * PORTS;
const HOST_BINDINGS: usize = NODES * NODES;
const VALUES: usize = 12;
const MAX_VALUE_BYTES: usize = SCALAR_ENCODED_LEN;
const VALUE_BYTES: usize = VALUES * MAX_VALUE_BYTES;
const SIGNS: usize = 256;
const MAXIMUM_OUTPUTS: usize = 4;

type Kernel = FixedScheduler<
    OperationDriver<StateSelectOperation, PORTS>,
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
pub enum StateSelectError {
    Catalog,
    Form,
    Placement,
    Plan,
    Lowering,
    Shape,
    Kernel,
    KernelDetail(conduit_kernel::scheduler::SchedulerError),
    KernelSetup(u8),
    Value,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateSelectProof {
    pub plan_id: conduit_core::PlanId,
    pub outputs: [Option<Scalar>; MAXIMUM_OUTPUTS],
    pub output_count: usize,
    pub decisions: u32,
    pub maximum_cord_items: u16,
    pub kernel_signs: u16,
}

struct Scheduler {
    kernel: Kernel,
    sink: NodeId,
}

pub fn run_state_select(
    prepared: &PreparedStateSelect,
) -> Result<StateSelectProof, StateSelectError> {
    let fragment = prepared
        .plan
        .fragments
        .first()
        .ok_or(StateSelectError::Shape)?;
    let lowered = lower_plan_fragment(fragment).map_err(|_| StateSelectError::Lowering)?;
    if lowered.nodes.len() != NODES
        || lowered.cords.len() != CORDS
        || !lowered.remote_endpoints.is_empty()
    {
        return Err(StateSelectError::Shape);
    }
    let mut scheduler = scheduler(fragment, &lowered, prepared)?;
    let mut outputs = [None; MAXIMUM_OUTPUTS];
    let mut output_count = 0;
    let mut maximum_cord_items = 0;
    loop {
        for cord in 0..CORDS {
            let (items, _) = scheduler
                .kernel
                .cord_usage(conduit_kernel::CordId(cord as u16))
                .map_err(StateSelectError::KernelDetail)?;
            maximum_cord_items = maximum_cord_items.max(items);
        }
        if let Some(request) = scheduler.kernel.next_host_request() {
            capture(&mut scheduler, request, &mut outputs, &mut output_count)?;
            continue;
        }
        match scheduler
            .kernel
            .step()
            .map_err(StateSelectError::KernelDetail)?
        {
            SchedulerStatus::Progress { .. } => {}
            SchedulerStatus::Complete => break,
            SchedulerStatus::Idle | SchedulerStatus::Cancelled => {
                return Err(StateSelectError::Kernel);
            }
        }
    }
    Ok(StateSelectProof {
        plan_id: prepared.plan.plan_id.clone(),
        outputs,
        output_count,
        decisions: scheduler.kernel.decisions(),
        maximum_cord_items,
        kernel_signs: scheduler.kernel.signs().len(),
    })
}

#[cfg(test)]
pub(super) fn cancel_state_select(
    prepared: &PreparedStateSelect,
) -> Result<[bool; 2], StateSelectError> {
    let fragment = prepared
        .plan
        .fragments
        .first()
        .ok_or(StateSelectError::Shape)?;
    let lowered = lower_plan_fragment(fragment).map_err(|_| StateSelectError::Lowering)?;
    let mut scheduler = scheduler(fragment, &lowered, prepared)?;
    scheduler
        .kernel
        .cancel()
        .map_err(StateSelectError::KernelDetail)?;
    if scheduler
        .kernel
        .step()
        .map_err(StateSelectError::KernelDetail)?
        != SchedulerStatus::Cancelled
    {
        return Err(StateSelectError::Kernel);
    }
    let mut requested = false;
    let mut terminal = false;
    for event in scheduler.kernel.signs().events() {
        requested |= event.kind == conduit_kernel::KernelEventKind::CancellationRequested;
        terminal |= event.kind == conduit_kernel::KernelEventKind::RunCancelled;
    }
    Ok([requested, terminal])
}

fn capture(
    scheduler: &mut Scheduler,
    request: HostOperationRequest,
    outputs: &mut [Option<Scalar>; MAXIMUM_OUTPUTS],
    output_count: &mut usize,
) -> Result<(), StateSelectError> {
    if request.node != scheduler.sink {
        return scheduler
            .kernel
            .complete_host_operation(
                request.node,
                request.request,
                HostOperationOutcome {
                    disposition: HostOperationDisposition::Completed,
                    output: None,
                    failure: None,
                },
            )
            .map_err(StateSelectError::KernelDetail);
    }
    if *output_count >= outputs.len() {
        return Err(StateSelectError::Shape);
    }
    outputs[*output_count] = Some(
        Scalar::decode(
            scheduler
                .kernel
                .host_value(request.input.value)
                .map_err(|_| StateSelectError::Value)?,
        )
        .map_err(|_| StateSelectError::Value)?,
    );
    *output_count += 1;
    scheduler
        .kernel
        .complete_host_operation(
            request.node,
            request.request,
            HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: None,
                failure: None,
            },
        )
        .map_err(StateSelectError::KernelDetail)
}

fn scheduler(
    fragment: &conduit_core::PlanFragment,
    lowered: &conduit_runtime::lowering::LoweredPlanFragment,
    prepared: &PreparedStateSelect,
) -> Result<Scheduler, StateSelectError> {
    let nodes = lowered
        .node_specs
        .as_slice()
        .try_into()
        .map_err(|_| StateSelectError::Shape)?;
    let cords: [CordSpec; CORDS] = lowered
        .cords
        .iter()
        .map(|cord| cord.spec)
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| StateSelectError::Shape)?;
    let mut routes = FixedRoutes::<ROUTES, CORDS>::new(PORTS as u16);
    for route in &lowered.routes {
        routes
            .install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )
            .map_err(|_| StateSelectError::KernelSetup(1))?;
    }
    routes
        .seal()
        .map_err(|_| StateSelectError::KernelSetup(2))?;
    let mut bindings = FixedHostOperationBindings::<HOST_BINDINGS>::new(NODES as u16);
    for operation in &lowered.host_operations {
        bindings
            .install(operation.node, operation.binding)
            .map_err(|_| StateSelectError::KernelSetup(3))?;
    }
    bindings
        .seal()
        .map_err(|_| StateSelectError::KernelSetup(4))?;
    let mut values = FixedValueStore::<VALUES, MAX_VALUE_BYTES>::new(VALUE_BYTES as u32)
        .map_err(|_| StateSelectError::Value)?;
    let selector_values = store_bool_sequence(&mut values, prepared.sequence.selectors)?;
    let false_values = store_scalar_sequence(&mut values, prepared.sequence.when_false)?;
    let true_values = store_scalar_sequence(&mut values, prepared.sequence.when_true)?;
    let mut sink = None;
    let drivers = fragment
        .placements
        .iter()
        .enumerate()
        .map(|(index, placement)| {
            let operation = match placement.kind_id.as_str() {
                SELECTOR_SOURCE_KIND => StateSelectOperation::Source {
                    values: selector_values,
                    phase: 0,
                },
                FALSE_SOURCE_KIND => StateSelectOperation::Source {
                    values: false_values,
                    phase: 0,
                },
                TRUE_SOURCE_KIND => StateSelectOperation::Source {
                    values: true_values,
                    phase: 0,
                },
                conduit_std_catalog::STATE_SELECT_KIND => StateSelectOperation::Select {
                    selector: None,
                    candidates: [None; 2],
                    closed: [false; 3],
                },
                SINK_KIND => {
                    sink = Some(NodeId(index as u16));
                    StateSelectOperation::Sink {
                        pending: false,
                        next_request: 0,
                    }
                }
                _ => return Err(StateSelectError::Shape),
            };
            OperationDriver::new(operation).map_err(|_| StateSelectError::KernelSetup(5))
        })
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| StateSelectError::Shape)?;
    let signs = FixedSignLog::<SIGNS>::new(
        (SIGNS * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32,
    )
    .map_err(|_| StateSelectError::KernelSetup(6))?;
    let kernel = FixedScheduler::new_with_host_operations(
        nodes, cords, routes, bindings, drivers, values, signs,
    )
    .map_err(|_| StateSelectError::KernelSetup(7))?;
    Ok(Scheduler {
        kernel,
        sink: sink.ok_or(StateSelectError::Shape)?,
    })
}

fn store_bool_sequence(
    values: &mut FixedValueStore<VALUES, MAX_VALUE_BYTES>,
    sequence: [Option<InfoBool>; 2],
) -> Result<[Option<conduit_kernel::ValueRef>; 2], StateSelectError> {
    let [first, second] = sequence.map(|value| {
        value
            .map(|value| {
                values
                    .store(&value.encode())
                    .map_err(|_| StateSelectError::Value)
            })
            .transpose()
    });
    Ok([first?, second?])
}

fn store_scalar_sequence(
    values: &mut FixedValueStore<VALUES, MAX_VALUE_BYTES>,
    sequence: [Option<Scalar>; 2],
) -> Result<[Option<conduit_kernel::ValueRef>; 2], StateSelectError> {
    let [first, second] = sequence.map(|value| {
        value
            .map(|value| {
                values
                    .store(&value.encode())
                    .map_err(|_| StateSelectError::Value)
            })
            .transpose()
    });
    Ok([first?, second?])
}
