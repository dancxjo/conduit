//! Browser/WASM sink half of the live S4 distributed Signal checkpoint.

use super::{
    map_scheduler_error, write_common_frame, write_presentation_completion_frame,
    write_presentation_frame, FrameWriter, PreparedProjection, FRAME_CAPACITY, MAXIMUM_RECEIPTS,
    PORTS,
};
use conduit_core::{
    bind_active_play, bind_evidence, bind_presentation, CapabilityId, ConnectionProvider,
    OperationId, Plan, PlanFragment,
};
use conduit_kernel::scheduler::{
    FixedScheduler, HostOperationRequest, OperationDriver, RemoteIngressOutcome, SchedulerStatus,
};
use conduit_kernel::{
    BoundedValueRef, CordId, EvidenceError, EvidenceQuery, Failure, FailureCode,
    FixedHostOperationBindings, FixedRoutes, HostOperationDisposition, HostOperationId,
    HostOperationOutcome, HostedEvidenceLog, HostedValueStore, KernelEventKind, Operation,
    OperationAction, OperationInput, PortId, RemoteEndpointId, RequestId, ValueStorage,
};
use conduit_planner::{plan_with_link_bindings, PlacementChoice, PlacementChoices};
use conduit_runtime::lowering::{
    lower_plan_fragment, KernelExecutionIdentityMap, LoweredPlanFragment, RemoteCordDirection,
};
use conduit_signal::{
    decode_signal_bytes, distributed_browser_sink_advertisement,
    distributed_std_source_advertisement, distributed_websocket_link_binding,
    signal_profile_catalog, triple, DISTRIBUTED_MAXIMUM_FRAME_BYTES,
    DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS, SHOW_KIND, SIGNAL_ENCODED_LEN,
};
use conduit_wire::{
    decode_session_frame, encode_session_frame_into, SessionBinding, SessionMachine,
    SessionMessage, SessionRole, SessionTerminalDisposition,
};
use std::cell::RefCell;
use std::collections::BTreeMap;

const OUTPUT_NONE: i32 = 0;
const OUTPUT_SESSION: i32 = 1;
const OUTPUT_PRESENT: i32 = 3;
const STATUS_RUNNING: i32 = 0;
const STATUS_COMPLETE: i32 = 1;
const ERROR_NOT_STARTED: i32 = -101;
const ERROR_PREPARE: i32 = -102;
const ERROR_SESSION: i32 = -103;
const ERROR_KERNEL: i32 = -104;
const ERROR_PRESENTATION: i32 = -105;
const ERROR_CANCELLED: i32 = -106;
const ERROR_EVIDENCE: i32 = -107;
const ERROR_CAPACITY: i32 = -108;
const ROUTE_SLOTS: usize = 1;
const EVIDENCE_ITEMS: u16 = 256;

type SinkScheduler = FixedScheduler<
    OperationDriver<ShowOperation, PORTS>,
    HostedValueStore,
    HostedEvidenceLog,
    1,
    1,
    PORTS,
    1,
    ROUTE_SLOTS,
    1,
    1,
    1,
>;

thread_local! {
    static DISTRIBUTED: RefCell<Option<DistributedSink>> = const { RefCell::new(None) };
    static DISTRIBUTED_INPUT: RefCell<[u8; FRAME_CAPACITY]> = const { RefCell::new([0; FRAME_CAPACITY]) };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CapacitySeal {
    values: (usize, usize),
    evidence: usize,
    identity: (usize, usize, usize),
    projections: usize,
}

struct ShowOperation {
    next: usize,
    pending: Option<RequestId>,
}

impl ShowOperation {
    fn fail(detail: u16) -> OperationAction {
        OperationAction::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            detail,
        })
    }
}

impl Operation for ShowOperation {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.pending.is_none() => {
                let Ok(sequence) = u32::try_from(self.next) else {
                    return Self::fail(1);
                };
                let request = RequestId(0x8000_0000 | sequence);
                self.pending = Some(request);
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(0),
                    input: BoundedValueRef::new(value, SIGNAL_ENCODED_LEN)
                        .expect("remote Signal was admitted at its exact byte bound"),
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request)
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.output.is_none()
                    && outcome.failure.is_none() =>
            {
                self.pending = None;
                self.next += 1;
                OperationAction::Await
            }
            OperationInput::Closed { port: PortId(0) }
                if self.pending.is_none() && self.next == MAXIMUM_RECEIPTS =>
            {
                OperationAction::Complete
            }
            _ => Self::fail(2),
        }
    }
}

