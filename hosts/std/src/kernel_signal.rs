//! Installed hosted-kernel profile for the exact two-node Signal form.

use super::{SignalReceipt, StdKernelExecutionReport, StdRunReport, TimerAdapter};
use conduit_core::{
    bind_active_play, bind_evidence, bind_presentation, HostAdvertisement, Observation,
    ObservationKind, PlanFragment, TerminalDisposition, ValuePayload,
};
use conduit_kernel::scheduler::{FixedScheduler, OperationDriver, SchedulerStatus};
use conduit_kernel::{
    BoundedValueRef, EvidenceSink, Failure, FailureCode, FixedHostOperationBindings, FixedRoutes,
    HostOperationDisposition, HostOperationId, HostOperationOutcome, HostedEvidenceLog,
    HostedValueStore, Operation, OperationAction, OperationInput, PortId, RequestId, ValueRef,
    ValueStorage,
};
use conduit_runtime::lowering::{lower_plan_fragment, MAXIMUM_KERNEL_PORTS_PER_NODE};
use conduit_signal::{
    decode_signal, encode_signal, parse_pulse_configuration, signal_value_kind, Signal, PULSE_KIND,
    SHOW_KIND, SIGNAL_ENCODED_LEN,
};
use std::io::Write;
use std::time::Duration;

const NODES: usize = 2;
const CORDS: usize = 1;
const PORTS: usize = MAXIMUM_KERNEL_PORTS_PER_NODE;
const QUEUE_SLOTS: usize = 4;
const ROUTE_SLOTS: usize = NODES * PORTS;
const ROUTE_TARGETS: usize = 1;
const HOST_BINDING_SLOTS: usize = NODES;
const PENDING_REQUESTS: usize = NODES;

type SignalScheduler = FixedScheduler<
    OperationDriver<SignalOperation, PORTS>,
    HostedValueStore,
    HostedEvidenceLog,
    NODES,
    CORDS,
    PORTS,
    QUEUE_SLOTS,
    ROUTE_SLOTS,
    ROUTE_TARGETS,
    HOST_BINDING_SLOTS,
    PENDING_REQUESTS,
>;

enum SignalOperation {
    Pulse {
        values: Vec<ValueRef>,
        waits: Vec<ValueRef>,
        next: usize,
        pending: Option<RequestId>,
    },
    Show {
        expected: Vec<ValueRef>,
        next: usize,
        pending: Option<RequestId>,
    },
}

impl SignalOperation {
    fn pulse(values: Vec<ValueRef>, waits: Vec<ValueRef>) -> Self {
        Self::Pulse {
            values,
            waits,
            next: 0,
            pending: None,
        }
    }

    fn show(expected: Vec<ValueRef>) -> Self {
        Self::Show {
            expected,
            next: 0,
            pending: None,
        }
    }

    fn fail(code: FailureCode, detail: u16) -> OperationAction {
        OperationAction::Fail(Failure { code, detail })
    }

    fn allocation_capacity(&self) -> usize {
        match self {
            Self::Pulse { values, waits, .. } => values.capacity() + waits.capacity(),
            Self::Show { expected, .. } => expected.capacity(),
        }
    }
}

impl Operation for SignalOperation {
    fn start(&mut self) -> OperationAction {
        match self {
            Self::Pulse { values, .. } => {
                values
                    .first()
                    .copied()
                    .map_or(OperationAction::Complete, |value| OperationAction::Emit {
                        port: PortId(0),
                        value,
                    })
            }
            Self::Show { .. } => OperationAction::Await,
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match (self, input) {
            (
                Self::Pulse {
                    values,
                    next,
                    pending,
                    ..
                },
                OperationInput::HostOperationCompleted { request, outcome },
            ) if *pending == Some(request)
                && outcome.disposition == HostOperationDisposition::Completed
                && outcome.output.is_none()
                && outcome.failure.is_none() =>
            {
                *pending = None;
                values.get(*next).copied().map_or_else(
                    || Self::fail(FailureCode::InvalidLifecycle, 1),
                    |value| OperationAction::Emit {
                        port: PortId(0),
                        value,
                    },
                )
            }
            (
                Self::Show {
                    expected,
                    next,
                    pending,
                },
                OperationInput::Value {
                    port: PortId(0),
                    value,
                },
            ) if pending.is_none() && expected.get(*next) == Some(&value) => {
                let Ok(sequence) = u32::try_from(*next) else {
                    return Self::fail(FailureCode::InvalidLifecycle, 2);
                };
                let request = RequestId(0x8000_0000 | sequence);
                *pending = Some(request);
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(0),
                    input: BoundedValueRef::new(value, SIGNAL_ENCODED_LEN)
                        .expect("sealed signal value is exactly admitted"),
                }
            }
            (
                Self::Show { next, pending, .. },
                OperationInput::HostOperationCompleted { request, outcome },
            ) if *pending == Some(request)
                && outcome.disposition == HostOperationDisposition::Completed
                && outcome.output.is_none()
                && outcome.failure.is_none() =>
            {
                *pending = None;
                *next += 1;
                OperationAction::Await
            }
            (
                Self::Show {
                    expected,
                    next,
                    pending,
                },
                OperationInput::Closed { port: PortId(0) },
            ) if pending.is_none() && *next == expected.len() => OperationAction::Complete,
            (Self::Pulse { .. }, _) => Self::fail(FailureCode::InvalidLifecycle, 3),
            (Self::Show { .. }, _) => Self::fail(FailureCode::InvalidInput, 4),
        }
    }

