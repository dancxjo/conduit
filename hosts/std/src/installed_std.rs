pub(super) mod contract;
mod operation;

use self::contract::{decode_tick, TICK_ENCODED_LEN};
#[cfg(test)]
use self::contract::{parse_tick_configuration, TICK_VALUE_KIND};
use self::operation::{InstalledFactory, InstalledOperation, TICK_FACTORY};
#[cfg(test)]
use self::operation::{TEST_OBSERVER_FACTORY, TEST_OBSERVER_IMPLEMENTATION, TEST_OBSERVER_KIND};
use super::{StdKernelExecutionReport, StdRunReport, TimerAdapter};
use conduit_core::{
    bind_active_play, bind_evidence, wait_host_operation_requirement, HostAdvertisement,
    ImplementationId, Observation, ObservationKind, PlanFragment, TerminalDisposition,
};
#[cfg(test)]
use conduit_core::{
    kind_id, present_host_operation_requirement, ArtifactId, CapabilityId, CapabilityLimits,
    CapabilityOffer, ExecutionProfileId, KindContractRevision, PortDescriptor, PortDirection,
};
use conduit_kernel::scheduler::{
    CordSpec, FixedScheduler, HostOperationRequest, NodeSpec, OperationDriver, SchedulerStatus,
};
use conduit_kernel::{
    CordEndpoint, CordId, EvidenceSink, FixedHostOperationBindings, FixedRoutes,
    HostOperationDisposition, HostOperationOutcome, HostedEvidenceLog, HostedValueStore, NodeId,
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
const HOST_BINDING_SLOTS: usize = MAX_NODES;
const PENDING_REQUESTS: usize = MAX_NODES;

type InstalledScheduler = FixedScheduler<
    OperationDriver<InstalledOperation, PORTS>,
    HostedValueStore,
    HostedEvidenceLog,
    MAX_NODES,
    MAX_CORDS,
    PORTS,
    MAX_QUEUE_SLOTS,
    ROUTE_SLOTS,
    ROUTE_TARGETS,
    HOST_BINDING_SLOTS,
    PENDING_REQUESTS,
>;

const FACTORIES: &[&InstalledFactory] = &[
    &TICK_FACTORY,
    #[cfg(test)]
    &TEST_OBSERVER_FACTORY,
];

pub(super) use contract::tick_offer;

fn factory(implementation_id: &ImplementationId) -> Option<&'static InstalledFactory> {
    FACTORIES
        .iter()
        .copied()
        .find(|factory| factory.implementation_id == implementation_id.as_str())
}

pub(super) fn supports(fragment: &PlanFragment) -> bool {
    !fragment.placements.is_empty()
        && fragment
            .placements
            .iter()
            .all(|placement| factory(&placement.implementation_id).is_some())
}

