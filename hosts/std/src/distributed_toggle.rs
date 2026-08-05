//! Exact std source half of the live S4 toggle-demo proof.
//!
//! The source fragment runs `interaction/activate` (reading Enter from stdin) and
//! `state/toggle` (stateful bool flip), then streams `Signal` values over a bounded
//! WebSocket remote cord to the browser `presentation/show` sink.

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
use conduit_planner::{plan_with_link_bindings, PlacementChoice, PlacementChoices};
use conduit_runtime::lowering::{
    lower_plan_fragment, KernelExecutionIdentityMap, LoweredPlanFragment, RemoteCordDirection,
    MAXIMUM_KERNEL_PORTS_PER_NODE,
};
use conduit_signal::{
    distributed_toggle_browser_sink_advertisement, distributed_toggle_std_source_advertisement,
    distributed_toggle_websocket_link_binding, encode_signal, parse_activate_configuration,
    parse_toggle_configuration, signal_profile_catalog, Signal, ACTIVATION_ENCODED_LEN,
    DISTRIBUTED_MAXIMUM_FRAME_BYTES, DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS, SIGNAL_ENCODED_LEN,
};
use conduit_wire::{
    decode_session_frame, encode_session_frame_into, SessionBinding, SessionMachine,
    SessionMessage, SessionRole, SessionTerminalDisposition,
};
use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::thread;
use std::time::Duration;

const PORTS: usize = MAXIMUM_KERNEL_PORTS_PER_NODE;
// 2 nodes: activate and toggle; 1 remote cord: toggle -> browser show
const ROUTE_SLOTS: usize = 2 * PORTS;
const MAXIMUM_VALUES: usize = 16;
// activate needs one wait per value; toggle needs no waits
const MAXIMUM_WAITS: usize = MAXIMUM_VALUES;
const MAXIMUM_STORED_ITEMS: u16 = (MAXIMUM_VALUES * 2 + MAXIMUM_WAITS) as u16;
const MAXIMUM_STORED_BYTES: u32 =
    MAXIMUM_VALUES as u32 * SIGNAL_ENCODED_LEN
        + MAXIMUM_VALUES as u32 * ACTIVATION_ENCODED_LEN
        + MAXIMUM_WAITS as u32 * 8;
const EVIDENCE_ITEMS: u16 = 256;

type ToggleScheduler = FixedScheduler<
    OperationDriver<ToggleSourceOperation, PORTS>,
    HostedValueStore,
    HostedEvidenceLog,
    2,
    2,
    PORTS,
    2,
    ROUTE_SLOTS,
    2,
    1,
    2,
>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DistributedTogglePlan {
    pub source_advertisement: HostAdvertisement,
    pub sink_advertisement: HostAdvertisement,
    pub plan: Plan,
}

