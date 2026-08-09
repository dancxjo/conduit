//! Kernel-owned std source for the exact planned std-to-Pico UsbCdc path.

use std::thread;
use std::time::Duration;

use conduit_core::{bind_active_play, BootId, HostId, Plan, PlanFragment};
use conduit_kernel::scheduler::{
    FixedScheduler, HostOperationRequest, OperationDriver, SchedulerStatus,
};
use conduit_kernel::{
    CordId, FixedHostOperationBindings, FixedRoutes, HostOperationDisposition, HostOperationId,
    HostOperationOutcome, HostedSignLog, HostedValueStore, KernelEventKind, RemoteEndpointId,
    RequestId, SignQuery, ValueStorage,
};
use conduit_runtime::lowering::{
    lower_plan_fragment, KernelExecutionIdentityMap, LoweredPlanFragment, RemoteCordDirection,
    MAXIMUM_KERNEL_PORTS_PER_NODE,
};
use conduit_signal::{
    encode_signal, exact_std_pico_usb_plan, parse_pulse_configuration, Signal,
    DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS, SIGNAL_ENCODED_LEN, STD_PICO_USB_SOURCE_HOST_ID,
};
use conduit_wire::{
    SessionBinding, SessionCheckpointAcceptance, SessionCheckpointOffer, SessionFrame,
    SessionMachine, SessionRole,
};

mod pulse;
use pulse::{PulseOperation, MAXIMUM_VALUES, MAXIMUM_WAITS};

const PORTS: usize = MAXIMUM_KERNEL_PORTS_PER_NODE;
const MAXIMUM_STORED_ITEMS: u16 = (MAXIMUM_VALUES + MAXIMUM_WAITS) as u16;
const MAXIMUM_STORED_BYTES: u32 =
    MAXIMUM_VALUES as u32 * SIGNAL_ENCODED_LEN + MAXIMUM_WAITS as u32 * 8;
const SIGN_ITEMS: u16 = 256;

type SourceScheduler = FixedScheduler<
    OperationDriver<PulseOperation, PORTS>,
    HostedValueStore,
    HostedSignLog,
    1,
    1,
    PORTS,
    1,
    PORTS,
    1,
    1,
    1,
>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CapacitySeal {
    values: (usize, usize),
    sign: usize,
    driver: usize,
    identity: (usize, usize, usize),
}

pub struct PicoUsbSource {
    scheduler: SourceScheduler,
    fragment: PlanFragment,
    lowered: LoweredPlanFragment,
    binding: SessionBinding,
    session: SessionMachine,
    identity: KernelExecutionIdentityMap,
    seal: CapacitySeal,
    pressure_retries: u32,
}

impl PicoUsbSource {
    pub fn prepare() -> Result<Self, String> {
        let exact = exact_std_pico_usb_plan()?;
        Self::prepare_plan(exact.plan, &HostId::from(STD_PICO_USB_SOURCE_HOST_ID))
    }