pub(super) fn run_fragment<W: Write, T: TimerAdapter>(
    advertisement: &HostAdvertisement,
    fragment: &PlanFragment,
    activation_sequence: u64,
    next_evidence_sequence: &mut u64,
    _output: &mut W,
    timer: &mut T,
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
    let mut evidence_items = 32_u16;
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
        evidence_items = evidence_items
            .checked_add(budget.evidence_items)
            .ok_or_else(|| "installed evidence item budget overflow".to_string())?;
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
        HostedValueStore::new(value_items.max(1), TICK_ENCODED_LEN, value_bytes.max(1))
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
    let mut host_bindings = FixedHostOperationBindings::<HOST_BINDING_SLOTS>::new(1);
    for operation in &lowered.host_operations {
        host_bindings
            .install(operation.node, operation.binding)
            .map_err(|error| format!("install std host operation: {error:?}"))?;
    }
    host_bindings
        .seal()
        .map_err(|error| format!("seal std host operations: {error:?}"))?;
    let evidence_bytes = u32::from(evidence_items)
        .checked_mul(
            u32::try_from(core::mem::size_of::<conduit_kernel::KernelEvent>())
                .map_err(|_| "installed evidence charge overflow".to_string())?,
        )
        .ok_or_else(|| "installed evidence byte budget overflow".to_string())?;
    let evidence = HostedEvidenceLog::new(evidence_items, evidence_bytes)
        .map_err(|error| format!("installed evidence store: {error:?}"))?;
    let mut scheduler = InstalledScheduler::new_with_active_counts_and_host_operations(
        active_nodes,
        active_cords,
        node_specs,
        cord_specs,
        routes,
        host_bindings,
        drivers,
        values,
        evidence,
    )
    .map_err(|error| format!("install std scheduler: {error:?}"))?;

    let active_play = bind_active_play(
        &fragment.plan_id,
        &advertisement.host_id,
        &advertisement.boot_id,
        activation_sequence,
    );
    let mut execution_identity =
        KernelExecutionIdentityMap::new(&lowered.identity, &active_play, request_capacity, 0, 1)
            .map_err(|error| format!("prepare std execution identity: {error:?}"))?;
    let mut requests = Vec::<HostOperationRequest>::with_capacity(request_capacity);
    let wait_contract_id = wait_host_operation_requirement().contract_id;
    #[cfg(test)]
    let mut observed_ticks = Vec::with_capacity(request_capacity / 2);
    #[cfg(test)]
    let observer_contract_id = present_host_operation_requirement(
        kind_id("conduit.test/tick-observation"),
        TICK_ENCODED_LEN,
    )
    .contract_id;
    #[cfg(test)]
    let activation_probe = crate::allocation_probe::begin();
    loop {
        while let Some(request) = scheduler.next_host_request() {
            let input = scheduler
                .host_value(request.input.value)
                .map_err(|error| format!("read std host input: {error:?}"))?;
            let contract = lowered
                .identity
                .host_operation_contract(request.node, request.operation)
                .ok_or_else(|| "host request has no lowered contract identity".to_string())?;
            if contract == &wait_contract_id {
                let duration = decode_tick(input).map_err(|error| error.to_string())?;
                timer.wait(Duration::from_millis(duration));
            } else {
                #[cfg(test)]
                {
                    if contract != &observer_contract_id {
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
            SchedulerStatus::Complete => break,
            SchedulerStatus::Idle => {
                return Err("installed kernel became idle before completion".to_string());
            }
            SchedulerStatus::Cancelled => return Err("installed kernel was cancelled".to_string()),
        }
    }
    #[cfg(test)]
    let post_activation_allocations = activation_probe.finish();

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
        return Err("installed storage grew after activation".to_string());
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
    let terminal_evidence = bind_evidence(
        &advertisement.host_id,
        &advertisement.boot_id,
        Some(&active_play.active_play_id),
        *next_evidence_sequence,
    );
    *next_evidence_sequence = next_evidence_sequence
        .checked_add(1)
        .ok_or_else(|| "std evidence sequence exhausted".to_string())?;
    execution_identity
        .bind_evidence(&terminal_evidence, None, None, None)
        .map_err(|error| format!("bind std terminal evidence: {error:?}"))?;
    let observations = vec![Observation {
        evidence_id: terminal_evidence.evidence_id,
        active_play_id: Some(active_play.active_play_id.clone()),
        presentation_id: None,
        host_id: advertisement.host_id.clone(),
        boot_id: advertisement.boot_id.clone(),
        plan_id: Some(fragment.plan_id.clone()),
        placement_id: None,
        connection_id: None,
        kind: ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed,
        },
    }];
    Ok(StdRunReport {
        observations,
        receipts: Vec::new(),
        kernel: Some(StdKernelExecutionReport {
            active_play_id: active_play.active_play_id,
            decisions: scheduler.decisions(),
            kernel_events: scheduler.evidence().len(),
            value_allocation_capacity_before: value_allocation_before,
            value_allocation_capacity_after: value_allocation_after,
            presentation_ids: Vec::new(),
            identity: execution_identity,
            #[cfg(test)]
            post_activation_allocations,
        }),
    })
}

#[cfg(test)]
const TEST_OBSERVER_REVISION: &str = "conduit.test/tick-observer@1";
#[cfg(test)]
const TEST_OBSERVER_PROFILE: &str = "conduit.test/tick-observer-kernel@1";
#[cfg(test)]
const TEST_OBSERVER_ARTIFACT: &str = "conduit-std-host/test-tick-observer@1";

#[cfg(test)]
pub(super) fn test_observer_offer() -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from("test-tick-observer"),
        kind_id: kind_id(TEST_OBSERVER_KIND),
        kind_contract_revision: KindContractRevision::from(TEST_OBSERVER_REVISION),
        execution_profile_id: ExecutionProfileId::from(TEST_OBSERVER_PROFILE),
        implementation_id: ImplementationId::from(TEST_OBSERVER_IMPLEMENTATION),
        artifact_id: ArtifactId::from(TEST_OBSERVER_ARTIFACT),
        inputs: vec![PortDescriptor {
            port_id: conduit_core::port_id("in"),
            value_kind: kind_id(TICK_VALUE_KIND),
            direction: PortDirection::Input,
        }],
        outputs: Vec::new(),
        host_operations: vec![present_host_operation_requirement(
            kind_id("conduit.test/tick-observation"),
            TICK_ENCODED_LEN,
        )],
        resource_requirements: vec![conduit_core::resource_requirement(
            conduit_core::PRESENTATION_RESOURCE_CLASS,
            1,
        )],
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 4,
            max_queue_bytes: 64,
        },
    }
}

#[cfg(test)]
pub(super) fn test_catalog() -> conduit_form::ProfileCatalog {
    use conduit_form::KindDefinition;

    let mut catalog = contract::test_tick_catalog();
    catalog
        .insert(KindDefinition {
            kind_id: kind_id(TEST_OBSERVER_KIND),
            kind_contract_revision: KindContractRevision::from(TEST_OBSERVER_REVISION),
            inputs: test_observer_offer().inputs,
            outputs: Vec::new(),
            configuration: Vec::new(),
        })
        .expect("test observer kind is distinct from typed tick");
    catalog
}