pub fn exact_distributed_toggle_plan() -> Result<DistributedTogglePlan, String> {
    let source_advertisement = distributed_toggle_std_source_advertisement();
    let sink_advertisement = distributed_toggle_browser_sink_advertisement();
    let form = conduit_form::parse(
        include_str!("../../../examples/remote-toggle.form"),
        &signal_profile_catalog(),
    )
    .map_err(|error| error.to_string())?;
    let placements = PlacementChoices {
        by_operation: BTreeMap::from([
            (
                OperationId::from("activate"),
                PlacementChoice {
                    host_id: source_advertisement.host_id.clone(),
                    capability_id: CapabilityId::from("activate-1"),
                },
            ),
            (
                OperationId::from("toggle"),
                PlacementChoice {
                    host_id: source_advertisement.host_id.clone(),
                    capability_id: CapabilityId::from("toggle-1"),
                },
            ),
            (
                OperationId::from("show"),
                PlacementChoice {
                    host_id: sink_advertisement.host_id.clone(),
                    capability_id: CapabilityId::from("toggle-dom-show-1"),
                },
            ),
        ]),
    };
    let link = distributed_toggle_websocket_link_binding();
    let plan = plan_with_link_bindings(
        &form,
        &[source_advertisement.clone(), sink_advertisement.clone()],
        &placements,
        &[ConnectionProvider::Local, ConnectionProvider::WebSocket],
        DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
        SIGNAL_ENCODED_LEN,
        &[link],
    )
    .map_err(|error| error.to_string())?;
    Ok(DistributedTogglePlan {
        source_advertisement,
        sink_advertisement,
        plan,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CapacitySeal {
    values: (usize, usize),
    evidence: usize,
    drivers: usize,
    identity: (usize, usize, usize),
}

/// Kernel operation covering both `interaction/activate` (stdin waits) and
/// `state/toggle` (stateful bool flip).  Each node in the source scheduler
/// gets its own driver of this enum type.
enum ToggleSourceOperation {
    /// Activate: waits for one Enter press, emits an `Activation` payload.
    Activate {
        /// Pre-stored wait tokens (one per activation).
        waits: Vec<ValueRef>,
        /// Pre-stored activation payloads.
        values: Vec<ValueRef>,
        next: usize,
        pending: Option<RequestId>,
    },
    /// Toggle: receives one activation, emits one Signal.
    Toggle {
        /// Pre-stored Signal payloads.
        signals: Vec<ValueRef>,
        next: usize,
    },
}

impl ToggleSourceOperation {
    fn fail(detail: u16) -> OperationAction {
        OperationAction::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            detail,
        })
    }

    fn allocation_capacity(&self) -> usize {
        match self {
            Self::Activate { waits, values, .. } => waits.capacity() + values.capacity(),
            Self::Toggle { signals, .. } => signals.capacity(),
        }
    }
}

impl Operation for ToggleSourceOperation {
    fn start(&mut self) -> OperationAction {
        match self {
            Self::Activate { waits, next, pending, .. } if !waits.is_empty() => {
                let Some(wait) = waits.first().copied() else {
                    return Self::fail(10);
                };
                let request = RequestId(0);
                *pending = Some(request);
                *next = 0;
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(0),
                    input: BoundedValueRef::new(wait, 8)
                        .expect("sealed wait value is exactly admitted"),
                }
            }
            Self::Activate { .. } => OperationAction::Complete,
            Self::Toggle { .. } => OperationAction::Await,
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match (self, input) {
            (
                Self::Activate {
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
                    || Self::fail(11),
                    |value| OperationAction::Emit {
                        port: PortId(0),
                        value,
                    },
                )
            }
            (
                Self::Toggle { signals, next },
                OperationInput::Value { port: PortId(0), .. },
            ) => {
                signals.get(*next).copied().map_or_else(
                    || Self::fail(12),
                    |value| OperationAction::Emit {
                        port: PortId(0),
                        value,
                    },
                )
            }
            _ => Self::fail(13),
        }
    }

    fn advance(&mut self) -> OperationAction {
        match self {
            Self::Activate {
                waits,
                next,
                pending,
                ..
            } => {
                *next += 1;
                if *next > waits.len() {
                    return OperationAction::Complete;
                }
                let Some(wait) = waits.get(*next - 1).copied() else {
                    return Self::fail(14);
                };
                let Ok(sequence) = u32::try_from(*next) else {
                    return Self::fail(15);
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
            Self::Toggle { next, .. } => {
                *next += 1;
                OperationAction::Await
            }
        }
    }
}

pub struct DistributedToggleSource {
    scheduler: ToggleScheduler,
    fragment: PlanFragment,
    lowered: LoweredPlanFragment,
    binding: SessionBinding,
    session: SessionMachine,
    identity: KernelExecutionIdentityMap,
    seal: CapacitySeal,
    pressure_retries: u32,
}

impl DistributedToggleSource {
    pub fn prepare() -> Result<Self, String> {
        let exact = exact_distributed_toggle_plan()?;
        let fragment = exact
            .plan
            .fragments
            .iter()
            .find(|fragment| fragment.host_id == exact.source_advertisement.host_id)
            .cloned()
            .ok_or_else(|| "source fragment missing".to_string())?;
        let lowered = lower_plan_fragment(&fragment).map_err(|error| format!("{error:?}"))?;
        if lowered.nodes.len() != 2
            || lowered.cords.len() != 2
            || lowered.remote_endpoints.len() != 1
            || lowered.remote_endpoints[0].direction != RemoteCordDirection::Egress
            || lowered.host_operations.len() != 1
        {
            return Err(format!(
                "source fragment did not lower to two nodes with one local and one remote cord: nodes={} cords={} remote_endpoints={} host_ops={}",
                lowered.nodes.len(), lowered.cords.len(), lowered.remote_endpoints.len(), lowered.host_operations.len()
            ));
        }

        // Find activate and toggle placements by kind_id
        let activate_placement = fragment
            .placements
            .iter()
            .find(|p| p.kind_id.as_str() == "interaction/activate")
            .ok_or("activate placement missing")?;
        let toggle_placement = fragment
            .placements
            .iter()
            .find(|p| p.kind_id.as_str() == "state/toggle")
            .ok_or("toggle placement missing")?;

        let activate_config =
            parse_activate_configuration(&activate_placement.configuration)
                .map_err(|error| error.to_string())?;
        let toggle_config =
            parse_toggle_configuration(&toggle_placement.configuration)
                .map_err(|error| error.to_string())?;
        if activate_config.count != MAXIMUM_VALUES as u64 {
            return Err("toggle form activation count is not the S4 vector".to_string());
        }

        // Pre-store all values
        let mut store = HostedValueStore::new(
            MAXIMUM_STORED_ITEMS,
            SIGNAL_ENCODED_LEN,
            MAXIMUM_STORED_BYTES,
        )
        .map_err(|error| format!("{error:?}"))?;

        // Activation payloads: 8 bytes each (sequence as u64 LE)
        let mut activation_values = Vec::with_capacity(MAXIMUM_VALUES);
        for sequence in 0..activate_config.count {
            let encoded = sequence.to_le_bytes();
            activation_values.push(
                store
                    .store(&encoded)
                    .map_err(|error| format!("{error:?}"))?,
            );
        }

        // Wait tokens (0ms, but still need the value slot): use 0u64 LE
        let mut wait_values = Vec::with_capacity(MAXIMUM_WAITS);
        for _ in 0..MAXIMUM_WAITS {
            wait_values.push(
                store
                    .store(&0u64.to_le_bytes())
                    .map_err(|error| format!("{error:?}"))?,
            );
        }

        // Pre-compute toggle signals
        let mut signal_values = Vec::with_capacity(MAXIMUM_VALUES);
        let mut current_level = toggle_config.initial;
        for sequence in 0..activate_config.count {
            current_level = !current_level;
            let payload = encode_signal(&Signal {
                sequence,
                level: current_level,
            });
            signal_values.push(
                store
                    .store(&payload.encoded)
                    .map_err(|error| format!("{error:?}"))?,
            );
        }

        // Install routes
        let mut routes = FixedRoutes::<ROUTE_SLOTS, 2>::new(PORTS as u16);
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

        // Install host operation bindings
        let mut host_bindings = FixedHostOperationBindings::<1>::new(1);
        host_bindings
            .install(
                lowered.host_operations[0].node,
                lowered.host_operations[0].binding,
            )
            .map_err(|error| format!("{error:?}"))?;
        host_bindings.seal().map_err(|error| format!("{error:?}"))?;

        // Find activate vs toggle node index by matching to lowered node ordering
        let activate_node_idx = lowered
            .nodes
            .iter()
            .position(|n| {
                fragment
                    .placements
                    .get(usize::from(n.node.0))
                    .map(|p| p.kind_id.as_str() == "interaction/activate")
                    .unwrap_or(false)
            })
            .ok_or("activate node not found in lowered fragment")?;
        let toggle_node_idx = lowered
            .nodes
            .iter()
            .position(|n| {
                fragment
                    .placements
                    .get(usize::from(n.node.0))
                    .map(|p| p.kind_id.as_str() == "state/toggle")
                    .unwrap_or(false)
            })
            .ok_or("toggle node not found in lowered fragment")?;

        let activate_driver = OperationDriver::new(ToggleSourceOperation::Activate {
            waits: wait_values,
            values: activation_values,
            next: 0,
            pending: None,
        })
        .map_err(|error| format!("{error:?}"))?;
        let toggle_driver = OperationDriver::new(ToggleSourceOperation::Toggle {
            signals: signal_values,
            next: 0,
        })
        .map_err(|error| format!("{error:?}"))?;

        // Build the [driver; 2] array in node index order
        let drivers = if activate_node_idx < toggle_node_idx {
            [activate_driver, toggle_driver]
        } else {
            [toggle_driver, activate_driver]
        };

        let evidence_bytes = u32::from(EVIDENCE_ITEMS)
            .checked_mul(core::mem::size_of::<conduit_kernel::KernelEvent>() as u32)
            .ok_or_else(|| "source evidence budget overflow".to_string())?;
        let evidence = HostedEvidenceLog::new(EVIDENCE_ITEMS, evidence_bytes)
            .map_err(|error| format!("{error:?}"))?;

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

        let scheduler = ToggleScheduler::new_with_host_operations(
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
            drivers,
            store,
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
        // Bind request identities for the activate node's waits
        let activate_node = lowered.nodes[activate_node_idx].node;
        for sequence in 0..MAXIMUM_WAITS {
            identity
                .bind_request(
                    &lowered.identity,
                    activate_node,
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
            drivers: scheduler
                .drivers()
                .iter()
                .map(|d| d.operation().allocation_capacity())
                .sum(),
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
            drivers: self
                .scheduler
                .drivers()
                .iter()
                .map(|d| d.operation().allocation_capacity())
                .sum(),
            identity: self.identity.allocation_capacities(),
        }
    }

    fn remote(&self) -> (RemoteEndpointId, CordId) {
        let remote = &self.lowered.remote_endpoints[0];
        (remote.endpoint, remote.cord)
    }

    fn complete_stdin_wait(&mut self, request: HostOperationRequest) -> Result<(), String> {
        let expected = self
            .identity
            .request(request.node, request.request)
            .ok_or_else(|| "unbound std wait request identity".to_string())?;
        if expected.operation != request.operation {
            return Err("std wait request operation identity mismatch".to_string());
        }
        // Sleep 0ms — the real operator interaction happens at the binary level
        // via stdin.  The kernel operation waits with duration_ms=0 so we just
        // complete immediately; the binary handles reading Enter before calling run().
        thread::sleep(Duration::from_millis(0));
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
                self.complete_stdin_wait(request)?;
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

    /// Drive the source, reading Enter from `stdin` before each activation.
    pub fn run<R: BufRead, W: Write>(
        mut self,
        listener: NativeWebSocketListener,
        stdin: &mut R,
        report: &mut W,
    ) -> Result<(), String> {
        let mut carrier = listener.accept().map_err(|error| format!("{error:?}"))?;
        let mut outbound = [0_u8; DISTRIBUTED_MAXIMUM_FRAME_BYTES as usize];
        let mut inbound = [0_u8; DISTRIBUTED_MAXIMUM_FRAME_BYTES as usize];

        if !matches!(
            self.receive(&mut carrier, &mut inbound).map_err(|detail| {
                format!("CND-TOG-S4-201 phase=before-readiness detail={detail}")
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
                format!("CND-TOG-S4-201 phase=before-readiness detail={detail}")
            })?,
            SessionMessage::Ready
        ) {
            return Err("browser did not report Ready".to_string());
        }
        self.send(&mut carrier, SessionMessage::Ready, &mut outbound)?;
        if !self.session.is_active() {
            return Err("std source activated before both exact readiness facts".to_string());
        }

        let mut activation_index = 0usize;
        while let Some((sequence, payload)) = {
            // Read one Enter press before letting the scheduler produce the next value.
            // This is only called when the kernel requests a wait (0ms),
            // which happens once per activation.  We block here so that the
            // operator controls the pace.
            if activation_index < MAXIMUM_VALUES {
                writeln!(report, "Press Enter to activate ({activation_index}/{MAXIMUM_VALUES})")
                    .map_err(|e| e.to_string())?;
                let mut line = String::new();
                stdin.read_line(&mut line).map_err(|e| e.to_string())?;
                activation_index += 1;
            }
            self.next_offer()?
        } {
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
                        "CND-TOG-S4-202 phase=value-in-flight sequence={sequence} detail={detail}"
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
                        "CND-TOG-S4-202 phase=value-in-flight sequence={sequence} detail={detail}"
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
            .map_err(|detail| format!("CND-TOG-S4-203 phase=terminal-agreement detail={detail}"))?
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
        {
            return Err("distributed toggle source terminal invariants failed".to_string());
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
    use conduit_runtime::lowering::RemoteCordDirection;

    #[test]
    fn unchanged_toggle_form_prepares_exact_independent_remote_fragments() {
        let source = DistributedToggleSource::prepare().expect("toggle source prepares");
        let exact = exact_distributed_toggle_plan().expect("distributed toggle plan resolves");
        let sink = exact
            .plan
            .fragments
            .iter()
            .find(|fragment| fragment.host_id == exact.sink_advertisement.host_id)
            .expect("sink fragment");
        let lowered = lower_plan_fragment(sink).expect("sink lowers");
        // Source fragment has 2 nodes (activate + toggle)
        assert_eq!(source.fragment().placements.len(), 2);
        // Sink fragment has 1 node (show)
        assert_eq!(sink.placements.len(), 1);
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
    }

    #[test]
    fn missing_link_binding_fails_toggle_planning() {
        let source = distributed_toggle_std_source_advertisement();
        let sink = distributed_toggle_browser_sink_advertisement();
        let form = conduit_form::parse(
            include_str!("../../../examples/remote-toggle.form"),
            &signal_profile_catalog(),
        )
        .unwrap();
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
        assert!(plan_with_link_bindings(
            &form,
            &[source, sink],
            &placements,
            &[ConnectionProvider::Local, ConnectionProvider::WebSocket],
            1,
            SIGNAL_ENCODED_LEN,
            &[],
        )
        .is_err());
    }
}
