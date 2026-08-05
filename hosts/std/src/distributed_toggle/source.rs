//! Orchestration for the S4 toggle-demo std source.
//!
//! `DistributedToggleSource` prepares the kernel fragment (activate + toggle)
//! and drives the WebSocket session to the browser sink host.
//!
//! Stdin reads are performed exclusively inside `complete_activation_wait`,
//! i.e. within the admitted await-activation host-operation lifecycle.

use super::operation::{CapacitySeal, ToggleSourceOperation};
use super::plan::exact_distributed_toggle_plan;
use crate::websocket::{NativeWebSocketCarrier, NativeWebSocketListener};
use conduit_core::{bind_active_play, PlanFragment};
use conduit_kernel::scheduler::{
    FixedScheduler, HostOperationRequest, OperationDriver, SchedulerStatus,
};
use conduit_kernel::{
    CordId, EvidenceQuery, FixedHostOperationBindings, FixedRoutes, HostOperationDisposition,
    HostOperationId, HostOperationOutcome, HostedEvidenceLog, HostedValueStore, KernelEventKind,
    RemoteEndpointId, RequestId, ValueStorage,
};
use conduit_runtime::lowering::{
    lower_plan_fragment, KernelExecutionIdentityMap, LoweredPlanFragment, RemoteCordDirection,
    MAXIMUM_KERNEL_PORTS_PER_NODE,
};
use conduit_signal::{
    encode_signal, parse_activate_configuration, parse_toggle_configuration, Signal,
    ACTIVATION_ENCODED_LEN, DISTRIBUTED_MAXIMUM_FRAME_BYTES, SIGNAL_ENCODED_LEN,
};
use conduit_wire::{
    decode_session_frame, encode_session_frame_into, SessionBinding, SessionMachine,
    SessionMessage, SessionRole, SessionTerminalDisposition,
};
use std::io::{BufRead, Write};

const PORTS: usize = MAXIMUM_KERNEL_PORTS_PER_NODE;
const ROUTE_SLOTS: usize = 2 * PORTS;
const MAXIMUM_VALUES: usize = 16;
const MAXIMUM_WAITS: usize = MAXIMUM_VALUES;
const MAXIMUM_STORED_ITEMS: u16 = (MAXIMUM_VALUES * 2 + MAXIMUM_WAITS) as u16;
const MAXIMUM_STORED_BYTES: u32 = MAXIMUM_VALUES as u32 * SIGNAL_ENCODED_LEN
    + MAXIMUM_VALUES as u32 * ACTIVATION_ENCODED_LEN
    + MAXIMUM_WAITS as u32; // 1-byte tokens per await-activation op
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

        let activate_config = parse_activate_configuration(&activate_placement.configuration)
            .map_err(|error| error.to_string())?;
        let toggle_config = parse_toggle_configuration(&toggle_placement.configuration)
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

        // 1-byte sequence correlation tokens for the await-activation host operation.
        // The token value is the sequence index (0..MAXIMUM_WAITS) as a single byte.
        // The std adapter reads stdin when completing the host-operation request.
        let mut token_values = Vec::with_capacity(MAXIMUM_WAITS);
        for seq in 0..MAXIMUM_WAITS {
            let token_byte = [seq as u8];
            token_values.push(
                store
                    .store(&token_byte)
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
            tokens: token_values,
            values: activation_values.clone(),
            next: 0,
            pending: None,
        })
        .map_err(|error| format!("{error:?}"))?;
        let toggle_driver = OperationDriver::new(ToggleSourceOperation::Toggle {
            signals: signal_values,
            expected_activations: activation_values,
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

    /// Complete an await-activation host-operation request by blocking on one
    /// operator input line.  The stdin read happens here — inside the admitted
    /// host-operation lifecycle — not before the kernel issues the request.
    fn complete_activation_wait<R: BufRead>(
        &mut self,
        request: HostOperationRequest,
        report: &mut impl Write,
        stdin: &mut R,
        activation_index: usize,
    ) -> Result<(), String> {
        let expected = self
            .identity
            .request(request.node, request.request)
            .ok_or_else(|| "unbound await-activation request identity".to_string())?;
        if expected.operation != request.operation {
            return Err("await-activation request operation identity mismatch".to_string());
        }
        writeln!(
            report,
            "Press Enter to activate ({activation_index}/{MAXIMUM_VALUES})"
        )
        .map_err(|e| e.to_string())?;
        let mut line = String::new();
        stdin
            .read_line(&mut line)
            .map_err(|e| format!("stdin read failed: {e}"))?;
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

    fn next_offer<R: BufRead>(
        &mut self,
        report: &mut impl Write,
        stdin: &mut R,
        activation_index: &mut usize,
    ) -> Result<Option<(u64, [u8; SIGNAL_ENCODED_LEN as usize])>, String> {
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
                let current_index = *activation_index;
                *activation_index += 1;
                self.complete_activation_wait(request, report, stdin, current_index)?;
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
        while let Some((sequence, payload)) =
            self.next_offer(report, stdin, &mut activation_index)?
        {
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
    use super::super::plan::exact_distributed_toggle_plan;
    use super::*;
    use conduit_core::{CapabilityId, ConnectionProvider, OperationId};
    use conduit_planner::{plan_with_link_bindings, PlacementChoice, PlacementChoices};
    use conduit_runtime::lowering::RemoteCordDirection;
    use conduit_signal::{
        distributed_toggle_browser_sink_advertisement, distributed_toggle_std_source_advertisement,
        signal_profile_catalog,
    };
    use std::collections::BTreeMap;

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
            include_str!("../../../../examples/remote-toggle.form"),
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
