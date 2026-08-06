//! Browser sink kernel and session logic for the S4 toggle-demo.
//!
//! `ToggleDistributedSink` prepares and drives the `presentation/show` fragment
//! receiving `Signal` values over the WebSocket cord from the std source.

use super::super::{
    map_scheduler_error, write_common_frame, write_presentation_completion_frame,
    write_presentation_frame, FrameWriter, PreparedProjection, FRAME_CAPACITY, MAXIMUM_RECEIPTS,
    PORTS,
};
use super::operation::{CapacitySeal, ToggleShowOperation};
use conduit_core::{
    bind_active_play, bind_evidence, bind_presentation, CapabilityId, ConnectionProvider,
    OperationId, Plan, PlanFragment,
};
use conduit_kernel::scheduler::{
    FixedScheduler, HostOperationRequest, OperationDriver, RemoteIngressOutcome, SchedulerStatus,
};
use conduit_kernel::{
    CordId, EvidenceError, EvidenceQuery, Failure, FailureCode, FixedHostOperationBindings,
    FixedRoutes, HostOperationDisposition, HostOperationId, HostOperationOutcome,
    HostedEvidenceLog, HostedValueStore, KernelEventKind, RemoteEndpointId, RequestId,
    ValueStorage,
};
use conduit_planner::{plan_with_link_bindings, PlacementChoice, PlacementChoices};
use conduit_runtime::lowering::{
    lower_plan_fragment, KernelExecutionIdentityMap, LoweredPlanFragment, RemoteCordDirection,
};
use conduit_signal::{
    decode_signal_bytes, distributed_toggle_browser_sink_advertisement,
    distributed_toggle_std_source_advertisement, distributed_toggle_websocket_link_binding,
    signal_profile_catalog, DISTRIBUTED_MAXIMUM_FRAME_BYTES, DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
    SHOW_KIND, SIGNAL_ENCODED_LEN,
};
use conduit_wire::{
    decode_session_frame, encode_session_frame_into, SessionBinding, SessionMachine,
    SessionMessage, SessionRole, SessionTerminalDisposition,
};
use std::collections::BTreeMap;

use super::{
    ERROR_CANCELLED, ERROR_CAPACITY, ERROR_EVIDENCE, ERROR_KERNEL, ERROR_PREPARE,
    ERROR_PRESENTATION, ERROR_SESSION, OUTPUT_NONE, OUTPUT_PRESENT, OUTPUT_SESSION,
    STATUS_COMPLETE, STATUS_RUNNING,
};

pub(super) const ROUTE_SLOTS: usize = 1;
pub(super) const EVIDENCE_ITEMS: u16 = 256;
const PRESSURE_HOLD_SEQUENCE: u64 = 1;

pub(super) type ToggleSinkScheduler = FixedScheduler<
    OperationDriver<ToggleShowOperation, PORTS>,
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

pub(super) struct ToggleDistributedSink {
    scheduler: ToggleSinkScheduler,
    pub(super) fragment: PlanFragment,
    pub(super) lowered: LoweredPlanFragment,
    pub(super) binding: SessionBinding,
    session: SessionMachine,
    identity: KernelExecutionIdentityMap,
    projections: Vec<PreparedProjection>,
    pub(super) output: [u8; FRAME_CAPACITY],
    pub(super) output_len: usize,
    pub(super) output_kind: i32,
    expected_completion: [u8; FRAME_CAPACITY],
    expected_completion_len: usize,
    current: Option<(HostOperationRequest, usize)>,
    pending_delivery: Option<u64>,
    delivery_after_presentation: Option<u64>,
    pending_pressure: Option<u64>,
    pending_failure_terminal: Option<SessionTerminalDisposition>,
    hold_for_pressure: bool,
    input_closed: bool,
    pub(super) receipts: usize,
    pressure_retries: u32,
    complete: bool,
    peer_terminal: bool,
    error: i32,
    pub(super) seal: CapacitySeal,
}

