//! Exact std source half of the live S4 std-to-browser Signal proof.

use crate::websocket::{NativeWebSocketCarrier, NativeWebSocketListener};
use conduit_core::{
    bind_active_play, CapabilityId, ConnectionProvider, HostAdvertisement, OperationId, Plan,
    PlanFragment,
};
use conduit_kernel::scheduler::{
    FixedScheduler, HostOperationRequest, OperationDriver, SchedulerStatus,
};
use conduit_kernel::{
    BoundedValueRef, CordId, EvidenceQuery, Failure, FailureCode, FixedHostOperationBindings,
    FixedRoutes, HostOperationDisposition, HostOperationId, HostOperationOutcome,
    HostedEvidenceLog, HostedValueStore, KernelEventKind, Operation, OperationAction,
    OperationInput, PortId, RemoteEndpointId, RequestId, ValueRef, ValueStorage,
};
#[cfg(test)]
use conduit_planner::plan_with_link_bindings;
use conduit_planner::{
    plan_expanded_canonical_with_options, PlacementChoice, PlacementChoices, PlanningOptions,
};
use conduit_runtime::lowering::{
    lower_plan_fragment, KernelExecutionIdentityMap, LoweredPlanFragment, RemoteCordDirection,
    MAXIMUM_KERNEL_PORTS_PER_NODE,
};
use conduit_signal::{
    distributed_browser_sink_advertisement, distributed_std_source_advertisement,
    distributed_websocket_link_binding, encode_signal, parse_pulse_configuration,
    signal_profile_catalog, Signal, DISTRIBUTED_MAXIMUM_FRAME_BYTES,
    DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS, SIGNAL_ENCODED_LEN,
};
use conduit_wire::{
    decode_session_frame, encode_session_frame_into, SessionBinding, SessionMachine,
    SessionMessage, SessionRole, SessionTerminalDisposition,
};
use std::collections::BTreeMap;
use std::io::Write;
use std::thread;
use std::time::Duration;

const PORTS: usize = MAXIMUM_KERNEL_PORTS_PER_NODE;
const ROUTE_SLOTS: usize = PORTS;
const MAXIMUM_VALUES: usize = 16;
const MAXIMUM_WAITS: usize = 15;
const MAXIMUM_STORED_ITEMS: u16 = (MAXIMUM_VALUES + MAXIMUM_WAITS) as u16;
const MAXIMUM_STORED_BYTES: u32 =
    MAXIMUM_VALUES as u32 * SIGNAL_ENCODED_LEN + MAXIMUM_WAITS as u32 * 8;
const EVIDENCE_ITEMS: u16 = 256;

type SourceScheduler = FixedScheduler<
    OperationDriver<PulseOperation, PORTS>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DistributedSignalPlan {
    pub source_advertisement: HostAdvertisement,
    pub sink_advertisement: HostAdvertisement,
    pub plan: Plan,
}

