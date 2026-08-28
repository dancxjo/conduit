//! Production fixed-scheduler execution for portable state and typed key fan-out.

use alloc::vec::Vec;
use conduit_core::InfoBool;
use conduit_human::KeyEvent;
use conduit_kernel::scheduler::{
    CordSpec, FixedScheduler, HostOperationRequest, OperationDriver, SchedulerStatus,
};
use conduit_kernel::{
    FixedHostOperationBindings, FixedRoutes, FixedSignLog, FixedValueStore,
    HostOperationDisposition, HostOperationOutcome, NodeId, ValueStorage,
};
use conduit_plan_lowering::lowering::{FIXED_KERNEL_STORAGE_PORTS_PER_NODE, lower_plan_fragment};

use super::{
    portable_state_input_operation::PortableStateInputOperation,
    portable_state_input_plan::{
        BOOL_SINK_KIND, CHORD_KEY_SINK_KIND, COUNT_SINK_KIND, KEY_SOURCE_KIND,
        PreparedPortableStateInput, TEXT_KEY_SINK_KIND, TICK_SOURCE_KIND,
    },
};

const PORTS: usize = FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const NODES: usize = 10;
const CORDS: usize = 7;
const ROUTES: usize = NODES * PORTS;
const HOST_BINDINGS: usize = NODES * NODES;
const VALUES: usize = 9;
const MAX_VALUE_BYTES: usize = conduit_semantic_catalog::COUNT_ENCODED_LEN as usize;
const VALUE_BYTES: usize = 64;
const SIGNS: usize = 192;