fn exact_toggle_plan() -> Result<Plan, i32> {
    let source = distributed_toggle_std_source_advertisement();
    let sink = distributed_toggle_browser_sink_advertisement();
    let form = conduit_form::parse(
        include_str!("../../../../examples/remote-toggle.form"),
        &signal_profile_catalog(),
    )
    .map_err(|_| ERROR_PREPARE)?;
    let placements = PlacementChoices {
        by_operation: BTreeMap::from([
            (
                OperationId::from("activate"),
                PlacementChoice {
                    host_id: source.host_id.clone(),
                    capability_id: CapabilityId::from("activate-1"),
                },
            ),
            (
                OperationId::from("toggle"),
                PlacementChoice {
                    host_id: source.host_id.clone(),
                    capability_id: CapabilityId::from("toggle-1"),
                },
            ),
            (
                OperationId::from("show"),
                PlacementChoice {
                    host_id: sink.host_id.clone(),
                    capability_id: CapabilityId::from("toggle-dom-show-1"),
                },
            ),
        ]),
    };
    plan_with_link_bindings(
        &form,
        &[source, sink],
        &placements,
        &[ConnectionProvider::Local, ConnectionProvider::WebSocket],
        DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
        SIGNAL_ENCODED_LEN,
        &[distributed_toggle_websocket_link_binding()],
    )
    .map_err(|_| ERROR_PREPARE)
}

impl ToggleDistributedSink {
    pub(super) fn prepare(evidence_override: Option<u16>) -> Result<Self, i32> {
        let advertisement = distributed_toggle_browser_sink_advertisement();
        let plan = exact_toggle_plan()?;
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
        let driver = OperationDriver::new(ToggleShowOperation {
            next: 0,
            pending: None,
        })
        .map_err(|_| ERROR_PREPARE)?;
        let scheduler = ToggleSinkScheduler::new_with_host_operations(
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
            // Toggle signals: level flips each time starting from !initial (=true for initial=false)
            let signal = conduit_signal::Signal {
                sequence: index as u64,
                level: index % 2 == 0,
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
            delivery_after_presentation: None,
            pending_pressure: None,
            pending_failure_terminal: None,
            hold_for_pressure: false,
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

    pub(super) fn capacity_seal(&self) -> CapacitySeal {
        CapacitySeal {
            values: self.scheduler.values().allocation_capacities(),
            evidence: self.scheduler.evidence().allocation_capacity(),
            identity: self.identity.allocation_capacities(),
            projections: self.projections.capacity(),
        }
    }

    pub(super) fn clear_output(&mut self) {
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

    pub(super) fn ingest(&mut self, bytes: &[u8]) -> Result<(), i32> {
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
                if signal.sequence != sequence {
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
                        if sequence == PRESSURE_HOLD_SEQUENCE && self.pressure_retries == 0 {
                            self.hold_for_pressure = true;
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

    pub(super) fn advance(&mut self) -> Result<(), i32> {
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
            if sequence == PRESSURE_HOLD_SEQUENCE && self.hold_for_pressure {
                return self.encode_session(SessionMessage::Delivered { sequence });
            }
            self.delivery_after_presentation = Some(sequence);
            return self.drive_scheduler();
        }
        if self.hold_for_pressure {
            self.hold_for_pressure = false;
            return Ok(());
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

    pub(super) fn prepare_presentation(
        &mut self,
        request: HostOperationRequest,
    ) -> Result<(), i32> {
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
        write_presentation_completion_frame(&mut expected, projection, input)?;
        self.expected_completion_len = expected.len();
        self.current = Some((request, self.receipts));
        self.output_kind = OUTPUT_PRESENT;
        Ok(())
    }

    pub(super) fn complete_presentation(&mut self, completion: &[u8]) -> Result<(), i32> {
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
        if let Some(sequence) = self.delivery_after_presentation.take() {
            return self.encode_session(SessionMessage::Delivered { sequence });
        }
        if let Some(sequence) = self.pending_pressure.take() {
            return self.encode_session(SessionMessage::Pressure { sequence });
        }
        self.drive_scheduler()
    }

    pub(super) fn cancel(&mut self) -> Result<(), i32> {
        self.scheduler.cancel().map_err(|_| ERROR_KERNEL)?;
        self.error = ERROR_CANCELLED;
        self.pending_failure_terminal = Some(SessionTerminalDisposition::Cancelled);
        self.encode_session(SessionMessage::Cancelled { code: 41 })?;
        Err(ERROR_CANCELLED)
    }

    pub(super) fn status(&self) -> i32 {
        if self.error < 0 {
            self.error
        } else if self.complete && self.peer_terminal && self.session.is_terminal() {
            STATUS_COMPLETE
        } else {
            STATUS_RUNNING
        }
    }
}
