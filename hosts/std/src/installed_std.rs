mod catalog;
pub(super) mod contract;
mod count_operations;
mod external_websocket;
mod external_websocket_host;
mod generate_text;
mod operation;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod test_text_source;
mod text_operations;
#[cfg(test)]
mod text_operations_tests;
mod tick_presentation;

use self::catalog::factory;
pub(crate) use self::catalog::supports;
#[cfg(test)]
use self::contract::parse_tick_configuration;
use self::contract::{decode_tick, TICK_ENCODED_LEN};
use self::generate_text::execute_fixture;
use self::operation::InstalledOperation;
#[cfg(test)]
use self::operation::{TEST_OBSERVER_IMPLEMENTATION, TICK_FACTORY};
use super::{
    RunControl, RunControlDisposition, RunControlReceipt, StdKernelExecutionReport, StdRunReport,
    TimerAdapter,
};
#[cfg(test)]
use conduit_core::present_host_operation_requirement;
use conduit_core::{
    bind_active_play, bind_clue, kind_id, wait_host_operation_requirement, CancellationReason,
    HostAdvertisement, Observation, ObservationKind, PlanFragment, TerminalDisposition,
};
use conduit_kernel::scheduler::{
    CordSpec, FixedScheduler, HostOperationRequest, NodeSpec, OperationDriver, SchedulerStatus,
};
use conduit_kernel::{
    BoundedValueRef, ClueSink, CordEndpoint, CordId, FixedHostOperationBindings, FixedRoutes,
    HostOperationDisposition, HostOperationOutcome, HostedClueLog, HostedValueStore, NodeId,
    PortId, ValueStorage,
};
use conduit_runtime::lowering::{
    lower_plan_fragment, KernelExecutionIdentityMap, MAXIMUM_KERNEL_PORTS_PER_NODE,
};
use std::io::Write;
use std::time::Duration;

const MAX_NODES: usize = 8;
const MAX_CORDS: usize = 8;
const PORTS: usize = MAXIMUM_KERNEL_PORTS_PER_NODE;
const MAX_QUEUE_SLOTS: usize = 64;
const ROUTE_SLOTS: usize = MAX_NODES * PORTS;
const ROUTE_TARGETS: usize = 64;

pub(super) use contract::text_offer;
#[cfg(test)]
pub(super) use test_support::{test_catalog, test_observer_offer};

#[cfg(test)]
pub(super) fn test_text_source_offer() -> conduit_core::CapabilityOffer {
    test_text_source::offer()
}
const HOST_OPERATIONS_PER_NODE: u16 = 3;
const HOST_BINDING_SLOTS: usize = MAX_NODES * HOST_OPERATIONS_PER_NODE as usize;
const PENDING_REQUESTS: usize = MAX_NODES;

type InstalledScheduler = FixedScheduler<
    OperationDriver<InstalledOperation, PORTS>,
    HostedValueStore,
    HostedClueLog,
    MAX_NODES,
    MAX_CORDS,
    PORTS,
    MAX_QUEUE_SLOTS,
    ROUTE_SLOTS,
    ROUTE_TARGETS,
    HOST_BINDING_SLOTS,
    PENDING_REQUESTS,
>;

pub(super) use contract::every_offer;
pub(super) use contract::tick_offer;