    fn advance(&mut self) -> OperationAction {
        match self {
            Self::Pulse {
                values,
                waits,
                next,
                pending,
            } => {
                *next += 1;
                if *next >= values.len() {
                    return OperationAction::Complete;
                }
                let Some(wait) = waits.get(*next - 1).copied() else {
                    return Self::fail(FailureCode::InvalidLifecycle, 5);
                };
                let Ok(sequence) = u32::try_from(*next) else {
                    return Self::fail(FailureCode::InvalidLifecycle, 6);
                };
                let request = RequestId(sequence);
                *pending = Some(request);
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(0),
                    input: BoundedValueRef::new(wait, 8)
                        .expect("sealed wait value is exactly admitted"),
                }
            }
            Self::Show { .. } => OperationAction::Await,
        }
    }
}

pub(super) fn run_signal_fragment<W: Write, T: TimerAdapter>(
    advertisement: &HostAdvertisement,
    fragment: &PlanFragment,
    activation_sequence: u64,
    next_evidence_sequence: &mut u64,
    output: &mut W,
    timer: &mut T,
) -> Result<StdRunReport, String> {
    let lowered = lower_plan_fragment(fragment).map_err(|error| format!("lowering: {error:?}"))?;
    if lowered.nodes.len() != NODES
        || lowered.cords.len() != CORDS
        || lowered.cord_value_slots != QUEUE_SLOTS as u16
        || lowered
            .routes
            .iter()
            .map(|route| route.targets.len())
            .sum::<usize>()
            != ROUTE_TARGETS
        || lowered.host_operations.len() != HOST_BINDING_SLOTS
    {
        return Err("fragment does not match the installed std signal kernel profile".to_string());
    }

    let pulse_node = lowered
        .nodes
        .iter()
        .find(|node| {
            fragment.placements[usize::from(node.node.0)]
                .kind_id
                .as_str()
                == PULSE_KIND
        })
        .ok_or_else(|| "signal kernel profile has no pulse node".to_string())?;
    let show_node = lowered
        .nodes
        .iter()
        .find(|node| {
            fragment.placements[usize::from(node.node.0)]
                .kind_id
                .as_str()
                == SHOW_KIND
        })
        .ok_or_else(|| "signal kernel profile has no show node".to_string())?;
    if pulse_node.node == show_node.node {
        return Err("signal kernel placements are not distinct".to_string());
    }
    let pulse_placement = &fragment.placements[usize::from(pulse_node.node.0)];
    let show_placement = &fragment.placements[usize::from(show_node.node.0)];
    let configuration = parse_pulse_configuration(&pulse_placement.configuration)
        .map_err(|error| error.to_string())?;
    let count = usize::try_from(configuration.count)
        .map_err(|_| "signal count does not fit this hosted profile".to_string())?;
    let wait_count = count.saturating_sub(1);
    let item_capacity = u16::try_from(count.saturating_add(wait_count).max(1))
        .map_err(|_| "signal value item budget overflow".to_string())?;
    let byte_capacity = configuration
        .count
        .checked_mul(u64::from(SIGNAL_ENCODED_LEN))
        .and_then(|bytes| bytes.checked_add(u64::try_from(wait_count).ok()?.checked_mul(8)?))
        .and_then(|bytes| u32::try_from(bytes.max(1)).ok())
        .ok_or_else(|| "signal value byte budget overflow".to_string())?;
    let mut values = HostedValueStore::new(item_capacity, SIGNAL_ENCODED_LEN, byte_capacity)
        .map_err(|error| format!("signal value store: {error:?}"))?;
    let mut signal_values = Vec::with_capacity(count);
    for sequence in 0..configuration.count {
        let payload = encode_signal(&Signal {
            sequence,
            level: if sequence.is_multiple_of(2) {
                configuration.initial_level
            } else {
                !configuration.initial_level
            },
        });
        signal_values.push(
            values
                .store(&payload.encoded)
                .map_err(|error| format!("preload signal value: {error:?}"))?,
        );
    }
    let mut wait_values = Vec::with_capacity(wait_count);
    for _ in 0..wait_count {
        wait_values.push(
            values
                .store(&configuration.period_ms.to_le_bytes())
                .map_err(|error| format!("preload wait value: {error:?}"))?,
        );
    }
    let value_allocation_before = values.allocation_capacities();

    let mut routes = FixedRoutes::<ROUTE_SLOTS, ROUTE_TARGETS>::new(PORTS as u16);
    for route in &lowered.routes {
        routes
            .install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )
            .map_err(|error| format!("install route: {error:?}"))?;
    }
    routes
        .seal()
        .map_err(|error| format!("seal routes: {error:?}"))?;
    let mut host_bindings = FixedHostOperationBindings::<HOST_BINDING_SLOTS>::new(1);
    for operation in &lowered.host_operations {
        host_bindings
            .install(operation.node, operation.binding)
            .map_err(|error| format!("install host operation: {error:?}"))?;
    }
    host_bindings
        .seal()
        .map_err(|error| format!("seal host operations: {error:?}"))?;

    let mut operations = [None, None];
    operations[usize::from(pulse_node.node.0)] =
        Some(SignalOperation::pulse(signal_values.clone(), wait_values));
    operations[usize::from(show_node.node.0)] = Some(SignalOperation::show(signal_values));
    let drivers: [OperationDriver<SignalOperation, PORTS>; NODES] = operations
        .map(|operation| {
            OperationDriver::new(
                operation.ok_or_else(|| "missing installed signal operation".to_string())?,
            )
            .map_err(|error| format!("prepare operation driver: {error:?}"))
        })
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| "signal driver table width changed".to_string())?;
    let driver_capacity_before: usize = drivers
        .iter()
        .map(|driver: &OperationDriver<SignalOperation, PORTS>| {
            driver.operation().allocation_capacity()
        })
        .sum();

    let evidence_items = u16::try_from(
        configuration
            .count
            .checked_mul(15)
            .and_then(|value| value.checked_add(64))
            .ok_or_else(|| "kernel evidence item budget overflow".to_string())?,
    )
    .map_err(|_| "kernel evidence item budget overflow".to_string())?;
    let evidence_bytes = u32::from(evidence_items)
        .checked_mul(
            u32::try_from(core::mem::size_of::<conduit_kernel::KernelEvent>())
                .map_err(|_| "kernel evidence charge overflow".to_string())?,
        )
        .ok_or_else(|| "kernel evidence byte budget overflow".to_string())?;
    let evidence = HostedEvidenceLog::new(evidence_items, evidence_bytes)
        .map_err(|error| format!("kernel evidence store: {error:?}"))?;
    let node_specs = lowered
        .node_specs
        .try_into()
        .map_err(|_| "signal node table width changed".to_string())?;
    let cord_specs = lowered
        .cords
        .iter()
        .map(|cord| cord.spec)
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| "signal cord table width changed".to_string())?;
    let mut scheduler = SignalScheduler::new_with_host_operations(
        node_specs,
        cord_specs,
        routes,
        host_bindings,
        drivers,
        values,
        evidence,
    )
    .map_err(|error| format!("install signal scheduler: {error:?}"))?;

    let active_play = bind_active_play(
        &fragment.plan_id,
        &advertisement.host_id,
        &advertisement.boot_id,
        activation_sequence,
    );
    let mut receipts = Vec::with_capacity(count);
    let mut observations = Vec::with_capacity(count.saturating_add(1));
    let mut presentation_ids = Vec::with_capacity(count);
    loop {
        while let Some(request) = scheduler.next_host_request() {
            let input = scheduler
                .host_value(request.input.value)
                .map_err(|error| format!("read host-operation input: {error:?}"))?;
            if request.node == pulse_node.node {
                let duration = input
                    .try_into()
                    .map(u64::from_le_bytes)
                    .map_err(|_| "wait host operation input is not eight bytes".to_string())?;
                timer.wait(Duration::from_millis(duration));
            } else if request.node == show_node.node {
                let payload = ValuePayload {
                    value_kind: signal_value_kind(),
                    encoded: input.to_vec(),
                };
                let signal = decode_signal(&payload).map_err(|error| error.to_string())?;
                if signal.sequence != receipts.len() as u64 {
                    return Err(format!(
                        "expected signal sequence {}, received {}",
                        receipts.len(),
                        signal.sequence
                    ));
                }
                let presentation = bind_presentation(
                    &active_play.active_play_id,
                    &show_placement.placement_id,
                    signal.sequence,
                );
                writeln!(
                    output,
                    "signal {} {}",
                    signal.sequence,
                    if signal.level { "on" } else { "off" }
                )
                .map_err(|error| error.to_string())?;
                writeln!(
                    output,
                    "receipt signal placement={} sequence={} level={}",
                    show_placement.placement_id.as_str(),
                    signal.sequence,
                    signal.level
                )
                .map_err(|error| error.to_string())?;
                receipts.push(SignalReceipt {
                    placement_id: show_placement.placement_id.clone(),
                    sequence: signal.sequence,
                    level: signal.level,
                });
                let evidence = bind_evidence(
                    &advertisement.host_id,
                    &advertisement.boot_id,
                    Some(&active_play.active_play_id),
                    *next_evidence_sequence,
                );
                *next_evidence_sequence = next_evidence_sequence
                    .checked_add(1)
                    .ok_or_else(|| "host evidence sequence exhausted".to_string())?;
                observations.push(Observation {
                    evidence_id: evidence.evidence_id,
                    active_play_id: Some(active_play.active_play_id.clone()),
                    presentation_id: Some(presentation.presentation_id.clone()),
                    host_id: advertisement.host_id.clone(),
                    boot_id: advertisement.boot_id.clone(),
                    plan_id: Some(fragment.plan_id.clone()),
                    placement_id: Some(show_placement.placement_id.clone()),
                    connection_id: lowered
                        .identity
                        .connections
                        .first()
                        .map(|(_, connection)| connection.clone()),
                    kind: ObservationKind::ValuePresented { value: payload },
                });
                presentation_ids.push(presentation.presentation_id);
            } else {
                return Err(format!(
                    "unmapped kernel host request from node {}",
                    request.node.0
                ));
            }
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
                .map_err(|error| format!("complete host operation: {error:?}"))?;
        }
        match scheduler
            .step()
            .map_err(|error| format!("kernel step: {error:?}"))?
        {
            SchedulerStatus::Progress { .. } => {}
            SchedulerStatus::Complete => break,
            SchedulerStatus::Idle => return Err("kernel became idle before completion".to_string()),
            SchedulerStatus::Cancelled => return Err("kernel was cancelled".to_string()),
        }
    }
    if receipts.len() != count || scheduler.values().used_items() != 0 {
        return Err("kernel completed with missing receipts or retained values".to_string());
    }
    let driver_capacity_after: usize = scheduler
        .drivers()
        .iter()
        .map(|driver| driver.operation().allocation_capacity())
        .sum();
    if driver_capacity_after != driver_capacity_before {
        return Err("operation storage grew after activation".to_string());
    }
    let value_allocation_after = scheduler.values().allocation_capacities();
    if value_allocation_after != value_allocation_before {
        return Err("kernel value storage grew after activation".to_string());
    }
    let terminal_evidence = bind_evidence(
        &advertisement.host_id,
        &advertisement.boot_id,
        Some(&active_play.active_play_id),
        *next_evidence_sequence,
    );
    *next_evidence_sequence = next_evidence_sequence
        .checked_add(1)
        .ok_or_else(|| "host evidence sequence exhausted".to_string())?;
    observations.push(Observation {
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
    });

    Ok(StdRunReport {
        observations,
        receipts,
        kernel: Some(StdKernelExecutionReport {
            active_play_id: active_play.active_play_id,
            decisions: scheduler.decisions(),
            kernel_events: scheduler.evidence().len(),
            value_allocation_capacity_before: value_allocation_before,
            value_allocation_capacity_after: value_allocation_after,
            presentation_ids,
        }),
    })
}