type Kernel = FixedScheduler<
    OperationDriver<PortableStateInputOperation, PORTS>,
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
pub enum PortableStateInputError {
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
pub struct PortableStateInputProof {
    pub plan_id: conduit_core::PlanId,
    pub counts: [u64; 2],
    pub toggles: [bool; 2],
    pub text_key: KeyEvent,
    pub chord_key: KeyEvent,
}

struct Scheduler {
    kernel: Kernel,
    count_sink: NodeId,
    bool_sink: NodeId,
    text_sink: NodeId,
    chord_sink: NodeId,
}

pub fn run_portable_state_input(
    prepared: &PreparedPortableStateInput,
) -> Result<PortableStateInputProof, PortableStateInputError> {
    let fragment = prepared
        .plan
        .fragments
        .first()
        .ok_or(PortableStateInputError::Shape)?;
    let lowered = lower_plan_fragment(fragment).map_err(|_| PortableStateInputError::Lowering)?;
    if lowered.nodes.len() != NODES
        || lowered.cords.len() != CORDS
        || !lowered.remote_endpoints.is_empty()
    {
        return Err(PortableStateInputError::Shape);
    }
    let mut scheduler = scheduler(fragment, &lowered, prepared)?;
    let mut counts = [None; 2];
    let mut count_len = 0;
    let mut toggles = [None; 2];
    let mut toggle_len = 0;
    let mut text_key = None;
    let mut chord_key = None;
    loop {
        if let Some(request) = scheduler.kernel.next_host_request() {
            capture(
                &mut scheduler,
                request,
                &mut counts,
                &mut count_len,
                &mut toggles,
                &mut toggle_len,
                &mut text_key,
                &mut chord_key,
            )?;
            continue;
        }
        match scheduler
            .kernel
            .step()
            .map_err(|_| PortableStateInputError::Kernel)?
        {
            SchedulerStatus::Progress { .. } => {}
            SchedulerStatus::Complete => break,
            SchedulerStatus::Idle | SchedulerStatus::Cancelled => {
                return Err(PortableStateInputError::Kernel);
            }
        }
    }
    let counts = [
        counts[0].ok_or(PortableStateInputError::Shape)?,
        counts[1].ok_or(PortableStateInputError::Shape)?,
    ];
    let toggles = [
        toggles[0].ok_or(PortableStateInputError::Shape)?,
        toggles[1].ok_or(PortableStateInputError::Shape)?,
    ];
    Ok(PortableStateInputProof {
        plan_id: prepared.plan.plan_id.clone(),
        counts,
        toggles,
        text_key: text_key.ok_or(PortableStateInputError::Shape)?,
        chord_key: chord_key.ok_or(PortableStateInputError::Shape)?,
    })
}

#[allow(clippy::too_many_arguments)]
fn capture(
    scheduler: &mut Scheduler,
    request: HostOperationRequest,
    counts: &mut [Option<u64>; 2],
    count_len: &mut usize,
    toggles: &mut [Option<bool>; 2],
    toggle_len: &mut usize,
    text_key: &mut Option<KeyEvent>,
    chord_key: &mut Option<KeyEvent>,
) -> Result<(), PortableStateInputError> {
    let bytes = scheduler
        .kernel
        .host_value(request.input.value)
        .map_err(|_| PortableStateInputError::Value)?;
    if request.node == scheduler.count_sink && *count_len < counts.len() {
        let raw: [u8; 8] = bytes
            .try_into()
            .map_err(|_| PortableStateInputError::Value)?;
        counts[*count_len] = Some(u64::from_le_bytes(raw));
        *count_len += 1;
    } else if request.node == scheduler.bool_sink && *toggle_len < toggles.len() {
        toggles[*toggle_len] = Some(
            InfoBool::decode(bytes)
                .map_err(|_| PortableStateInputError::Value)?
                .get(),
        );
        *toggle_len += 1;
    } else if request.node == scheduler.text_sink && text_key.is_none() {
        *text_key = Some(KeyEvent::decode(bytes).map_err(|_| PortableStateInputError::Value)?);
    } else if request.node == scheduler.chord_sink && chord_key.is_none() {
        *chord_key = Some(KeyEvent::decode(bytes).map_err(|_| PortableStateInputError::Value)?);
    } else {
        return Err(PortableStateInputError::Shape);
    }
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
        .map_err(|_| PortableStateInputError::Kernel)
}

fn scheduler(
    fragment: &conduit_core::PlanFragment,
    lowered: &conduit_plan_lowering::lowering::LoweredPlanFragment,
    prepared: &PreparedPortableStateInput,
) -> Result<Scheduler, PortableStateInputError> {
    let nodes = lowered
        .node_specs
        .as_slice()
        .try_into()
        .map_err(|_| PortableStateInputError::Shape)?;
    let cords: [CordSpec; CORDS] = lowered
        .cords
        .iter()
        .map(|cord| cord.spec)
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| PortableStateInputError::Shape)?;
    let mut routes = FixedRoutes::<ROUTES, CORDS>::new(PORTS as u16);
    for route in &lowered.routes {
        routes
            .install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )
            .map_err(|_| PortableStateInputError::Kernel)?;
    }
    routes.seal().map_err(|_| PortableStateInputError::Kernel)?;
    let mut bindings = FixedHostOperationBindings::<HOST_BINDINGS>::new(NODES as u16);
    for operation in &lowered.host_operations {
        bindings
            .install(operation.node, operation.binding)
            .map_err(|_| PortableStateInputError::Kernel)?;
    }
    bindings
        .seal()
        .map_err(|_| PortableStateInputError::Kernel)?;
    let mut values = FixedValueStore::<VALUES, MAX_VALUE_BYTES>::new(VALUE_BYTES as u32)
        .map_err(|_| PortableStateInputError::Value)?;
    let tick_bytes = conduit_time::encode_tick(prepared.tick_sequence);
    let ticks = [
        values
            .store(&tick_bytes)
            .map_err(|_| PortableStateInputError::Value)?,
        values
            .store(&tick_bytes)
            .map_err(|_| PortableStateInputError::Value)?,
    ];
    let key = values
        .store(&prepared.key.encode())
        .map_err(|_| PortableStateInputError::Value)?;
    let count_values = [
        values
            .store(&prepared.count_start.to_le_bytes())
            .map_err(|_| PortableStateInputError::Value)?,
        values
            .store(
                &conduit_semantic_catalog::bounded_count_value(prepared.count_start, 1)
                    .ok_or(PortableStateInputError::Value)?
                    .to_le_bytes(),
            )
            .map_err(|_| PortableStateInputError::Value)?,
    ];
    let toggle_values = [
        values
            .store(
                &InfoBool::new(
                    conduit_semantic_catalog::bounded_toggle_value(prepared.toggle_initial, 0)
                        .ok_or(PortableStateInputError::Value)?,
                )
                .encode(),
            )
            .map_err(|_| PortableStateInputError::Value)?,
        values
            .store(
                &InfoBool::new(
                    conduit_semantic_catalog::bounded_toggle_value(prepared.toggle_initial, 1)
                        .ok_or(PortableStateInputError::Value)?,
                )
                .encode(),
            )
            .map_err(|_| PortableStateInputError::Value)?,
    ];
    let mut count_sink = None;
    let mut bool_sink = None;
    let mut text_sink = None;
    let mut chord_sink = None;
    let mut tick_source = 0;
    let drivers = fragment
        .placements
        .iter()
        .enumerate()
        .map(|(index, placement)| {
            let operation = match placement.kind_id.as_str() {
                TICK_SOURCE_KIND if tick_source < ticks.len() => {
                    let value = ticks[tick_source];
                    tick_source += 1;
                    PortableStateInputOperation::Source {
                        value,
                        emitted: false,
                    }
                }
                KEY_SOURCE_KIND => PortableStateInputOperation::Source {
                    value: key,
                    emitted: false,
                },
                conduit_semantic_catalog::STATE_COUNT_KIND => PortableStateInputOperation::Count {
                    values: count_values,
                    next: 0,
                    initial_emitted: false,
                },
                conduit_semantic_catalog::STATE_TOGGLE_KIND => {
                    PortableStateInputOperation::Toggle {
                        values: toggle_values,
                        next: 0,
                        initial_emitted: false,
                    }
                }
                conduit_semantic_catalog::KEY_EVENT_TEE_KIND => {
                    PortableStateInputOperation::KeyTee {
                        pending: None,
                        phase: 0,
                    }
                }
                COUNT_SINK_KIND | BOOL_SINK_KIND | TEXT_KEY_SINK_KIND | CHORD_KEY_SINK_KIND => {
                    let node = NodeId(index as u16);
                    let maximum_bytes = match placement.kind_id.as_str() {
                        COUNT_SINK_KIND => {
                            count_sink = Some(node);
                            conduit_semantic_catalog::COUNT_ENCODED_LEN
                        }
                        BOOL_SINK_KIND => {
                            bool_sink = Some(node);
                            conduit_core::BOOL_ENCODED_LEN as u32
                        }
                        TEXT_KEY_SINK_KIND => {
                            text_sink = Some(node);
                            conduit_human::KEY_EVENT_ENCODED_LEN as u32
                        }
                        _ => {
                            chord_sink = Some(node);
                            conduit_human::KEY_EVENT_ENCODED_LEN as u32
                        }
                    };
                    PortableStateInputOperation::Sink {
                        maximum_bytes,
                        pending: false,
                        next_request: 0,
                    }
                }
                _ => return Err(PortableStateInputError::Shape),
            };
            OperationDriver::new(operation).map_err(|_| PortableStateInputError::Kernel)
        })
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| PortableStateInputError::Shape)?;
    let signs = FixedSignLog::<SIGNS>::new(
        (SIGNS * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32,
    )
    .map_err(|_| PortableStateInputError::Kernel)?;
    let kernel = FixedScheduler::new_with_host_operations(
        nodes, cords, routes, bindings, drivers, values, signs,
    )
    .map_err(|_| PortableStateInputError::Kernel)?;
    Ok(Scheduler {
        kernel,
        count_sink: count_sink.ok_or(PortableStateInputError::Shape)?,
        bool_sink: bool_sink.ok_or(PortableStateInputError::Shape)?,
        text_sink: text_sink.ok_or(PortableStateInputError::Shape)?,
        chord_sink: chord_sink.ok_or(PortableStateInputError::Shape)?,
    })
}