struct DistributedSink {
    scheduler: SinkScheduler,
    fragment: PlanFragment,
    lowered: LoweredPlanFragment,
    binding: SessionBinding,
    session: SessionMachine,
    identity: KernelExecutionIdentityMap,
    projections: Vec<PreparedProjection>,
    output: [u8; FRAME_CAPACITY],
    output_len: usize,
    output_kind: i32,
    expected_completion: [u8; FRAME_CAPACITY],
    expected_completion_len: usize,
    current: Option<(HostOperationRequest, usize)>,
    pending_delivery: Option<u64>,
    pending_pressure: Option<u64>,
    pending_failure_terminal: Option<SessionTerminalDisposition>,
    drive_after_delivery: bool,
    hold_first_value: bool,
    input_closed: bool,
    receipts: usize,
    pressure_retries: u32,
    complete: bool,
    peer_terminal: bool,
    error: i32,
    seal: CapacitySeal,
}

#[derive(Clone, Copy)]
enum PlanKind {
    StdBrowser,
    Triple,
}

fn exact_plan(kind: PlanKind) -> Result<Plan, i32> {
    if matches!(kind, PlanKind::Triple) {
        return triple::exact_plan()
            .map(|exact| exact.plan)
            .map_err(|_| ERROR_PREPARE);
    }
    let source = distributed_std_source_advertisement();
    let sink = distributed_browser_sink_advertisement();
    let form = conduit_form::parse(
        include_str!("../../../examples/signal-demo.form"),
        &signal_profile_catalog(),
    )
    .map_err(|_| ERROR_PREPARE)?;
    let placements = PlacementChoices {
        by_operation: BTreeMap::from([
            (
                OperationId::from("pulse"),
                PlacementChoice {
                    host_id: source.host_id.clone(),
                    capability_id: CapabilityId::from("pulse-1"),
                },
            ),
            (
                OperationId::from("show"),
                PlacementChoice {
                    host_id: sink.host_id.clone(),
                    capability_id: CapabilityId::from("dom-show-1"),
                },
            ),
        ]),
    };
    plan_with_link_bindings(
        &form,
        &[source, sink],
        &placements,
        &[ConnectionProvider::WebSocket],
        DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
        SIGNAL_ENCODED_LEN,
        &[distributed_websocket_link_binding()],
    )
    .map_err(|_| ERROR_PREPARE)
}

