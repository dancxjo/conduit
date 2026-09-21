//! Installed hosted-kernel profile for the exact two-node Signal form.

use super::{SignalReceipt, StdKernelExecutionReport, StdRunReport, TimerAdapter};
use conduit_core::{
    bind_active_play, bind_presentation, bind_sign, HostAdvertisement, Observation,
    ObservationKind, PlanFragment, TerminalDisposition, ValuePayload,
};
use conduit_kernel::scheduler::{
    FixedScheduler, HostCallRequest, SchedulerStatus, StepBack, StepInputBytes, StepIo, StepOutcome,
};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, FixedHostCallBindings, FixedRoutes, HostCallDisposition,
    HostCallId, HostCallOutcome, HostedSignLog, HostedValueStore, PortId, RequestId, SignSink,
    ValueRef, ValueStorage,
};
use conduit_plan_lowering::lowering::{
    lower_plan_fragment, KernelExecutionIdentityMap, FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
};
use conduit_signal::{
    decode_signal_bytes, encode_signal, parse_pulse_configuration, Signal, PULSE_KIND, SHOW_KIND,
    SIGNAL_ENCODED_LEN,
};
use std::io::Write;
use std::time::Duration;

const PORTS: usize = FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const ROUTE_SLOTS: usize = 4 * PORTS;

/// Exact platform manifestation boundary for the installed Signal kernel.
/// Implementations perform only the selected presentation effect; the kernel
/// retains sequence, level, Presentation, Sign, and terminal truth.
pub trait SignalManifestation {
    fn manifest(&mut self, signal: &Signal, operator_output: &mut dyn Write) -> Result<(), String>;
}

struct TextSignalManifestation;

impl SignalManifestation for TextSignalManifestation {
    fn manifest(&mut self, signal: &Signal, operator_output: &mut dyn Write) -> Result<(), String> {
        writeln!(
            operator_output,
            "signal {} {}",
            signal.sequence,
            if signal.level { "on" } else { "off" }
        )
        .map_err(|error| error.to_string())
    }
}

type SignalScheduler<
    const NODES: usize,
    const CORDS: usize,
    const QUEUE_SLOTS: usize,
    const ROUTE_TARGETS: usize,
    const HOST_BINDING_SLOTS: usize,
    const PENDING_REQUESTS: usize,
> = FixedScheduler<
    SignalBack,
    HostedValueStore,
    HostedSignLog,
    NODES,
    CORDS,
    PORTS,
    QUEUE_SLOTS,
    ROUTE_SLOTS,
    ROUTE_TARGETS,
    HOST_BINDING_SLOTS,
    PENDING_REQUESTS,
>;

struct PreparedSignalProjection {
    node: conduit_kernel::NodeId,
    signal: Signal,
    presentation: conduit_core::PresentationIdentity,
    sign: conduit_core::SignIdentity,
    payload: ValuePayload,
    connection_id: Option<conduit_core::ConnectionId>,
}

enum SignalBack {
    Pulse {
        values: Vec<ValueRef>,
        waits: Vec<ValueRef>,
        next: usize,
        pending: Option<RequestId>,
        emitted: bool,
    },
    Show {
        expected: Vec<ValueRef>,
        next: usize,
        pending: Option<RequestId>,
    },
}

impl SignalBack {
    fn pulse(values: Vec<ValueRef>, waits: Vec<ValueRef>) -> Self {
        Self::Pulse {
            values,
            waits,
            next: 0,
            pending: None,
            emitted: false,
        }
    }

    fn show(expected: Vec<ValueRef>) -> Self {
        Self::Show {
            expected,
            next: 0,
            pending: None,
        }
    }

    fn fail(code: FailureCode, detail: u16) -> StepOutcome {
        StepOutcome::Fail(Failure { code, detail })
    }

    fn allocation_capacity(&self) -> usize {
        match self {
            Self::Pulse { values, waits, .. } => values.capacity() + waits.capacity(),
            Self::Show { expected, .. } => expected.capacity(),
        }
    }
}