    pub fn prepare_plan(plan: Plan, source_host_id: &HostId) -> Result<Self, String> {
        let fragment = plan
            .fragments
            .iter()
            .find(|fragment| &fragment.host_id == source_host_id)
            .cloned()
            .ok_or_else(|| "exact std-to-Pico source fragment missing".to_owned())?;
        let lowered = lower_plan_fragment(&fragment).map_err(|error| format!("{error:?}"))?;
        if lowered.nodes.len() != 1
            || lowered.cords.len() != 1
            || lowered.remote_endpoints.is_empty()
            || lowered
                .remote_endpoints
                .iter()
                .any(|endpoint| endpoint.direction != RemoteCordDirection::Egress)
            || lowered.host_operations.len() != 1
        {
            return Err("std fragment is not one exact kernel remote egress".to_owned());
        }
        let remote = &lowered.remote_endpoints[0];
        let connection = fragment
            .connections
            .iter()
            .find(|connection| connection.connection_id == remote.connection_id)
            .ok_or_else(|| "planned source connection missing".to_owned())?;
        let binding = SessionBinding::from_planned_connection(
            fragment.plan_id.clone(),
            remote.source_fragment_id.clone(),
            remote.sink_fragment_id.clone(),
            connection,
        )
        .map_err(|error| format!("{error:?}"))?;
        if binding.limits.maximum_in_flight_items != DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS {
            return Err("planned remote Signal item bound changed".to_owned());
        }

        let configuration = parse_pulse_configuration(&fragment.placements[0].configuration)
            .map_err(|error| error.to_string())?;
        if configuration.count != MAXIMUM_VALUES as u64 {
            return Err("unchanged Signal form no longer produces sixteen values".to_owned());
        }
        let mut values = HostedValueStore::new(
            MAXIMUM_STORED_ITEMS,
            SIGNAL_ENCODED_LEN,
            MAXIMUM_STORED_BYTES,
        )
        .map_err(|error| format!("{error:?}"))?;
        let mut signal_values = Vec::with_capacity(MAXIMUM_VALUES);
        for sequence in 0..configuration.count {
            let signal = Signal {
                sequence,
                level: conduit_signal::signal_level_for_sequence(
                    sequence,
                    configuration.initial_level,
                ),
            };
            signal_values.push(
                values
                    .store(&encode_signal(&signal).encoded)
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

        let mut routes = FixedRoutes::<PORTS, 1>::new(PORTS as u16);
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
        let driver = OperationDriver::new(PulseOperation::new(signal_values, waits))
            .map_err(|error| format!("{error:?}"))?;
        let sign_bytes = u32::from(SIGN_ITEMS)
            .checked_mul(core::mem::size_of::<conduit_kernel::KernelEvent>() as u32)
            .ok_or_else(|| "source sign byte bound overflow".to_owned())?;
        let sign =
            HostedSignLog::new(SIGN_ITEMS, sign_bytes).map_err(|error| format!("{error:?}"))?;
        let scheduler = SourceScheduler::new_with_host_operations(
            lowered
                .node_specs
                .clone()
                .try_into()
                .map_err(|_| "source node width".to_owned())?,
            lowered
                .cords
                .iter()
                .map(|cord| cord.spec)
                .collect::<Vec<_>>()
                .try_into()
                .map_err(|_| "source cord width".to_owned())?,
            routes,
            host_bindings,
            [driver],
            values,
            sign,
        )
        .map_err(|error| format!("{error:?}"))?;
        let active_play =
            bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
        if active_play.active_play_id != binding.source_active_play_id {
            return Err("planned source play disagrees with session".to_owned());
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
            sign: scheduler.signs().allocation_capacity(),
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

    pub fn source_host_id(&self) -> &HostId {
        &self.fragment.host_id
    }

    pub fn binding(&self) -> &SessionBinding {
        &self.binding
    }

    pub fn checkpoint_offer(&self) -> SessionCheckpointOffer<'_> {
        self.session.checkpoint_offer()
    }

    pub fn resume_with_line(
        &mut self,
        line: &conduit_core::AdmittedLine,
        peer: SessionCheckpointOffer<'_>,
    ) -> Result<SessionCheckpointAcceptance, String> {
        let remote = &self.lowered.remote_endpoints[0];
        let connection = self
            .fragment
            .connections
            .iter()
            .find(|connection| connection.connection_id == remote.connection_id)
            .ok_or_else(|| "planned source connection missing".to_owned())?;
        let binding = SessionBinding::from_planned_connection_with_line(
            self.fragment.plan_id.clone(),
            remote.source_fragment_id.clone(),
            remote.sink_fragment_id.clone(),
            connection,
            line,
        )
        .and_then(|binding| {
            binding.with_observed_boots(
                self.binding.source.boot_id.clone(),
                self.binding.sink.boot_id.clone(),
            )
        })
        .map_err(|error| format!("{error:?}"))?;
        let acceptance = self
            .session
            .resume_with_attachment(binding.clone(), peer)
            .map_err(|error| format!("{error:?}"))?;
        self.binding = binding;
        Ok(acceptance)
    }

    pub fn observe_sink_boot(&mut self, sink_boot: BootId) -> Result<(), String> {
        let binding = self
            .binding
            .clone()
            .with_observed_boots(self.binding.source.boot_id.clone(), sink_boot)
            .map_err(|error| format!("{error:?}"))?;
        self.session = SessionMachine::new(binding.clone(), SessionRole::Source)
            .map_err(|error| format!("{error:?}"))?;
        self.binding = binding;
        Ok(())
    }

    pub fn fragment(&self) -> &PlanFragment {
        &self.fragment
    }

    pub fn admit_outbound(&mut self, frame: SessionFrame<'_>) -> Result<(), String> {
        self.session
            .admit_outbound(frame)
            .map_err(|error| format!("{error:?}"))
    }

    pub fn admit_inbound(&mut self, frame: SessionFrame<'_>) -> Result<(), String> {
        self.session
            .admit_inbound(frame)
            .map_err(|error| format!("{error:?}"))
    }

    pub fn is_active(&self) -> bool {
        self.session.is_active()
    }

    pub fn is_terminal(&self) -> bool {
        self.session.is_terminal()
    }

    pub fn next_offer(
        &mut self,
    ) -> Result<Option<(u64, [u8; SIGNAL_ENCODED_LEN as usize])>, String> {
        let (endpoint, cord) = self.remote();
        loop {
            if let Some(offer) = self
                .scheduler
                .remote_egress_offer(endpoint, cord)
                .map_err(|error| format!("{error:?}"))?
            {
                let payload = self
                    .scheduler
                    .host_value(offer.value)
                    .map_err(|error| format!("{error:?}"))?
                    .try_into()
                    .map_err(|_| "kernel emitted a non-Signal payload width".to_owned())?;
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
                SchedulerStatus::Idle => return Err("source kernel became idle early".to_owned()),
                SchedulerStatus::Cancelled => return Err("source kernel cancelled".to_owned()),
            }
        }
    }

    pub fn pressure(&mut self, sequence: u64) -> Result<(), String> {
        if sequence != self.session.next_sequence() {
            return Err("pressure sequence disagrees with source session".to_owned());
        }
        self.pressure_retries = self.pressure_retries.saturating_add(1);
        Ok(())
    }

    pub fn accepted(&mut self, sequence: u64) -> Result<(), String> {
        let (endpoint, cord) = self.remote();
        self.scheduler
            .remote_egress_accept(endpoint, cord, sequence)
            .map_err(|error| format!("{error:?}"))
    }

    pub fn delivered(&mut self, sequence: u64) -> Result<(), String> {
        let (endpoint, cord) = self.remote();
        self.scheduler
            .remote_egress_delivered(endpoint, cord, sequence)
            .map_err(|error| format!("{error:?}"))
    }

    pub fn finish_kernel(&mut self) -> Result<u64, String> {
        let (endpoint, cord) = self.remote();
        if !self
            .scheduler
            .remote_egress_terminal(endpoint, cord)
            .map_err(|error| format!("{error:?}"))?
        {
            return Err("source remote egress did not become terminal".to_owned());
        }
        if self.scheduler.values().used_items() != 0
            || self
                .scheduler
                .cord_usage(cord)
                .map_err(|error| format!("{error:?}"))?
                != (0, 0)
            || !self
                .scheduler
                .signs()
                .contains_kind(KernelEventKind::RemoteValueDelivered)
            || !self
                .scheduler
                .signs()
                .contains_kind(KernelEventKind::OperationCompleted)
            || self.capacity_seal() != self.seal
        {
            return Err("source kernel terminal/capacity invariants failed".to_owned());
        }
        Ok(MAXIMUM_VALUES as u64)
    }

    pub fn pressure_retries(&self) -> u32 {
        self.pressure_retries
    }

    /// Cancel the exact source kernel when the owning session reaches a
    /// cancelled or failed disposition.
    pub fn cancel(&mut self) -> Result<(), String> {
        self.scheduler
            .cancel()
            .map_err(|error| format!("{error:?}"))
    }

    fn remote(&self) -> (RemoteEndpointId, CordId) {
        let remote = &self.lowered.remote_endpoints[0];
        (remote.endpoint, remote.cord)
    }

    fn complete_wait(&mut self, request: HostOperationRequest) -> Result<(), String> {
        let identity = self
            .identity
            .request(request.node, request.request)
            .ok_or_else(|| "unbound std wait request".to_owned())?;
        if identity.operation != request.operation {
            return Err("std wait operation identity mismatch".to_owned());
        }
        let duration = u64::from_le_bytes(
            self.scheduler
                .host_value(request.input.value)
                .map_err(|error| format!("{error:?}"))?
                .try_into()
                .map_err(|_| "std wait input width".to_owned())?,
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

    fn capacity_seal(&self) -> CapacitySeal {
        CapacitySeal {
            values: self.scheduler.values().allocation_capacities(),
            sign: self.scheduler.signs().allocation_capacity(),
            driver: self.scheduler.drivers()[0]
                .operation()
                .allocation_capacity(),
            identity: self.identity.allocation_capacities(),
        }
    }
}