impl DistributedSink {
    fn prepare(evidence_override: Option<u16>, kind: PlanKind) -> Result<Self, i32> {
        let advertisement = match kind {
            PlanKind::StdBrowser => distributed_browser_sink_advertisement(),
            PlanKind::Triple => triple::browser_advertisement(),
        };
        let plan = exact_plan(kind)?;
        let fragment = plan
            .fragments
            .into_iter()
            .find(|fragment| fragment.host_id == advertisement.host_id)
            .ok_or(ERROR_PREPARE)?;
        let lowered = lower_plan_fragment(&fragment).map_err(|_| ERROR_PREPARE)?;
        if lowered.nodes.len() != 1
            || lowered.cords.len() != 1
            || lowered.remote_endpoints.len() != 1
            || lowered.remote_endpoints[0].direction != RemoteCordDirection::Ingress
            || lowered.host_operations.len() != 1
            || fragment.placements[0].kind_id.as_str() != SHOW_KIND
        {
            return Err(ERROR_PREPARE);
        }
        let remote = &lowered.remote_endpoints[0];
        let connection = fragment
            .connections
            .iter()
            .find(|connection| connection.connection_id == remote.connection_id)
            .ok_or(ERROR_PREPARE)?;
        let binding = SessionBinding::from_planned_connection(
            fragment.plan_id.clone(),
            remote.source_fragment_id.clone(),
            remote.sink_fragment_id.clone(),
            connection,
        )
        .map_err(|_| ERROR_SESSION)?;
        let mut routes = FixedRoutes::<ROUTE_SLOTS, 1>::new(PORTS as u16);
        routes.seal().map_err(|_| ERROR_PREPARE)?;
        let mut host_bindings = FixedHostOperationBindings::<1>::new(1);
        host_bindings
            .install(
                lowered.host_operations[0].node,
                lowered.host_operations[0].binding,
            )
            .map_err(|_| ERROR_PREPARE)?;
        host_bindings.seal().map_err(|_| ERROR_PREPARE)?;
        let values = HostedValueStore::new(1, SIGNAL_ENCODED_LEN, SIGNAL_ENCODED_LEN)
            .map_err(|_| ERROR_PREPARE)?;
        let evidence_items = evidence_override.unwrap_or(EVIDENCE_ITEMS);
        let evidence_bytes = u32::from(evidence_items)
            .checked_mul(core::mem::size_of::<conduit_kernel::KernelEvent>() as u32)
            .ok_or(ERROR_PREPARE)?;
        let evidence = HostedEvidenceLog::new(evidence_items, evidence_bytes.max(1))
            .map_err(|_| ERROR_PREPARE)?;
        let driver = OperationDriver::new(ShowOperation {
            next: 0,
            pending: None,
        })
        .map_err(|_| ERROR_PREPARE)?;
        let scheduler = SinkScheduler::new_with_host_operations(
            lowered
                .node_specs
                .clone()
                .try_into()
                .map_err(|_| ERROR_PREPARE)?,
            lowered
                .cords
                .iter()
                .map(|cord| cord.spec)
                .collect::<Vec<_>>()
                .try_into()
                .map_err(|_| ERROR_PREPARE)?,
            routes,
            host_bindings,
            [driver],
            values,
            evidence,
        )
        .map_err(|_| ERROR_PREPARE)?;
        let active_play =
            bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
        if active_play.active_play_id != binding.sink_active_play_id {
            return Err(ERROR_SESSION);
        }
        let mut identity = KernelExecutionIdentityMap::new(
            &lowered.identity,
            &active_play,
            MAXIMUM_RECEIPTS,
            MAXIMUM_RECEIPTS,
            MAXIMUM_RECEIPTS + 1,
        )
        .map_err(|_| ERROR_PREPARE)?;
        let show_node = lowered.nodes[0].node;
        let placement = &fragment.placements[0];
        let mut projections = Vec::with_capacity(MAXIMUM_RECEIPTS);
        for index in 0..MAXIMUM_RECEIPTS {
            let request = RequestId(0x8000_0000 | index as u32);
            identity
                .bind_request(&lowered.identity, show_node, request, HostOperationId(0))
                .map_err(|_| ERROR_PREPARE)?;
            let signal = conduit_signal::Signal {
                sequence: index as u64,
                level: index % 2 == 1,
            };
            let presentation = bind_presentation(
                &active_play.active_play_id,
                &placement.placement_id,
                index as u64,
            );
            let evidence = bind_evidence(
                &fragment.host_id,
                &fragment.boot_id,
                Some(&active_play.active_play_id),
                index as u64,
            );
            identity
                .bind_presentation(&lowered.identity, show_node, request, &presentation)
                .map_err(|_| ERROR_PREPARE)?;
            identity
                .bind_evidence(
                    &evidence,
                    Some(show_node),
                    Some(request),
                    Some(&presentation.presentation_id),
                )
                .map_err(|_| ERROR_PREPARE)?;
            projections.push(PreparedProjection {
                node: show_node,
                signal,
                presentation,
                evidence,
            });
        }
        let terminal = bind_evidence(
            &fragment.host_id,
            &fragment.boot_id,
            Some(&active_play.active_play_id),
            MAXIMUM_RECEIPTS as u64,
        );
        identity
            .bind_evidence(&terminal, None, None, None)
            .map_err(|_| ERROR_PREPARE)?;
        let session =
            SessionMachine::new(binding.clone(), SessionRole::Sink).map_err(|_| ERROR_SESSION)?;
        let seal = CapacitySeal {
            values: scheduler.values().allocation_capacities(),
            evidence: scheduler.evidence().allocation_capacity(),
            identity: identity.allocation_capacities(),
            projections: projections.capacity(),
        };
        let mut sink = Self {
            scheduler,
            fragment,
            lowered,
            binding,
            session,
            identity,
            projections,
            output: [0; FRAME_CAPACITY],
            output_len: 0,
            output_kind: OUTPUT_NONE,
            expected_completion: [0; FRAME_CAPACITY],
            expected_completion_len: 0,
            current: None,
            pending_delivery: None,
            pending_pressure: None,
            pending_failure_terminal: None,
            drive_after_delivery: false,
            hold_first_value: false,
            input_closed: false,
            receipts: 0,
            pressure_retries: 0,
            complete: false,
            peer_terminal: false,
            error: STATUS_RUNNING,
            seal,
        };
        let hello_binding = sink.binding.clone();
        sink.encode_session(hello_binding.hello_frame().message)?;
        Ok(sink)
    }

