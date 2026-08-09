//! Struct definition and kernel orchestration for the S4 toggle-demo std source.
//!
//! `DistributedToggleSource` prepares the kernel fragment (trigger + toggle)
//! and drives the WebSocket session to the browser sink host.
//!
//! Stdin reads are performed exclusively inside `complete_trigger_wait`,
//! i.e. within the admitted await-trigger host-operation lifecycle.
//!
//! Session/line transport lives in `line.rs`; tests live in `source_tests.rs`.

use super::operation::{CapacitySeal, ToggleSourceOperation};
use super::plan::exact_distributed_toggle_plan;
use crate::websocket::NativeWebSocketListener;
use conduit_core::{bind_active_play, PlanFragment};
use conduit_kernel::scheduler::{
    FixedScheduler, HostOperationRequest, OperationDriver, SchedulerStatus,
};
use conduit_kernel::{
    CordId, FixedHostOperationBindings, FixedRoutes, HostOperationDisposition, HostOperationId,
    HostOperationOutcome, HostedSignLog, HostedValueStore, RemoteEndpointId, RequestId,
    ValueStorage,
};
use conduit_runtime::lowering::{
    lower_plan_fragment, KernelExecutionIdentityMap, LoweredPlanFragment, RemoteCordDirection,
    MAXIMUM_KERNEL_PORTS_PER_NODE,
};
use conduit_signal::{
    encode_signal, parse_toggle_configuration, parse_trigger_configuration, Signal,
    SIGNAL_ENCODED_LEN, TRIGGER_ENCODED_LEN,
};
use conduit_wire::{SessionBinding, SessionMachine, SessionRole};
use std::io::{BufRead, Write};

pub(super) const PORTS: usize = MAXIMUM_KERNEL_PORTS_PER_NODE;
pub(super) const ROUTE_SLOTS: usize = 2 * PORTS;
pub(super) const MAXIMUM_VALUES: usize = 16;
pub(super) const MAXIMUM_WAITS: usize = MAXIMUM_VALUES;
const MAXIMUM_STORED_ITEMS: u16 = (MAXIMUM_VALUES * 2 + MAXIMUM_WAITS) as u16;
const MAXIMUM_STORED_BYTES: u32 = MAXIMUM_VALUES as u32 * SIGNAL_ENCODED_LEN
    + MAXIMUM_VALUES as u32 * TRIGGER_ENCODED_LEN
    + MAXIMUM_WAITS as u32;
pub(super) const SIGN_ITEMS: u16 = 256;

pub(super) type ToggleScheduler = FixedScheduler<
    OperationDriver<ToggleSourceOperation, PORTS>,
    HostedValueStore,
    HostedSignLog,
    2,
    2,
    PORTS,
    2,
    ROUTE_SLOTS,
    2,
    2,
    2,
>;

pub struct DistributedToggleSource {
    pub(super) scheduler: ToggleScheduler,
    pub(super) fragment: PlanFragment,
    pub(super) lowered: LoweredPlanFragment,
    pub(super) binding: SessionBinding,
    pub(super) session: SessionMachine,
    pub(super) identity: KernelExecutionIdentityMap,
    pub(super) seal: CapacitySeal,
    pub(super) pressure_retries: u32,
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

        let trigger_placement = fragment
            .placements
            .iter()
            .find(|placement| placement.kind_id.as_str() == "interaction/trigger")
            .ok_or("trigger placement missing")?;
        let toggle_placement = fragment
            .placements
            .iter()
            .find(|placement| placement.kind_id.as_str() == "state/toggle")
            .ok_or("toggle placement missing")?;

        let trigger_config = parse_trigger_configuration(&trigger_placement.configuration)
            .map_err(|error| error.to_string())?;
        let toggle_config = parse_toggle_configuration(&toggle_placement.configuration)
            .map_err(|error| error.to_string())?;
        if trigger_config.count != MAXIMUM_VALUES as u64 {
            return Err("toggle form trigger count is not the S4 vector".to_string());
        }

        let mut store = HostedValueStore::new(
            MAXIMUM_STORED_ITEMS,
            SIGNAL_ENCODED_LEN,
            MAXIMUM_STORED_BYTES,
        )
        .map_err(|error| format!("{error:?}"))?;

        let mut trigger_values = Vec::with_capacity(MAXIMUM_VALUES);
        for sequence in 0..trigger_config.count {
            trigger_values.push(
                store
                    .store(&sequence.to_le_bytes())
                    .map_err(|error| format!("{error:?}"))?,
            );
        }

        let mut token_values = Vec::with_capacity(MAXIMUM_WAITS);
        for sequence in 0..MAXIMUM_WAITS {
            token_values.push(
                store
                    .store(&[sequence as u8])
                    .map_err(|error| format!("{error:?}"))?,
            );
        }