impl StepBack<PORTS> for SignalBack {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        _input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        match self {
            Self::Pulse {
                values,
                waits,
                next,
                pending,
                emitted,
            } => {
                if let Some(expected) = *pending {
                    let Some((request, outcome)) = io.host_completion() else {
                        return StepOutcome::Await;
                    };
                    let Some(value) = values.get(*next).copied() else {
                        return Self::fail(FailureCode::InvalidLifecycle, 1);
                    };
                    if request != expected
                        || outcome.disposition != HostCallDisposition::Completed
                        || outcome.output.is_some()
                        || outcome.failure.is_some()
                        || !io.output_ready(PortId(0))
                        || io.consume_host_completion().is_err()
                        || io.send(PortId(0), value).is_err()
                    {
                        return Self::fail(FailureCode::InvalidLifecycle, 3);
                    }
                    *pending = None;
                    *emitted = true;
                    return StepOutcome::Progress;
                }
                if !*emitted {
                    let Some(value) = values.get(*next).copied() else {
                        return StepOutcome::Complete;
                    };
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    if io.send(PortId(0), value).is_err() {
                        return Self::fail(FailureCode::InvalidLifecycle, 3);
                    }
                    *emitted = true;
                    return StepOutcome::Progress;
                }
                *next += 1;
                if *next >= values.len() {
                    return StepOutcome::Complete;
                }
                let Some(wait) = waits.get(*next - 1).copied() else {
                    return Self::fail(FailureCode::InvalidLifecycle, 5);
                };
                let Ok(sequence) = u32::try_from(*next) else {
                    return Self::fail(FailureCode::InvalidLifecycle, 6);
                };
                let request = RequestId(sequence);
                let input =
                    BoundedValueRef::new(wait, 8).expect("sealed wait value is exactly admitted");
                if io.request_host_call(request, HostCallId(0), input).is_err() {
                    return Self::fail(FailureCode::InvalidLifecycle, 3);
                }
                *pending = Some(request);
                *emitted = false;
                StepOutcome::Progress
            }
            Self::Show {
                expected,
                next,
                pending,
            } => {
                if let Some(expected_request) = *pending {
                    let Some((request, outcome)) = io.host_completion() else {
                        return StepOutcome::Await;
                    };
                    if request != expected_request
                        || outcome.disposition != HostCallDisposition::Completed
                        || outcome.output.is_some()
                        || outcome.failure.is_some()
                        || io.consume_host_completion().is_err()
                    {
                        return Self::fail(FailureCode::InvalidInput, 4);
                    }
                    *pending = None;
                    *next += 1;
                    return StepOutcome::Progress;
                }
                if let Some(value) = io.input(PortId(0)) {
                    if expected.get(*next) != Some(&value) {
                        return Self::fail(FailureCode::InvalidInput, 4);
                    }
                    let Ok(sequence) = u32::try_from(*next) else {
                        return Self::fail(FailureCode::InvalidLifecycle, 2);
                    };
                    let request = RequestId(0x8000_0000 | sequence);
                    let input = BoundedValueRef::new(value, SIGNAL_ENCODED_LEN)
                        .expect("sealed Signal is exactly admitted");
                    if io.consume(PortId(0)).is_err()
                        || io.request_host_call(request, HostCallId(0), input).is_err()
                    {
                        return Self::fail(FailureCode::InvalidInput, 4);
                    }
                    *pending = Some(request);
                    return StepOutcome::Progress;
                }
                if io.input_closed(PortId(0)) {
                    if *next != expected.len() || io.consume_closed(PortId(0)).is_err() {
                        return Self::fail(FailureCode::InvalidInput, 4);
                    }
                    return StepOutcome::Complete;
                }
                StepOutcome::Await
            }
        }
    }

    fn cancel(&mut self) {
        match self {
            Self::Pulse { pending, .. } | Self::Show { pending, .. } => *pending = None,
        }
    }
}

pub(super) fn run_signal_fragment<W: Write, T: TimerAdapter>(
    advertisement: &HostAdvertisement,
    fragment: &PlanFragment,
    play_sequence: u64,
    next_sign_sequence: &mut u64,
    output: &mut W,
    timer: &mut T,
) -> Result<StdRunReport, String> {
    run_signal_fragment_with_manifestation(
        advertisement,
        fragment,
        play_sequence,
        next_sign_sequence,
        output,
        timer,
        &mut TextSignalManifestation,
    )
}

pub(super) fn run_signal_fragment_with_manifestation<
    W: Write,
    T: TimerAdapter,
    M: SignalManifestation,
>(
    advertisement: &HostAdvertisement,
    fragment: &PlanFragment,
    play_sequence: u64,
    next_sign_sequence: &mut u64,
    output: &mut W,
    timer: &mut T,
    manifestation: &mut M,
) -> Result<StdRunReport, String> {
    match (fragment.placements.len(), fragment.connections.len()) {
        (2, 1) => run_signal_profile::<W, T, M, 2, 1, 4, 1, 2, 2>(
            advertisement,
            fragment,
            play_sequence,
            next_sign_sequence,
            output,
            timer,
            manifestation,
        ),
        (4, 3) => run_signal_profile::<W, T, M, 4, 3, 12, 3, 4, 4>(
            advertisement,
            fragment,
            play_sequence,
            next_sign_sequence,
            output,
            timer,
            manifestation,
        ),
        _ => Err("fragment does not match an installed std signal kernel profile".to_string()),
    }
}