    fn remote(&self) -> (RemoteEndpointId, CordId) {
        let remote = &self.lowered.remote_endpoints[0];
        (remote.endpoint, remote.cord)
    }

    fn capacity_seal(&self) -> CapacitySeal {
        CapacitySeal {
            values: self.scheduler.values().allocation_capacities(),
            evidence: self.scheduler.evidence().allocation_capacity(),
            identity: self.identity.allocation_capacities(),
            projections: self.projections.capacity(),
        }
    }

    fn clear_output(&mut self) {
        self.output_len = 0;
        self.output_kind = OUTPUT_NONE;
    }

    fn encode_session(&mut self, message: SessionMessage<'_>) -> Result<(), i32> {
        self.clear_output();
        let binding = self.binding.clone();
        let frame = binding.frame(message);
        self.session
            .admit_outbound(frame)
            .map_err(|_| ERROR_SESSION)?;
        self.output_len = encode_session_frame_into(
            frame,
            &mut self.output,
            SIGNAL_ENCODED_LEN,
            DISTRIBUTED_MAXIMUM_FRAME_BYTES,
        )
        .map_err(|_| ERROR_SESSION)?;
        self.output_kind = OUTPUT_SESSION;
        Ok(())
    }

    fn fail_session(&mut self, code: u16, error: i32) -> Result<(), i32> {
        let _ = self.scheduler.cancel();
        self.error = error;
        self.pending_failure_terminal = Some(SessionTerminalDisposition::Failed);
        self.encode_session(SessionMessage::Failed { code })?;
        Err(error)
    }

    fn ingest(&mut self, bytes: &[u8]) -> Result<(), i32> {
        if self.error < 0 {
            return Err(self.error);
        }
        self.clear_output();
        let frame =
            decode_session_frame(bytes, SIGNAL_ENCODED_LEN, DISTRIBUTED_MAXIMUM_FRAME_BYTES)
                .map_err(|_| ERROR_SESSION)?;
        self.session
            .admit_inbound(frame)
            .map_err(|_| ERROR_SESSION)?;
        match frame.message {
            SessionMessage::Hello(_) => self.encode_session(SessionMessage::Ready),
            SessionMessage::Ready => {
                if self.session.is_active() {
                    Ok(())
                } else {
                    Err(ERROR_SESSION)
                }
            }
            SessionMessage::Offered { sequence, payload } => {
                let signal = decode_signal_bytes(payload).map_err(|_| ERROR_SESSION)?;
                if signal.sequence != sequence || signal.level != (sequence % 2 == 1) {
                    return self.fail_session(21, ERROR_SESSION);
                }
                let (endpoint, cord) = self.remote();
                let admission = match self
                    .scheduler
                    .admit_remote_input(endpoint, cord, sequence, payload)
                {
                    Ok(admission) => admission,
                    Err(conduit_kernel::scheduler::SchedulerError::Evidence(
                        EvidenceError::ItemCapacityExceeded | EvidenceError::ByteCapacityExceeded,
                    )) => return self.fail_session(23, ERROR_EVIDENCE),
                    Err(_) => return self.fail_session(24, ERROR_KERNEL),
                };
                match admission {
                    RemoteIngressOutcome::Accepted { .. } => {
                        self.pending_delivery = Some(sequence);
                        if sequence == 0 && self.pressure_retries == 0 {
                            self.hold_first_value = true;
                        }
                        self.encode_session(SessionMessage::Accepted { sequence })
                    }
                    RemoteIngressOutcome::Full { .. } => {
                        self.pending_pressure = Some(sequence);
                        self.pressure_retries += 1;
                        self.drive_scheduler()
                    }
                }
            }
            SessionMessage::InputClosed { final_sequence }
                if final_sequence == MAXIMUM_RECEIPTS as u64 =>
            {
                let (endpoint, cord) = self.remote();
                self.scheduler
                    .close_remote_input(endpoint, cord)
                    .map_err(|_| ERROR_KERNEL)?;
                self.input_closed = true;
                self.drive_scheduler()
            }
            SessionMessage::Terminal {
                disposition: SessionTerminalDisposition::Completed,
                final_sequence,
            } if final_sequence == MAXIMUM_RECEIPTS as u64 => {
                self.peer_terminal = true;
                Ok(())
            }
            SessionMessage::Cancelled { .. } | SessionMessage::Failed { .. } => {
                self.scheduler.cancel().map_err(|_| ERROR_KERNEL)?;
                self.error = ERROR_CANCELLED;
                Err(ERROR_CANCELLED)
            }
            _ => self.fail_session(22, ERROR_SESSION),
        }
    }