pub fn exact_distributed_signal_plan() -> Result<DistributedSignalPlan, String> {
    let source_advertisement = distributed_std_source_advertisement();
    let sink_advertisement = distributed_browser_sink_advertisement();
    let source = include_str!("../../../examples/signal-demo.conduit");
    let syntax = conduit_form::parse_syntax_document(source);
    let checked =
        conduit_form::check_syntax_document(&syntax, &conduit_signal::signal_startup_catalog())
            .map_err(|error| format!("{}: {}", error.code, error.message))?;
    let form =
        conduit_form::expand_canonical_form(&checked, "signal-demo", &signal_profile_catalog())
            .map_err(|error| error.to_string())?;
    let placements = PlacementChoices {
        by_operation: BTreeMap::from([
            (
                OperationId::from("signal-demo/pulse"),
                PlacementChoice {
                    host_id: source_advertisement.host_id.clone(),
                    capability_id: CapabilityId::from("pulse-1"),
                },
            ),
            (
                OperationId::from("signal-demo/show"),
                PlacementChoice {
                    host_id: sink_advertisement.host_id.clone(),
                    capability_id: CapabilityId::from("dom-show-1"),
                },
            ),
        ]),
    };
    let link = distributed_websocket_link_binding();
    let plan = plan_expanded_canonical_with_options(
        &form,
        &[source_advertisement.clone(), sink_advertisement.clone()],
        &placements,
        &[ConnectionProvider::WebSocket],
        PlanningOptions {
            connection_providers: &BTreeMap::new(),
            connection_item_capacity: DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
            connection_byte_capacity: SIGNAL_ENCODED_LEN,
            authority_grants: &[],
            link_bindings: &[link],
        },
    )
    .map_err(|error| error.to_string())?;
    Ok(DistributedSignalPlan {
        source_advertisement,
        sink_advertisement,
        plan,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CapacitySeal {
    values: (usize, usize),
    evidence: usize,
    driver: usize,
    identity: (usize, usize, usize),
}

struct PulseOperation {
    values: Vec<ValueRef>,
    waits: Vec<ValueRef>,
    next: usize,
    pending: Option<RequestId>,
}

impl PulseOperation {
    fn allocation_capacity(&self) -> usize {
        self.values.capacity() + self.waits.capacity()
    }

    fn fail(detail: u16) -> OperationAction {
        OperationAction::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            detail,
        })
    }
}

impl Operation for PulseOperation {
    fn start(&mut self) -> OperationAction {
        self.values
            .first()
            .copied()
            .map_or(OperationAction::Complete, |value| OperationAction::Emit {
                port: PortId(0),
                value,
            })
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request)
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.output.is_none()
                    && outcome.failure.is_none() =>
            {
                self.pending = None;
                self.values.get(self.next).copied().map_or_else(
                    || Self::fail(1),
                    |value| OperationAction::Emit {
                        port: PortId(0),
                        value,
                    },
                )
            }
            _ => Self::fail(2),
        }
    }

    fn advance(&mut self) -> OperationAction {
        self.next += 1;
        if self.next >= self.values.len() {
            return OperationAction::Complete;
        }
        let Some(wait) = self.waits.get(self.next - 1).copied() else {
            return Self::fail(3);
        };
        let Ok(sequence) = u32::try_from(self.next) else {
            return Self::fail(4);
        };
        let request = RequestId(sequence);
        self.pending = Some(request);
        OperationAction::RequestHostOperation {
            request,
            operation: HostOperationId(0),
            input: BoundedValueRef::new(wait, 8).expect("sealed wait value is exactly admitted"),
        }
    }
}

pub struct DistributedSource {
    scheduler: SourceScheduler,
    fragment: PlanFragment,
    lowered: LoweredPlanFragment,
    binding: SessionBinding,
    session: SessionMachine,
    identity: KernelExecutionIdentityMap,
    seal: CapacitySeal,
    pressure_retries: u32,
}