        let mut signal_values = Vec::with_capacity(MAXIMUM_VALUES);
        let mut current_level = toggle_config.initial;
        for sequence in 0..trigger_config.count {
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

        let mut host_bindings = FixedHostOperationBindings::<2>::new(1);
        host_bindings
            .install(
                lowered.host_operations[0].node,
                lowered.host_operations[0].binding,
            )
            .map_err(|error| format!("{error:?}"))?;
        host_bindings.seal().map_err(|error| format!("{error:?}"))?;

        let trigger_node_idx = lowered
            .nodes
            .iter()
            .position(|node| {
                fragment
                    .placements
                    .get(usize::from(node.node.0))
                    .is_some_and(|placement| placement.kind_id.as_str() == "interaction/trigger")
            })
            .ok_or("trigger node not found in lowered fragment")?;
        let toggle_node_idx = lowered
            .nodes
            .iter()
            .position(|node| {
                fragment
                    .placements
                    .get(usize::from(node.node.0))
                    .is_some_and(|placement| placement.kind_id.as_str() == "state/toggle")
            })
            .ok_or("toggle node not found in lowered fragment")?;

        let trigger_driver = OperationDriver::new(ToggleSourceOperation::Trigger {
            tokens: token_values,
            values: trigger_values.clone(),
            next: 0,
            pending: None,
        })
        .map_err(|error| format!("{error:?}"))?;
        let toggle_driver = OperationDriver::new(ToggleSourceOperation::Toggle {
            signals: signal_values,
            expected_triggers: trigger_values,
            next: 0,
        })
        .map_err(|error| format!("{error:?}"))?;

        let drivers = if trigger_node_idx < toggle_node_idx {
            [trigger_driver, toggle_driver]
        } else {
            [toggle_driver, trigger_driver]
        };

        let sign_bytes = u32::from(SIGN_ITEMS)
            .checked_mul(core::mem::size_of::<conduit_kernel::KernelEvent>() as u32)
            .ok_or_else(|| "source sign budget overflow".to_string())?;
        let sign =
            HostedSignLog::new(SIGN_ITEMS, sign_bytes).map_err(|error| format!("{error:?}"))?;

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
            sign,
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
        let trigger_node = lowered.nodes[trigger_node_idx].node;
        for sequence in 0..MAXIMUM_WAITS {
            identity
                .bind_request(
                    &lowered.identity,
                    trigger_node,
                    RequestId(sequence as u32),
                    HostOperationId(0),
                )
                .map_err(|error| format!("{error:?}"))?;
        }

        let session = SessionMachine::new(binding.clone(), SessionRole::Source)
            .map_err(|error| format!("{error:?}"))?;
        let seal = CapacitySeal {
            values: scheduler.values().allocation_capacities(),
            sign: scheduler.signs().allocation_capacity(),
            drivers: scheduler
                .drivers()
                .iter()
                .map(|driver| driver.operation().allocation_capacity())
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

    pub(super) fn capacity_seal(&self) -> CapacitySeal {
        CapacitySeal {
            values: self.scheduler.values().allocation_capacities(),
            sign: self.scheduler.signs().allocation_capacity(),
            drivers: self
                .scheduler
                .drivers()
                .iter()
                .map(|driver| driver.operation().allocation_capacity())
                .sum(),
            identity: self.identity.allocation_capacities(),
        }
    }

    pub(super) fn remote(&self) -> (RemoteEndpointId, CordId) {
        let remote = &self.lowered.remote_endpoints[0];
        (remote.endpoint, remote.cord)
    }

    fn complete_trigger_wait<R: BufRead>(
        &mut self,
        request: HostOperationRequest,
        report: &mut impl Write,
        stdin: &mut R,
        trigger_index: usize,
    ) -> Result<(), String> {
        let expected = self
            .identity
            .request(request.node, request.request)
            .ok_or_else(|| "unbound await-trigger request identity".to_string())?;
        if expected.operation != request.operation {
            return Err("await-trigger request operation identity mismatch".to_string());
        }
        writeln!(
            report,
            "Press Enter to trigger ({trigger_index}/{MAXIMUM_VALUES})"
        )
        .map_err(|error| error.to_string())?;
        let mut line = String::new();
        let bytes_read = stdin
            .read_line(&mut line)
            .map_err(|error| format!("CND-TOG-TRIGGER-EOF stdin read failed: {error}"))?;
        if bytes_read == 0 {
            return Err(
                "CND-TOG-TRIGGER-EOF stdin reached EOF before all triggers were received"
                    .to_string(),
            );
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
            .map_err(|error| format!("{error:?}"))
    }

    pub(super) fn next_offer<R: BufRead>(
        &mut self,
        report: &mut impl Write,
        stdin: &mut R,
        trigger_index: &mut usize,
    ) -> Result<Option<(u64, [u8; SIGNAL_ENCODED_LEN as usize])>, String> {
        let (endpoint, cord) = self.remote();
        let mut completed_trigger = false;
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
            if !completed_trigger {
                if let Some(request) = self.scheduler.next_host_request() {
                    let current_index = *trigger_index;
                    *trigger_index += 1;
                    self.complete_trigger_wait(request, report, stdin, current_index)?;
                    completed_trigger = true;
                    continue;
                }
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

    pub fn run<R: BufRead, W: Write>(
        self,
        listener: NativeWebSocketListener,
        stdin: &mut R,
        report: &mut W,
    ) -> Result<(), String> {
        super::line::run_source(self, listener, stdin, report)
    }

    pub fn fragment(&self) -> &PlanFragment {
        &self.fragment
    }

    pub fn binding(&self) -> &SessionBinding {
        &self.binding
    }
}

#[cfg(test)]
#[path = "source_tests.rs"]
mod tests;