    fn advance(&mut self) -> Result<(), i32> {
        if let Some(disposition) = self.pending_failure_terminal.take() {
            let final_sequence = self.session.next_sequence();
            self.encode_session(SessionMessage::Terminal {
                disposition,
                final_sequence,
            })?;
            return Err(self.error);
        }
        if self.error < 0 {
            return Err(self.error);
        }
        self.clear_output();
        if let Some(sequence) = self.pending_delivery.take() {
            self.drive_after_delivery = !(sequence == 0 && self.hold_first_value);
            return self.encode_session(SessionMessage::Delivered { sequence });
        }
        if self.hold_first_value {
            self.hold_first_value = false;
            return Ok(());
        }
        if self.drive_after_delivery {
            self.drive_after_delivery = false;
            return self.drive_scheduler();
        }
        self.drive_scheduler()
    }

    fn drive_scheduler(&mut self) -> Result<(), i32> {
        self.clear_output();
        loop {
            if let Some(request) = self.scheduler.next_host_request() {
                return self.prepare_presentation(request);
            }
            match self.scheduler.step() {
                Ok(SchedulerStatus::Progress { .. }) => {}
                Ok(SchedulerStatus::Idle) => return Ok(()),
                Ok(SchedulerStatus::Complete) => {
                    let (_, cord) = self.remote();
                    if !self.input_closed
                        || self.receipts != MAXIMUM_RECEIPTS
                        || self.scheduler.values().used_items() != 0
                        || self.scheduler.cord_usage(cord).map_err(|_| ERROR_KERNEL)? != (0, 0)
                        || !self
                            .scheduler
                            .evidence()
                            .contains_kind(KernelEventKind::RemoteInputClosed)
                        || !self
                            .scheduler
                            .evidence()
                            .contains_kind(KernelEventKind::OperationCompleted)
                        || self.capacity_seal() != self.seal
                        || self.pressure_retries != 1
                    {
                        self.error = ERROR_CAPACITY;
                        return Err(ERROR_CAPACITY);
                    }
                    self.complete = true;
                    return self.encode_session(SessionMessage::Terminal {
                        disposition: SessionTerminalDisposition::Completed,
                        final_sequence: MAXIMUM_RECEIPTS as u64,
                    });
                }
                Ok(SchedulerStatus::Cancelled) => {
                    self.error = ERROR_CANCELLED;
                    return Err(ERROR_CANCELLED);
                }
                Err(conduit_kernel::scheduler::SchedulerError::Evidence(
                    EvidenceError::ItemCapacityExceeded | EvidenceError::ByteCapacityExceeded,
                )) => return self.fail_session(25, ERROR_EVIDENCE),
                Err(error) => return self.fail_session(26, map_scheduler_error(error)),
            }
        }
    }