impl DistributedSource {
    pub fn prepare() -> Result<Self, String> {
        let exact = exact_distributed_signal_plan()?;
        let fragment = exact
            .plan
            .fragments
            .iter()
            .find(|fragment| fragment.host_id == exact.source_advertisement.host_id)
            .cloned()
            .ok_or_else(|| "source fragment missing".to_string())?;
        let lowered = lower_plan_fragment(&fragment).map_err(|error| format!("{error:?}"))?;
        if lowered.nodes.len() != 1
            || lowered.cords.len() != 1
            || lowered.remote_endpoints.len() != 1
            || lowered.remote_endpoints[0].direction != RemoteCordDirection::Egress
            || lowered.host_operations.len() != 1
        {
            return Err("source fragment did not lower to one exact remote egress".to_string());
        }
        let remote = &lowered.remote_endpoints[0];
        let connection = fragment
            .connections
            .iter()
            .find(|connection| connection.connection_id == remote.connection_id)
            .ok_or_else(|| "source remote connection missing".to_string())?;
        let binding = SessionBinding::from_planned_connection(
            fragment.plan_id.clone(),
            remote.source_fragment_id.clone(),
            remote.sink_fragment_id.clone(),
            connection,
        )
        .map_err(|error| format!("{error:?}"))?;
        let configuration = parse_pulse_configuration(&fragment.placements[0].configuration)
            .map_err(|error| error.to_string())?;
        if configuration.count != MAXIMUM_VALUES as u64 || configuration.period_ms != 250 {
            return Err("unchanged Signal form configuration is not the S4 vector".to_string());
        }

        let mut values = HostedValueStore::new(
            MAXIMUM_STORED_ITEMS,
            SIGNAL_ENCODED_LEN,
            MAXIMUM_STORED_BYTES,
        )
        .map_err(|error| format!("{error:?}"))?;
        let mut signal_values = Vec::with_capacity(MAXIMUM_VALUES);
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
                    .map_err(|error| format!("{error:?}"))?,
            );
        }
        let mut waits = Vec::with_capacity(MAXIMUM_WAITS);
        for _ in 0..MAXIMUM_WAITS {
            waits.push(
                values
                    .store(&configuration.period_ms.to_le_bytes())
                    .map_err(|error| format!("{error:?}"))?,
            );
        }
        let mut routes = FixedRoutes::<ROUTE_SLOTS, 1>::new(PORTS as u16);
        for route in &lowered.routes {
            routes
                .install(
                    route.source_node,
                    route.source_port,
                    route.range,
                    &route.targets,
                )
                .map_err(|error| format!("{error:?}"))?;
        }
        routes.seal().map_err(|error| format!("{error:?}"))?;
        let mut host_bindings = FixedHostOperationBindings::<1>::new(1);
        host_bindings
            .install(
                lowered.host_operations[0].node,
                lowered.host_operations[0].binding,
            )
            .map_err(|error| format!("{error:?}"))?;
        host_bindings.seal().map_err(|error| format!("{error:?}"))?;
        let driver = OperationDriver::new(PulseOperation {
            values: signal_values,
            waits,
            next: 0,
            pending: None,
        })
        .map_err(|error| format!("{error:?}"))?;
        let evidence_bytes = u32::from(EVIDENCE_ITEMS)
            .checked_mul(core::mem::size_of::<conduit_kernel::KernelEvent>() as u32)
            .ok_or_else(|| "source evidence budget overflow".to_string())?;
        let evidence = HostedEvidenceLog::new(EVIDENCE_ITEMS, evidence_bytes)
            .map_err(|error| format!("{error:?}"))?;
        let scheduler = SourceScheduler::new_with_host_operations(
            lowered
                .node_specs
                .clone()
                .try_into()
                .map_err(|_| "source node table width".to_string())?,
            lowered
                .cords
                .iter()
                .map(|cord| cord.spec)
                .collect::<Vec<_>>()
                .try_into()
                .map_err(|_| "source cord table width".to_string())?,
            routes,
            host_bindings,
            [driver],
            values,
            evidence,
        )
        .map_err(|error| format!("{error:?}"))?;
        let active_play =
            bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
        if active_play.active_play_id != binding.source_active_play_id {
            return Err("source active-play identity disagrees with session".to_string());
        }
        let mut identity =
            KernelExecutionIdentityMap::new(&lowered.identity, &active_play, MAXIMUM_WAITS, 0, 1)
                .map_err(|error| format!("{error:?}"))?;
        for sequence in 1..=MAXIMUM_WAITS {
            identity
                .bind_request(
                    &lowered.identity,
                    lowered.nodes[0].node,
                    RequestId(sequence as u32),
                    HostOperationId(0),
                )
                .map_err(|error| format!("{error:?}"))?;
        }
        let session = SessionMachine::new(binding.clone(), SessionRole::Source)
            .map_err(|error| format!("{error:?}"))?;
        let seal = CapacitySeal {
            values: scheduler.values().allocation_capacities(),
            evidence: scheduler.evidence().allocation_capacity(),
            driver: scheduler.drivers()[0].operation().allocation_capacity(),
            identity: identity.allocation_capacities(),
        };
        Ok(Self {
            scheduler,
            fragment,
            lowered,
            binding,
            session,
            identity,
            seal,
            pressure_retries: 0,
        })
    }

    fn capacity_seal(&self) -> CapacitySeal {
        CapacitySeal {
            values: self.scheduler.values().allocation_capacities(),
            evidence: self.scheduler.evidence().allocation_capacity(),
            driver: self.scheduler.drivers()[0]
                .operation()
                .allocation_capacity(),
            identity: self.identity.allocation_capacities(),
        }
    }

    fn remote(&self) -> (RemoteEndpointId, CordId) {
        let remote = &self.lowered.remote_endpoints[0];
        (remote.endpoint, remote.cord)
    }

    fn complete_wait(&mut self, request: HostOperationRequest) -> Result<(), String> {
        let expected = self
            .identity
            .request(request.node, request.request)
            .ok_or_else(|| "unbound std wait request identity".to_string())?;
        if expected.operation != request.operation {
            return Err("std wait request operation identity mismatch".to_string());
        }
        let bytes = self
            .scheduler
            .host_value(request.input.value)
            .map_err(|error| format!("{error:?}"))?;
        let duration = u64::from_le_bytes(
            bytes
                .try_into()
                .map_err(|_| "std wait duration is not eight bytes".to_string())?,
        );
        thread::sleep(Duration::from_millis(duration));
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
            .map_err(|error| format!("{error:?}"))
    }

    fn next_offer(&mut self) -> Result<Option<(u64, [u8; SIGNAL_ENCODED_LEN as usize])>, String> {
        let (endpoint, cord) = self.remote();
        loop {
            if let Some(offer) = self
                .scheduler
                .remote_egress_offer(endpoint, cord)
                .map_err(|error| format!("{error:?}"))?
            {
                let bytes = self
                    .scheduler
                    .host_value(offer.value)
                    .map_err(|error| format!("{error:?}"))?;
                let payload = bytes
                    .try_into()
                    .map_err(|_| "remote Signal payload width mismatch".to_string())?;
                return Ok(Some((offer.sequence, payload)));
            }
            if let Some(request) = self.scheduler.next_host_request() {
                self.complete_wait(request)?;
                continue;
            }
            match self
                .scheduler
                .step()
                .map_err(|error| format!("{error:?}"))?
            {
                SchedulerStatus::Progress { .. } => {}
                SchedulerStatus::Complete => return Ok(None),
                SchedulerStatus::Idle => {
                    return Err("std source became idle before remote terminal".to_string())
                }
                SchedulerStatus::Cancelled => {
                    return Err("std source cancelled during successful proof".to_string())
                }
            }
        }
    }

    fn send(
        &mut self,
        carrier: &mut NativeWebSocketCarrier,
        message: SessionMessage<'_>,
        output: &mut [u8; DISTRIBUTED_MAXIMUM_FRAME_BYTES as usize],
    ) -> Result<(), String> {
        let frame = self.binding.frame(message);
        self.session
            .admit_outbound(frame)
            .map_err(|error| format!("{error:?}"))?;
        let length = encode_session_frame_into(
            frame,
            output,
            SIGNAL_ENCODED_LEN,
            DISTRIBUTED_MAXIMUM_FRAME_BYTES,
        )
        .map_err(|error| format!("{error:?}"))?;
        carrier
            .send_binary(&output[..length])
            .map_err(|error| format!("{error:?}"))
    }

    fn receive<'a>(
        &mut self,
        carrier: &mut NativeWebSocketCarrier,
        input: &'a mut [u8; DISTRIBUTED_MAXIMUM_FRAME_BYTES as usize],
    ) -> Result<SessionMessage<'a>, String> {
        let length = carrier
            .receive_binary(input)
            .map_err(|error| format!("{error:?}"))?;
        let frame = decode_session_frame(
            &input[..length],
            SIGNAL_ENCODED_LEN,
            DISTRIBUTED_MAXIMUM_FRAME_BYTES,
        )
        .map_err(|error| format!("{error:?}"))?;
        self.session
            .admit_inbound(frame)
            .map_err(|error| format!("{error:?}"))?;
        Ok(frame.message)
    }

    pub fn run<W: Write>(
        mut self,
        listener: NativeWebSocketListener,
        report: &mut W,
    ) -> Result<(), String> {
        let mut carrier = listener.accept().map_err(|error| format!("{error:?}"))?;
        let mut outbound = [0_u8; DISTRIBUTED_MAXIMUM_FRAME_BYTES as usize];
        let mut inbound = [0_u8; DISTRIBUTED_MAXIMUM_FRAME_BYTES as usize];

        if !matches!(
            self.receive(&mut carrier, &mut inbound).map_err(|detail| {
                format!("CND-DST-S4-201 phase=before-readiness detail={detail}")
            })?,
            SessionMessage::Hello(_)
        ) {
            return Err("browser did not begin with exact Hello".to_string());
        }
        let hello_binding = self.binding.clone();
        let hello = hello_binding.hello_frame().message;
        self.send(&mut carrier, hello, &mut outbound)?;
        if !matches!(
            self.receive(&mut carrier, &mut inbound).map_err(|detail| {
                format!("CND-DST-S4-201 phase=before-readiness detail={detail}")
            })?,
            SessionMessage::Ready
        ) {
            return Err("browser did not report Ready".to_string());
        }
        self.send(&mut carrier, SessionMessage::Ready, &mut outbound)?;
        if !self.session.is_active() {
            return Err("std source activated before both exact readiness facts".to_string());
        }

        while let Some((sequence, payload)) = self.next_offer()? {
            loop {
                self.send(
                    &mut carrier,
                    SessionMessage::Offered {
                        sequence,
                        payload: &payload,
                    },
                    &mut outbound,
                )?;
                match self.receive(&mut carrier, &mut inbound).map_err(|detail| {
                    format!(
                        "CND-DST-S4-202 phase=value-in-flight sequence={sequence} detail={detail}"
                    )
                })? {
                    SessionMessage::Pressure {
                        sequence: pressured,
                    } if pressured == sequence => {
                        self.pressure_retries += 1;
                        continue;
                    }
                    SessionMessage::Accepted { sequence: accepted } if accepted == sequence => {
                        let (endpoint, cord) = self.remote();
                        self.scheduler
                            .remote_egress_accept(endpoint, cord, sequence)
                            .map_err(|error| format!("{error:?}"))?;
                    }
                    other => return Err(format!("unexpected offer response {other:?}")),
                }
                match self.receive(&mut carrier, &mut inbound).map_err(|detail| {
                    format!(
                        "CND-DST-S4-202 phase=value-in-flight sequence={sequence} detail={detail}"
                    )
                })? {
                    SessionMessage::Delivered {
                        sequence: delivered,
                    } if delivered == sequence => {
                        let (endpoint, cord) = self.remote();
                        self.scheduler
                            .remote_egress_delivered(endpoint, cord, sequence)
                            .map_err(|error| format!("{error:?}"))?;
                        break;
                    }
                    other => return Err(format!("unexpected delivery response {other:?}")),
                }
            }
        }
        let (endpoint, cord) = self.remote();
        if !self
            .scheduler
            .remote_egress_terminal(endpoint, cord)
            .map_err(|error| format!("{error:?}"))?
        {
            return Err("std remote egress was not terminal".to_string());
        }
        let final_sequence = MAXIMUM_VALUES as u64;
        self.send(
            &mut carrier,
            SessionMessage::InputClosed { final_sequence },
            &mut outbound,
        )?;
        self.send(
            &mut carrier,
            SessionMessage::Terminal {
                disposition: SessionTerminalDisposition::Completed,
                final_sequence,
            },
            &mut outbound,
        )?;
        match self
            .receive(&mut carrier, &mut inbound)
            .map_err(|detail| format!("CND-DST-S4-203 phase=terminal-agreement detail={detail}"))?
        {
            SessionMessage::Terminal {
                disposition: SessionTerminalDisposition::Completed,
                final_sequence: peer_final,
            } if peer_final == final_sequence => {}
            other => return Err(format!("unexpected browser terminal {other:?}")),
        }
        if !self.session.is_terminal()
            || self.scheduler.values().used_items() != 0
            || self
                .scheduler
                .cord_usage(cord)
                .map_err(|error| format!("{error:?}"))?
                != (0, 0)
            || !self
                .scheduler
                .evidence()
                .contains_kind(KernelEventKind::RemoteValueDelivered)
            || !self
                .scheduler
                .evidence()
                .contains_kind(KernelEventKind::OperationCompleted)
            || self.capacity_seal() != self.seal
            || self.pressure_retries != 1
        {
            return Err("distributed source terminal invariants failed".to_string());
        }
        writeln!(
            report,
            "summary plan={} source_fragment={} sink_fragment={} source_play={} browser_play={} values={} pressure_retries={} retained=0 in_flight=0 source_terminal=completed browser_terminal=completed capacity_stable=true",
            self.binding.plan_id.as_str(),
            self.binding.source_fragment_id.as_str(),
            self.binding.sink_fragment_id.as_str(),
            self.binding.source_active_play_id.as_str(),
            self.binding.sink_active_play_id.as_str(),
            MAXIMUM_VALUES,
            self.pressure_retries,
        )
        .map_err(|error| error.to_string())?;
        carrier.close().map_err(|error| format!("{error:?}"))?;
        Ok(())
    }

    pub fn fragment(&self) -> &PlanFragment {
        &self.fragment
    }

    pub fn binding(&self) -> &SessionBinding {
        &self.binding
    }
}