pub(super) fn run_fragment<W: Write, T: TimerAdapter>(
    advertisement: &HostAdvertisement,
    fragment: &PlanFragment,
    play_sequence: u64,
    next_clue_sequence: &mut u64,
    _output: &mut W,
    timer: &mut T,
    control: &RunControl,
) -> Result<StdRunReport, String> {
    let lowered = lower_plan_fragment(fragment).map_err(|error| format!("lowering: {error:?}"))?;
    let active_nodes = lowered.nodes.len();
    let active_cords = lowered.cords.len();
    if !supports(fragment)
        || active_nodes == 0
        || active_nodes > MAX_NODES
        || active_cords == 0
        || active_cords > MAX_CORDS
        || lowered.cord_value_slots as usize > MAX_QUEUE_SLOTS
        || lowered.routes.len() > ROUTE_SLOTS
        || lowered
            .routes
            .iter()
            .map(|route| route.targets.len())
            .sum::<usize>()
            > ROUTE_TARGETS
        || !lowered.remote_endpoints.is_empty()
        || lowered.host_operations.len() > HOST_BINDING_SLOTS
    {
        return Err("fragment exceeds the installed std kernel profile".to_string());
    }

    let mut value_items = 0_u16;
    let mut value_bytes = 0_u32;
    let mut request_capacity = 0_usize;
    let mut maximum_value_bytes = TICK_ENCODED_LEN;
    let mut clue_items = 32_u16;
    for placement in &fragment.placements {
        let factory = factory(&placement.implementation_id)
            .ok_or_else(|| "planned implementation is not installed".to_string())?;
        let budget = (factory.budget)(placement)?;
        value_items = value_items
            .checked_add(budget.value_items)
            .ok_or_else(|| "installed value item budget overflow".to_string())?;
        value_bytes = value_bytes
            .checked_add(budget.value_bytes)
            .ok_or_else(|| "installed value byte budget overflow".to_string())?;
        request_capacity = request_capacity
            .checked_add(budget.host_requests)
            .ok_or_else(|| "installed request budget overflow".to_string())?;
        clue_items = clue_items
            .checked_add(budget.clue_items)
            .ok_or_else(|| "installed clue item budget overflow".to_string())?;
        maximum_value_bytes = maximum_value_bytes.max(budget.maximum_value_bytes);
    }
    #[cfg(test)]
    if fragment
        .placements
        .iter()
        .any(|placement| placement.implementation_id.as_str() == TEST_OBSERVER_IMPLEMENTATION)
    {
        request_capacity = request_capacity
            .checked_mul(2)
            .ok_or_else(|| "fixture request budget overflow".to_string())?;
    }

    let mut values =
        HostedValueStore::new(value_items.max(1), maximum_value_bytes, value_bytes.max(1))
            .map_err(|error| format!("installed value store: {error:?}"))?;
    let mut operations = Vec::with_capacity(MAX_NODES);
    for node in &lowered.nodes {
        let placement = fragment
            .placements
            .get(usize::from(node.node.0))
            .ok_or_else(|| "lowered node has no planned placement".to_string())?;
        let factory = factory(&placement.implementation_id)
            .ok_or_else(|| "planned implementation is not installed".to_string())?;
        operations.push((factory.prepare)(placement, &mut values)?);
    }
    while operations.len() < MAX_NODES {
        operations.push(InstalledOperation::inactive());
    }
    let drivers: [OperationDriver<InstalledOperation, PORTS>; MAX_NODES] = operations
        .into_iter()
        .map(|operation| {
            OperationDriver::new(operation)
                .map_err(|error| format!("prepare installed operation: {error:?}"))
        })
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| "installed driver capacity changed".to_string())?;
    let driver_capacity_before = drivers
        .iter()
        .map(|driver| driver.operation().allocation_capacity())
        .sum::<usize>();
    let value_allocation_before = values.allocation_capacities();

    let inactive_node = NodeSpec {
        input_cords: [None; PORTS],
        maximum_step_work: 1,
    };
    let mut node_specs = [inactive_node; MAX_NODES];
    node_specs[..active_nodes].copy_from_slice(&lowered.node_specs);
    let inactive_cord = CordSpec {
        cord: CordId(u16::MAX),
        source: CordEndpoint::local(NodeId(u16::MAX), PortId(u16::MAX)),
        sink: CordEndpoint::local(NodeId(u16::MAX), PortId(u16::MAX)),
        slot_start: u16::MAX,
        item_capacity: 0,
        byte_capacity: 0,
    };
    let mut cord_specs = [inactive_cord; MAX_CORDS];
    for (destination, lowered_cord) in cord_specs
        .iter_mut()
        .zip(lowered.cords.iter())
        .take(active_cords)
    {
        *destination = lowered_cord.spec;
    }
    let mut routes = FixedRoutes::<ROUTE_SLOTS, ROUTE_TARGETS>::new(PORTS as u16);
    for route in &lowered.routes {
        routes
            .install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )
            .map_err(|error| format!("install std route: {error:?}"))?;
    }
    routes
        .seal()
        .map_err(|error| format!("seal std routes: {error:?}"))?;
    let mut host_bindings =
        FixedHostOperationBindings::<HOST_BINDING_SLOTS>::new(HOST_OPERATIONS_PER_NODE);
    for operation in &lowered.host_operations {
        host_bindings
            .install(operation.node, operation.binding)
            .map_err(|error| format!("install std host operation: {error:?}"))?;
    }
    host_bindings
        .seal()
        .map_err(|error| format!("seal std host operations: {error:?}"))?;
    let clue_bytes = u32::from(clue_items)
        .checked_mul(
            u32::try_from(core::mem::size_of::<conduit_kernel::KernelEvent>())
                .map_err(|_| "installed clue charge overflow".to_string())?,
        )
        .ok_or_else(|| "installed clue byte budget overflow".to_string())?;
    let clue = HostedClueLog::new(clue_items, clue_bytes)
        .map_err(|error| format!("installed clue store: {error:?}"))?;
    let mut external_listener = external_websocket_host::prepare(fragment)?;
    if let Some(listener) = &external_listener {
        writeln!(
            _output,
            "external-websocket-ready address={}",
            listener
                .local_addr()
                .map_err(|error| format!("read external WebSocket address: {error:?}"))?
        )
        .map_err(|error| error.to_string())?;
        _output.flush().map_err(|error| error.to_string())?;
    }
    let mut scheduler = InstalledScheduler::new_with_active_counts_and_host_operations(
        active_nodes,
        active_cords,
        node_specs,
        cord_specs,
        routes,
        host_bindings,
        drivers,
        values,
        clue,
    )
    .map_err(|error| format!("install std scheduler: {error:?}"))?;

    let active_play = bind_active_play(
        &fragment.plan_id,
        &advertisement.host_id,
        &advertisement.boot_id,
        play_sequence,
    );
    let mut execution_identity =
        KernelExecutionIdentityMap::new(&lowered.identity, &active_play, request_capacity, 0, 1)
            .map_err(|error| format!("prepare std execution identity: {error:?}"))?;
    let mut requests = Vec::<HostOperationRequest>::with_capacity(request_capacity);
    let wait_contract_id = wait_host_operation_requirement().contract_id;
    let text_target_kind = kind_id("presentation/stdout-text");
    let tick_target_kind = kind_id(conduit_std_catalog::TICK_PRESENTATION_TARGET);
    let count_target_kind = kind_id(conduit_std_catalog::COUNT_PRESENTATION_TARGET);
    let upper_contract_id = conduit_core::HostOperationContractId::from(
        conduit_std_catalog::TEXT_UPPER_HOST_OPERATION_CONTRACT,
    );
    let upper_target_kind = kind_id(conduit_std_catalog::TEXT_UPPER_HOST_OPERATION_TARGET);
    let join_contract_id = conduit_core::HostOperationContractId::from(
        conduit_std_catalog::TEXT_JOIN_HOST_OPERATION_CONTRACT,
    );
    let join_target_kind = kind_id(conduit_std_catalog::TEXT_JOIN_HOST_OPERATION_TARGET);
    let mut uppercase_buffer = Vec::with_capacity(contract::MAX_TEXT_BYTES as usize);
    let mut external_output =
        Vec::with_capacity(conduit_net::MAXIMUM_EXTERNAL_WEBSOCKET_MESSAGE_BYTES as usize + 1);
    let mut generate_text_output =
        Vec::with_capacity(conduit_ai::MAXIMUM_OUTPUT_TOKENS as usize * 4);
    #[cfg(test)]
    let mut observed_ticks = Vec::with_capacity(request_capacity / 2);
    #[cfg(test)]
    let observer_contract_id = present_host_operation_requirement(
        kind_id("conduit.test/tick-observation"),
        TICK_ENCODED_LEN,
    )
    .contract_id;
    #[cfg(test)]
    let observer_target_kind = kind_id("conduit.test/tick-observation");
    #[cfg(test)]
    let play_start_probe = crate::allocation_probe::begin();
    let mut accepted_stop = None;
    let terminal_disposition = loop {
        if accepted_stop.is_none() {
            if let Some(request_id) = control.requested_stop() {
                scheduler
                    .cancel()
                    .map_err(|error| format!("cancel installed kernel: {error:?}"))?;
                accepted_stop = Some(request_id);
            }
        }
        while let Some(request) = scheduler.next_host_request() {
            let input = scheduler
                .host_value(request.input.value)
                .map_err(|error| format!("read std host input: {error:?}"))?;
            let lowered_operation = lowered
                .host_operations
                .iter()
                .find(|operation| {
                    operation.node == request.node && operation.operation == request.operation
                })
                .ok_or_else(|| "host request has no lowered contract identity".to_string())?;
            let contract = &lowered_operation.contract_id;
            if contract.as_str() == conduit_ai::GENERATE_TEXT_HOST_OPERATION {
                let placement = fragment
                    .placements
                    .get(usize::from(request.node.0))
                    .ok_or_else(|| "generate-text request has no exact placement".to_string())?;
                execute_fixture(placement, input, &mut generate_text_output)?;
                let value = scheduler
                    .store_host_value(&generate_text_output)
                    .map_err(|error| format!("store generate-text fixture output: {error:?}"))?;
                requests.push(request);
                scheduler
                    .complete_host_operation(
                        request.node,
                        request.request,
                        HostOperationOutcome {
                            disposition: HostOperationDisposition::Completed,
                            output: Some(
                                BoundedValueRef::new(
                                    value,
                                    lowered_operation.binding.maximum_output_bytes,
                                )
                                .map_err(|error| {
                                    format!("bound generate-text fixture output: {error:?}")
                                })?,
                            ),
                            failure: None,
                        },
                    )
                    .map_err(|error| {
                        format!("complete generate-text fixture operation: {error:?}")
                    })?;
                continue;
            } else if contract
                .as_str()
                .starts_with("conduit.host/external-websocket-listener-")
            {
                let completion = external_websocket_host::execute(
                    contract.as_str(),
                    input,
                    &mut external_listener,
                    &mut external_output,
                )?;
                let (disposition, output) = match completion {
                    external_websocket_host::ExternalHostCompletion::Output => {
                        let value =
                            scheduler
                                .store_host_value(&external_output)
                                .map_err(|error| {
                                    format!("store external WebSocket output: {error:?}")
                                })?;
                        (
                            HostOperationDisposition::Completed,
                            Some(
                                BoundedValueRef::new(
                                    value,
                                    lowered_operation.binding.maximum_output_bytes,
                                )
                                .map_err(|error| {
                                    format!("bound external WebSocket output: {error:?}")
                                })?,
                            ),
                        )
                    }
                    external_websocket_host::ExternalHostCompletion::NoOutput => {
                        (HostOperationDisposition::Completed, None)
                    }
                    external_websocket_host::ExternalHostCompletion::ReturnedInput => {
                        (HostOperationDisposition::Completed, Some(request.input))
                    }
                    external_websocket_host::ExternalHostCompletion::Disconnected => {
                        let output = if external_output.is_empty() {
                            None
                        } else {
                            let value =
                                scheduler
                                    .store_host_value(&external_output)
                                    .map_err(|error| {
                                        format!("store external WebSocket disconnect: {error:?}")
                                    })?;
                            Some(
                                BoundedValueRef::new(
                                    value,
                                    lowered_operation.binding.maximum_output_bytes,
                                )
                                .map_err(|error| {
                                    format!("bound external WebSocket disconnect: {error:?}")
                                })?,
                            )
                        };
                        (HostOperationDisposition::Cancelled, output)
                    }
                };
                requests.push(request);
                scheduler
                    .complete_host_operation(
                        request.node,
                        request.request,
                        HostOperationOutcome {
                            disposition,
                            output,
                            failure: None,
                        },
                    )
                    .map_err(|error| {
                        format!("complete external WebSocket host operation: {error:?}")
                    })?;
                continue;
            } else if contract == &wait_contract_id {
                let duration = decode_tick(input).map_err(|error| error.to_string())?;
                timer.wait(Duration::from_millis(duration));
            } else if contract == &upper_contract_id
                && lowered_operation.target_kind.as_ref() == Some(&upper_target_kind)
            {
                text_operations::uppercase_utf8(input, &mut uppercase_buffer)?;
                let value = scheduler
                    .store_host_value(&uppercase_buffer)
                    .map_err(|error| format!("store uppercase text output: {error:?}"))?;
                requests.push(request);
                scheduler
                    .complete_host_operation(
                        request.node,
                        request.request,
                        text_operations::completed_with_output(value),
                    )
                    .map_err(|error| format!("complete text/upper host operation: {error:?}"))?;
                continue;
            } else if contract == &join_contract_id
                && lowered_operation.target_kind.as_ref() == Some(&join_target_kind)
            {
                let placement = fragment
                    .placements
                    .get(usize::from(request.node.0))
                    .ok_or_else(|| "text/join request has no exact placement".to_string())?;
                let prefix = text_operations::join_prefix(placement)?;
                text_operations::prefix_utf8(prefix, input, &mut uppercase_buffer)?;
                let value = scheduler
                    .store_host_value(&uppercase_buffer)
                    .map_err(|error| format!("store joined text output: {error:?}"))?;
                requests.push(request);
                scheduler
                    .complete_host_operation(
                        request.node,
                        request.request,
                        text_operations::completed_with_output(value),
                    )
                    .map_err(|error| format!("complete text/join host operation: {error:?}"))?;
                continue;
            } else if lowered_operation.target_kind.as_ref() == Some(&text_target_kind) {
                let text = std::str::from_utf8(input)
                    .map_err(|_| "text presentation input is not valid UTF-8".to_string())?;
                writeln!(_output, "{text}").map_err(|error| error.to_string())?;
            } else if lowered_operation.target_kind.as_ref() == Some(&tick_target_kind) {
                let tick = decode_tick(input).map_err(|error| error.to_string())?;
                writeln!(_output, "tick sequence={tick}").map_err(|error| error.to_string())?;
            } else if lowered_operation.target_kind.as_ref() == Some(&count_target_kind) {
                let count = count_operations::decode_count(input)?;
                writeln!(_output, "count value={count}").map_err(|error| error.to_string())?;
            } else {
                #[cfg(test)]
                {
                    if contract != &observer_contract_id
                        || lowered_operation.target_kind.as_ref() != Some(&observer_target_kind)
                    {
                        return Err("installed host-operation contract is unsupported".to_string());
                    }
                    let tick = decode_tick(input).map_err(|error| error.to_string())?;
                    observed_ticks.push(tick);
                    writeln!(_output, "receipt tick sequence={tick}")
                        .map_err(|error| error.to_string())?;
                }
                #[cfg(not(test))]
                return Err("installed host-operation contract is unsupported".to_string());
            }
            requests.push(request);
            scheduler
                .complete_host_operation(
                    request.node,
                    request.request,
                    HostOperationOutcome {
                        disposition: HostOperationDisposition::Completed,
                        output: None,
                        failure: None,
                    },
                )
                .map_err(|error| format!("complete std host operation: {error:?}"))?;
        }
        match scheduler
            .step()
            .map_err(|error| format!("installed kernel step: {error:?}"))?
        {
            SchedulerStatus::Progress { .. } => {}
            SchedulerStatus::Complete => break TerminalDisposition::Completed,
            SchedulerStatus::Idle => {
                return Err("installed kernel became idle before completion".to_string());
            }
            SchedulerStatus::Cancelled => {
                break TerminalDisposition::Cancelled {
                    reason: CancellationReason::OperatorRequested,
                }
            }
        }
    };
    #[cfg(test)]
    let post_play_start_allocations = play_start_probe.finish();

    #[cfg(test)]
    if fragment
        .placements
        .iter()
        .any(|placement| placement.implementation_id.as_str() == TEST_OBSERVER_IMPLEMENTATION)
    {
        let tick_placement = fragment
            .placements
            .iter()
            .find(|placement| {
                placement.implementation_id.as_str() == TICK_FACTORY.implementation_id
            })
            .ok_or_else(|| "tick observer fixture has no installed tick source".to_string())?;
        let count = parse_tick_configuration(&tick_placement.configuration)
            .map_err(|error| error.to_string())?
            .count;
        if observed_ticks != (0..count).collect::<Vec<_>>() {
            return Err("tick observer received an incomplete or reordered sequence".to_string());
        }
    }
    if scheduler.values().used_items() != 0 {
        return Err("installed kernel retained values after completion".to_string());
    }
    let driver_capacity_after = scheduler
        .drivers()
        .iter()
        .map(|driver| driver.operation().allocation_capacity())
        .sum::<usize>();
    let value_allocation_after = scheduler.values().allocation_capacities();
    if driver_capacity_after != driver_capacity_before
        || value_allocation_after != value_allocation_before
    {
        return Err("installed storage grew after Play start".to_string());
    }
    for request in &requests {
        execution_identity
            .bind_request(
                &lowered.identity,
                request.node,
                request.request,
                request.operation,
            )
            .map_err(|error| format!("bind std request identity: {error:?}"))?;
    }
    let terminal_clue = bind_clue(
        &advertisement.host_id,
        &advertisement.boot_id,
        Some(&active_play.active_play_id),
        *next_clue_sequence,
    );
    *next_clue_sequence = next_clue_sequence
        .checked_add(1)
        .ok_or_else(|| "std clue sequence exhausted".to_string())?;
    execution_identity
        .bind_clue(&terminal_clue, None, None, None)
        .map_err(|error| format!("bind std terminal clue: {error:?}"))?;
    let observations = vec![Observation {
        clue_id: terminal_clue.clue_id,
        active_play_id: Some(active_play.active_play_id.clone()),
        presentation_id: None,
        host_id: advertisement.host_id.clone(),
        boot_id: advertisement.boot_id.clone(),
        plan_id: Some(fragment.plan_id.clone()),
        placement_id: None,
        connection_id: None,
        kind: ObservationKind::PlanTerminal {
            disposition: terminal_disposition,
        },
    }];
    let control_receipts = accepted_stop
        .map(|request_id| RunControlReceipt {
            request_id,
            active_play_id: active_play.active_play_id.clone(),
            disposition: RunControlDisposition::Accepted,
        })
        .into_iter()
        .collect();
    Ok(StdRunReport {
        observations,
        receipts: Vec::new(),
        control_receipts,
        kernel: Some(StdKernelExecutionReport {
            active_play_id: active_play.active_play_id,
            decisions: scheduler.decisions(),
            kernel_events: scheduler.clues().len(),
            kernel_clue: scheduler.clues().events().collect(),
            value_allocation_capacity_before: value_allocation_before,
            value_allocation_capacity_after: value_allocation_after,
            presentation_ids: Vec::new(),
            identity: execution_identity,
            #[cfg(test)]
            post_play_start_allocations,
        }),
    })
}