#[allow(clippy::too_many_arguments)]
fn run_signal_profile<
    W: Write,
    T: TimerAdapter,
    M: SignalManifestation,
    const NODES: usize,
    const CORDS: usize,
    const QUEUE_SLOTS: usize,
    const ROUTE_TARGETS: usize,
    const HOST_BINDING_SLOTS: usize,
    const PENDING_REQUESTS: usize,
>(
    advertisement: &HostAdvertisement,
    fragment: &PlanFragment,
    play_sequence: u64,
    next_sign_sequence: &mut u64,
    output: &mut W,
    timer: &mut T,
    manifestation: &mut M,
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
        || lowered.host_calls.len() != HOST_BINDING_SLOTS
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
    let mut show_nodes = lowered
        .nodes
        .iter()
        .filter(|node| {
            fragment.placements[usize::from(node.node.0)]
                .kind_id
                .as_str()
                == SHOW_KIND
        })
        .collect::<Vec<_>>();
    if show_nodes.len() != NODES.saturating_sub(1)
        || show_nodes.iter().any(|show| show.node == pulse_node.node)
    {
        return Err("signal kernel profile has incomplete or duplicate show nodes".to_string());
    }
    show_nodes.sort_by_key(|show| {
        (usize::from(show.node.0) + NODES - usize::from(pulse_node.node.0)) % NODES
    });
    let pulse_placement = &fragment.placements[usize::from(pulse_node.node.0)];
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
    let mut host_bindings = FixedHostCallBindings::<HOST_BINDING_SLOTS>::new(1);
    for operation in &lowered.host_calls {
        host_bindings
            .install(operation.node, operation.binding)
            .map_err(|error| format!("install host-call: {error:?}"))?;
    }
    host_bindings
        .seal()
        .map_err(|error| format!("seal Host Calls: {error:?}"))?;

    let mut backs: [Option<SignalBack>; NODES] = core::array::from_fn(|_| None);
    backs[usize::from(pulse_node.node.0)] =
        Some(SignalBack::pulse(signal_values.clone(), wait_values));
    for show_node in &show_nodes {
        backs[usize::from(show_node.node.0)] = Some(SignalBack::show(signal_values.clone()));
    }
    let backs: [SignalBack; NODES] = backs
        .map(|back| back.ok_or_else(|| "missing installed Signal Back".to_string()))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| "Signal Back table width changed".to_string())?;
    let back_capacity_before: usize = backs.iter().map(SignalBack::allocation_capacity).sum();

    let sign_events_per_signal = 10_u64
        .checked_add(
            u64::try_from(show_nodes.len())
                .ok()
                .and_then(|shows| shows.checked_mul(8))
                .ok_or_else(|| "kernel sign item budget overflow".to_string())?,
        )
        .ok_or_else(|| "kernel sign item budget overflow".to_string())?;
    let sign_items = u16::try_from(
        configuration
            .count
            .checked_mul(sign_events_per_signal)
            .and_then(|value| value.checked_add(64))
            .ok_or_else(|| "kernel sign item budget overflow".to_string())?,
    )
    .map_err(|_| "kernel sign item budget overflow".to_string())?;
    let sign_bytes = u32::from(sign_items)
        .checked_mul(
            u32::try_from(core::mem::size_of::<conduit_kernel::KernelEvent>())
                .map_err(|_| "kernel sign charge overflow".to_string())?,
        )
        .ok_or_else(|| "kernel sign byte budget overflow".to_string())?;
    let sign = HostedSignLog::new(sign_items, sign_bytes)
        .map_err(|error| format!("kernel sign store: {error:?}"))?;
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
    let mut scheduler = SignalScheduler::<
        NODES,
        CORDS,
        QUEUE_SLOTS,
        ROUTE_TARGETS,
        HOST_BINDING_SLOTS,
        PENDING_REQUESTS,
    >::new_with_host_calls(
        node_specs,
        cord_specs,
        routes,
        host_bindings,
        backs,
        values,
        sign,
    )
    .map_err(|error| format!("install signal scheduler: {error:?}"))?;

    let active_play = bind_active_play(
        &fragment.plan_id,
        &advertisement.host_id,
        &advertisement.boot_id,
        play_sequence,
    );
    let presentation_capacity = count
        .checked_mul(show_nodes.len())
        .ok_or_else(|| "presentation identity capacity overflow".to_string())?;
    let request_capacity = presentation_capacity
        .checked_add(wait_count)
        .ok_or_else(|| "execution request identity capacity overflow".to_string())?;
    let sign_capacity = presentation_capacity
        .checked_add(1)
        .ok_or_else(|| "execution sign identity capacity overflow".to_string())?;
    let mut execution_identity = KernelExecutionIdentityMap::new(
        &lowered.identity,
        &active_play,
        request_capacity,
        presentation_capacity,
        sign_capacity,
    )
    .map_err(|error| format!("prepare execution identity map: {error:?}"))?;
    let identity_capacity_before = execution_identity.allocation_capacities();
    let mut receipts = Vec::with_capacity(presentation_capacity);
    let mut observations = Vec::with_capacity(sign_capacity);
    let mut presentation_ids = Vec::with_capacity(presentation_capacity);
    let mut dispatched_requests = Vec::<HostCallRequest>::with_capacity(request_capacity);
    let mut manifested_requests = Vec::<HostCallRequest>::with_capacity(presentation_capacity);
    let sign_sequence_start = *next_sign_sequence;
    let sign_count =
        u64::try_from(sign_capacity).map_err(|_| "execution sign count overflow".to_string())?;
    let sign_sequence_end = sign_sequence_start
        .checked_add(sign_count)
        .ok_or_else(|| "host sign sequence exhausted".to_string())?;
    let mut prepared_projections = Vec::with_capacity(presentation_capacity);
    for index in 0..count {
        let sequence =
            u64::try_from(index).map_err(|_| "signal projection sequence overflow".to_string())?;
        let signal = Signal {
            sequence,
            level: if sequence.is_multiple_of(2) {
                configuration.initial_level
            } else {
                !configuration.initial_level
            },
        };
        for show_node in &show_nodes {
            let show_placement = &fragment.placements[usize::from(show_node.node.0)];
            let presentation = bind_presentation(
                &active_play.active_play_id,
                &show_placement.placement_id,
                sequence,
            );
            let sign_offset = u64::try_from(prepared_projections.len())
                .map_err(|_| "host sign sequence exhausted".to_string())?;
            let sign = bind_sign(
                &advertisement.host_id,
                &advertisement.boot_id,
                Some(&active_play.active_play_id),
                sign_sequence_start
                    .checked_add(sign_offset)
                    .ok_or_else(|| "host sign sequence exhausted".to_string())?,
            );
            let connection_id = fragment
                .connections
                .iter()
                .find(|connection| connection.sink_placement_id == show_placement.placement_id)
                .map(|connection| connection.connection_id.clone());
            prepared_projections.push(PreparedSignalProjection {
                node: show_node.node,
                payload: encode_signal(&signal),
                signal: signal.clone(),
                presentation,
                sign,
                connection_id,
            });
        }
    }
    let terminal_sign = bind_sign(
        &advertisement.host_id,
        &advertisement.boot_id,
        Some(&active_play.active_play_id),
        sign_sequence_end - 1,
    );
    // SEALED PROFILE PLAY START BEGIN: numeric tables and preallocated capture only.
    #[cfg(test)]
    let play_start_probe = crate::allocation_probe::begin();
    loop {
        while let Some(request) = scheduler.next_host_request() {
            dispatched_requests.push(request);
            let input = scheduler
                .host_value(request.input.value)
                .map_err(|error| format!("read Host Call input: {error:?}"))?;
            if request.node == pulse_node.node {
                let duration = input
                    .try_into()
                    .map(u64::from_le_bytes)
                    .map_err(|_| "wait Host Call input is not eight bytes".to_string())?;
                timer.wait(Duration::from_millis(duration));
            } else {
                let expected = prepared_projections
                    .get(manifested_requests.len())
                    .ok_or_else(|| "signal manifestation exceeded sealed profile".to_string())?;
                let signal = decode_signal_bytes(input).map_err(|error| error.to_string())?;
                if request.node != expected.node || signal != expected.signal {
                    return Err(format!(
                        "expected node {} signal {:?}, received node {} signal {:?}",
                        expected.node.0, expected.signal, request.node.0, signal
                    ));
                }
                manifestation.manifest(&signal, output)?;
                writeln!(
                    output,
                    "receipt signal placement={} sequence={} level={}",
                    expected.presentation.placement_id.as_str(),
                    signal.sequence,
                    signal.level
                )
                .map_err(|error| error.to_string())?;
                manifested_requests.push(request);
            }
            scheduler
                .complete_host_call(
                    request.node,
                    request.request,
                    HostCallOutcome {
                        disposition: HostCallDisposition::Completed,
                        output: None,
                        failure: None,
                    },
                )
                .map_err(|error| format!("complete host-call: {error:?}"))?;
        }
        match scheduler
            .step()
            .map_err(|error| format!("kernel step: {error:?}"))?
        {
            SchedulerStatus::Progress { .. } => {}
            SchedulerStatus::Drained => break,
            SchedulerStatus::Idle => return Err("kernel became idle before completion".to_string()),
            SchedulerStatus::Cancelled => return Err("kernel was cancelled".to_string()),
        }
    }
    // SEALED PROFILE PLAY START END: hosted identity/observation projection resumes.
    #[cfg(test)]
    let post_play_start_allocations = play_start_probe.finish();
    if dispatched_requests.len() != request_capacity
        || manifested_requests.len() != presentation_capacity
        || scheduler.values().used_items() != 0
    {
        return Err("kernel completed with missing receipts or retained values".to_string());
    }
    let back_capacity_after: usize = scheduler
        .drivers()
        .iter()
        .map(SignalBack::allocation_capacity)
        .sum();
    if back_capacity_after != back_capacity_before {
        return Err("Back storage grew after Play start".to_string());
    }
    let value_allocation_after = scheduler.values().allocation_capacities();
    if value_allocation_after != value_allocation_before {
        return Err("kernel value storage grew after Play start".to_string());
    }
    for request in &dispatched_requests {
        execution_identity
            .bind_request(
                &lowered.identity,
                request.node,
                request.request,
                request.call,
            )
            .map_err(|error| format!("bind host request identity: {error:?}"))?;
    }
    for (request, prepared) in manifested_requests.into_iter().zip(prepared_projections) {
        execution_identity
            .bind_presentation(
                &lowered.identity,
                request.node,
                request.request,
                &prepared.presentation,
            )
            .map_err(|error| format!("bind presentation identity: {error:?}"))?;
        execution_identity
            .bind_sign(
                &prepared.sign,
                Some(request.node),
                Some(request.request),
                Some(&prepared.presentation.presentation_id),
            )
            .map_err(|error| format!("bind presentation sign identity: {error:?}"))?;
        receipts.push(SignalReceipt {
            placement_id: prepared.presentation.placement_id.clone(),
            sequence: prepared.signal.sequence,
            level: prepared.signal.level,
        });
        observations.push(Observation {
            sign_id: prepared.sign.sign_id,
            active_play_id: Some(active_play.active_play_id.clone()),
            presentation_id: Some(prepared.presentation.presentation_id.clone()),
            host_id: advertisement.host_id.clone(),
            boot_id: advertisement.boot_id.clone(),
            plan_id: Some(fragment.plan_id.clone()),
            placement_id: Some(prepared.presentation.placement_id.clone()),
            connection_id: prepared.connection_id,
            kind: ObservationKind::ValuePresented {
                value: prepared.payload,
            },
        });
        presentation_ids.push(prepared.presentation.presentation_id);
    }
    *next_sign_sequence = sign_sequence_end;
    execution_identity
        .bind_sign(&terminal_sign, None, None, None)
        .map_err(|error| format!("bind terminal sign identity: {error:?}"))?;
    observations.push(Observation {
        sign_id: terminal_sign.sign_id,
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
    if execution_identity.lengths() != (request_capacity, presentation_capacity, sign_capacity)
        || execution_identity.allocation_capacities() != identity_capacity_before
    {
        return Err("execution identity map is incomplete or grew after Play start".to_string());
    }

    Ok(StdRunReport {
        observations,
        receipts,
        control_receipts: Vec::new(),
        speech_synthesis: Vec::new(),
        speech_recognition: Vec::new(),
        microphone: Vec::new(),
        kernel: Some(StdKernelExecutionReport {
            active_play_id: active_play.active_play_id,
            decisions: scheduler.decisions(),
            kernel_events: scheduler.signs().len(),
            kernel_sign: scheduler.signs().events().collect(),
            value_allocation_capacity_before: value_allocation_before,
            value_allocation_capacity_after: value_allocation_after,
            presentation_ids,
            playback: Vec::new(),
            wav_artifacts: Vec::new(),
            midi_input: Vec::new(),
            midi_output: Vec::new(),
            identity: execution_identity,
            #[cfg(test)]
            post_play_start_allocations,
        }),
    })
}