pub fn bind_listener() -> Result<NativeWebSocketListener, String> {
    NativeWebSocketListener::bind_loopback(DISTRIBUTED_MAXIMUM_FRAME_BYTES)
        .map_err(|error| format!("{error:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unchanged_form_prepares_exact_independent_remote_fragments() {
        let canonical_source = include_str!("../../../examples/signal-demo.conduit");
        for realization_fact in ["std", "browser", "websocket", "host", "carrier"] {
            assert!(!canonical_source
                .to_ascii_lowercase()
                .contains(realization_fact));
        }
        let source = DistributedSource::prepare().expect("source prepares");
        let exact = exact_distributed_signal_plan().expect("distributed plan resolves");
        let sink = exact
            .plan
            .fragments
            .iter()
            .find(|fragment| fragment.host_id == exact.sink_advertisement.host_id)
            .expect("sink fragment");
        let lowered = lower_plan_fragment(sink).expect("sink lowers");
        assert_eq!(source.fragment().placements.len(), 1);
        assert_eq!(sink.placements.len(), 1);
        assert_ne!(source.fragment().host_id, sink.host_id);
        assert_eq!(
            source.fragment().connections[0].provider,
            ConnectionProvider::WebSocket
        );
        assert_eq!(lowered.remote_endpoints.len(), 1);
        assert_eq!(
            lowered.remote_endpoints[0].direction,
            RemoteCordDirection::Ingress
        );
        assert_eq!(source.binding().plan_id, sink.plan_id);
        assert_eq!(source.binding().sink_fragment_id, sink.fragment_id);
        assert_eq!(source.binding().limits.maximum_in_flight_items, 1);
        assert_eq!(
            source.binding().limits.maximum_payload_bytes,
            SIGNAL_ENCODED_LEN
        );
        assert_eq!(
            source.binding().limits.maximum_buffered_bytes,
            SIGNAL_ENCODED_LEN
        );
    }

    #[test]
    fn missing_and_stale_observed_links_fail_planning() {
        let source = distributed_std_source_advertisement();
        let sink = distributed_browser_sink_advertisement();
        let form = conduit_form::parse(
            include_str!("../../../examples/signal-demo.form"),
            &signal_profile_catalog(),
        )
        .unwrap();
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
        assert!(plan_with_link_bindings(
            &form,
            &[source.clone(), sink.clone()],
            &placements,
            &[ConnectionProvider::WebSocket],
            1,
            SIGNAL_ENCODED_LEN,
            &[],
        )
        .is_err());
        let mut stale_source = distributed_websocket_link_binding();
        stale_source.source.boot_id = conduit_core::BootId::from("stale-source");
        assert!(plan_with_link_bindings(
            &form,
            &[source.clone(), sink.clone()],
            &placements,
            &[ConnectionProvider::WebSocket],
            1,
            SIGNAL_ENCODED_LEN,
            &[stale_source],
        )
        .is_err());
        let mut stale_sink = distributed_websocket_link_binding();
        stale_sink.sink.boot_id = conduit_core::BootId::from("stale-browser");
        assert!(plan_with_link_bindings(
            &form,
            &[source, sink],
            &placements,
            &[ConnectionProvider::WebSocket],
            1,
            SIGNAL_ENCODED_LEN,
            &[stale_sink],
        )
        .is_err());
    }

    #[test]
    fn source_cancellation_releases_in_flight_values_and_rejects_late_acknowledgement() {
        let mut source = DistributedSource::prepare().expect("source prepares");
        let binding = source.binding.clone();
        source
            .session
            .admit_outbound(binding.hello_frame())
            .unwrap();
        source.session.admit_inbound(binding.hello_frame()).unwrap();
        source
            .session
            .admit_outbound(binding.frame(SessionMessage::Ready))
            .unwrap();
        source
            .session
            .admit_inbound(binding.frame(SessionMessage::Ready))
            .unwrap();
        let (sequence, payload) = source.next_offer().unwrap().unwrap();
        source
            .session
            .admit_outbound(binding.frame(SessionMessage::Offered {
                sequence,
                payload: &payload,
            }))
            .unwrap();
        source.scheduler.cancel().unwrap();
        source
            .session
            .admit_outbound(binding.frame(SessionMessage::Cancelled { code: 51 }))
            .unwrap();
        source
            .session
            .admit_outbound(binding.frame(SessionMessage::Terminal {
                disposition: SessionTerminalDisposition::Cancelled,
                final_sequence: 0,
            }))
            .unwrap();
        assert_eq!(
            source
                .session
                .admit_inbound(binding.frame(SessionMessage::Accepted { sequence })),
            Err(conduit_wire::WireError::InvalidState)
        );
        assert_eq!(source.scheduler.values().used_items(), 0);
        assert_eq!(source.capacity_seal(), source.seal);
    }
}