    fn prepare_presentation(&mut self, request: HostOperationRequest) -> Result<(), i32> {
        let projection = self
            .projections
            .get(self.receipts)
            .ok_or(ERROR_PRESENTATION)?;
        let request_identity = self
            .identity
            .request(request.node, request.request)
            .ok_or(ERROR_PRESENTATION)?;
        let placement = &self.fragment.placements[0];
        let input = self
            .scheduler
            .host_value(request.input.value)
            .map_err(|_| ERROR_KERNEL)?;
        let signal = decode_signal_bytes(input).map_err(|_| ERROR_PRESENTATION)?;
        if projection.node != request.node
            || projection.signal != signal
            || request_identity.operation != request.operation
        {
            return Err(ERROR_PRESENTATION);
        }
        let active_play_id = self.binding.sink_active_play_id.clone();
        let mut output = FrameWriter::new(&mut self.output);
        write_common_frame(
            &mut output,
            OUTPUT_PRESENT as u8,
            &self.fragment,
            &active_play_id,
            request,
            &request_identity.contract_id,
            &placement.placement_id,
        )?;
        write_remote_identity_frame(&mut output, &self.binding)?;
        write_presentation_frame(&mut output, projection, input)?;
        self.output_len = output.len();
        let mut expected = FrameWriter::new(&mut self.expected_completion);
        write_common_frame(
            &mut expected,
            OUTPUT_PRESENT as u8,
            &self.fragment,
            &active_play_id,
            request,
            &request_identity.contract_id,
            &placement.placement_id,
        )?;
        write_remote_identity_frame(&mut expected, &self.binding)?;
        write_presentation_completion_frame(&mut expected, projection, input)?;
        self.expected_completion_len = expected.len();
        self.current = Some((request, self.receipts));
        self.output_kind = OUTPUT_PRESENT;
        Ok(())
    }

    fn complete_presentation(&mut self, completion: &[u8]) -> Result<(), i32> {
        let (request, projection) = self.current.take().ok_or(ERROR_PRESENTATION)?;
        if completion.len() != self.expected_completion_len + 1
            || completion[..self.expected_completion_len]
                != self.expected_completion[..self.expected_completion_len]
        {
            return self.fail_session(31, ERROR_PRESENTATION);
        }
        let success = match completion[self.expected_completion_len] {
            0 => false,
            1 => true,
            _ => return self.fail_session(33, ERROR_PRESENTATION),
        };
        if !success || projection != self.receipts {
            self.scheduler
                .complete_host_operation(
                    request.node,
                    request.request,
                    HostOperationOutcome {
                        disposition: HostOperationDisposition::Failed,
                        output: None,
                        failure: Some(Failure {
                            code: FailureCode::HostOperationFailed,
                            detail: 1,
                        }),
                    },
                )
                .map_err(|_| ERROR_KERNEL)?;
            return self.fail_session(32, ERROR_PRESENTATION);
        }
        self.scheduler
            .complete_host_operation(
                request.node,
                request.request,
                HostOperationOutcome {
                    disposition: HostOperationDisposition::Completed,
                    output: None,
                    failure: None,
                },
            )
            .map_err(|_| ERROR_KERNEL)?;
        self.receipts += 1;
        self.expected_completion_len = 0;
        self.clear_output();
        if let Some(sequence) = self.pending_pressure.take() {
            return self.encode_session(SessionMessage::Pressure { sequence });
        }
        self.drive_scheduler()
    }

    fn cancel(&mut self) -> Result<(), i32> {
        self.scheduler.cancel().map_err(|_| ERROR_KERNEL)?;
        self.error = ERROR_CANCELLED;
        self.pending_failure_terminal = Some(SessionTerminalDisposition::Cancelled);
        self.encode_session(SessionMessage::Cancelled { code: 41 })?;
        Err(ERROR_CANCELLED)
    }

    fn status(&self) -> i32 {
        if self.error < 0 {
            self.error
        } else if self.complete && self.peer_terminal && self.session.is_terminal() {
            STATUS_COMPLETE
        } else {
            STATUS_RUNNING
        }
    }
}

fn write_remote_identity_frame(
    writer: &mut FrameWriter<'_>,
    binding: &SessionBinding,
) -> Result<(), i32> {
    for identity in [
        binding.source_fragment_id.as_str(),
        binding.source.host_id.as_str(),
        binding.source.boot_id.as_str(),
        binding.source_active_play_id.as_str(),
        binding.source.endpoint_id.as_str(),
        binding.sink.endpoint_id.as_str(),
        binding.connection_id.as_str(),
        binding.link_binding_id.as_str(),
        binding.provider_instance_id.as_str(),
    ] {
        writer.text(identity)?;
    }
    Ok(())
}

#[path = "distributed_abi.rs"]
mod abi;

#[cfg(test)]
mod tests;
