//! Kernel-owned source of volatile R1 network credentials for the exact Pico
//! UsbCdc bootstrap Plan.

use conduit_core::{bind_active_play, BootId, PlanFragment};
use conduit_kernel::scheduler::{
    FixedScheduler, HostOperationRequest, OperationDriver, SchedulerStatus,
};
use conduit_kernel::{
    BoundedValueRef, CordId, Failure, FailureCode, FixedHostOperationBindings, FixedRoutes,
    HostOperationDisposition, HostOperationId, HostOperationOutcome, HostedSignLog,
    HostedValueStore, KernelEventKind, Operation, OperationAction, OperationInput, PortId,
    RemoteEndpointId, RequestId, SignQuery, ValueStorage,
};
use conduit_runtime::lowering::{
    lower_plan_fragment, RemoteCordDirection, MAXIMUM_KERNEL_PORTS_PER_NODE,
};
use conduit_wire::{SessionBinding, SessionFrame, SessionMachine, SessionRole};

const PORTS: usize = MAXIMUM_KERNEL_PORTS_PER_NODE;
const VALUE_ITEMS: u16 = 2;
const SIGN_ITEMS: u16 = 32;

struct VolatileCredentials {
    ssid: [u8; conduit_net::MAXIMUM_SSID_BYTES],
    ssid_len: usize,
    credential: [u8; conduit_net::MAXIMUM_CREDENTIAL_BYTES],
    credential_len: usize,
}

impl VolatileCredentials {
    fn new(ssid: &[u8], credential: &[u8]) -> Result<Self, String> {
        let mut encoded = [0_u8; conduit_net::MAXIMUM_JOIN_INPUT_BYTES as usize];
        conduit_net::encode_network_join_request(
            conduit_net::NetworkJoinRequest { ssid, credential },
            &mut encoded,
        )
        .map_err(|error| format!("invalid bounded network credentials: {error:?}"))?;
        encoded.fill(0);
        let mut value = Self {
            ssid: [0; conduit_net::MAXIMUM_SSID_BYTES],
            ssid_len: ssid.len(),
            credential: [0; conduit_net::MAXIMUM_CREDENTIAL_BYTES],
            credential_len: credential.len(),
        };
        value.ssid[..ssid.len()].copy_from_slice(ssid);
        value.credential[..credential.len()].copy_from_slice(credential);
        Ok(value)
    }

    fn encode_and_clear(
        &mut self,
    ) -> Result<([u8; conduit_net::MAXIMUM_JOIN_INPUT_BYTES as usize], usize), String> {
        let mut encoded = [0_u8; conduit_net::MAXIMUM_JOIN_INPUT_BYTES as usize];
        let encoded_len = conduit_net::encode_network_join_request(
            conduit_net::NetworkJoinRequest {
                ssid: &self.ssid[..self.ssid_len],
                credential: &self.credential[..self.credential_len],
            },
            &mut encoded,
        )
        .map_err(|error| format!("network credential encoding failed: {error:?}"))?;
        self.ssid.fill(0);
        self.credential.fill(0);
        self.ssid_len = 0;
        self.credential_len = 0;
        Ok((encoded, encoded_len))
    }
}

impl Drop for VolatileCredentials {
    fn drop(&mut self) {
        self.ssid.fill(0);
        self.credential.fill(0);
    }
}

struct CredentialOperation {
    output_port: PortId,
    operation: HostOperationId,
    pending: bool,
    emitted: bool,
    empty: conduit_kernel::ValueRef,
    output: conduit_kernel::ValueRef,
}

impl Operation for CredentialOperation {
    fn start(&mut self) -> OperationAction {
        self.pending = true;
        OperationAction::RequestHostOperation {
            request: RequestId(0),
            operation: self.operation,
            input: BoundedValueRef::new(self.empty, 1)
                .expect("empty credential request is bounded"),
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending
                    && request == RequestId(0)
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.failure.is_none() =>
            {
                let Some(output) = outcome.output else {
                    return Self::fail(1);
                };
                self.pending = false;
                self.emitted = true;
                OperationAction::Emit {
                    port: self.output_port,
                    value: output.value,
                }
            }
            _ => Self::fail(2),
        }
    }

    fn advance(&mut self) -> OperationAction {
        if self.emitted {
            OperationAction::Complete
        } else {
            Self::fail(3)
        }
    }
}

impl CredentialOperation {
    fn fail(detail: u16) -> OperationAction {
        OperationAction::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            detail,
        })
    }
}

type CredentialScheduler = FixedScheduler<
    OperationDriver<CredentialOperation, PORTS>,
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

pub struct PicoWifiBootstrapSource {
    scheduler: CredentialScheduler,
    fragment: PlanFragment,
    endpoint: RemoteEndpointId,
    cord: CordId,
    operation_node: conduit_kernel::NodeId,
    operation: HostOperationId,
    binding: SessionBinding,
    session: SessionMachine,
}

impl PicoWifiBootstrapSource {
    pub fn prepare(ssid: &[u8], credential: &[u8]) -> Result<Self, String> {
        let mut credentials = VolatileCredentials::new(ssid, credential)?;
        let exact = conduit_r1_network_conformance::exact_r1_network_bootstrap_plan()?;
        let fragment = exact
            .plan
            .fragments
            .iter()
            .find(|fragment| {
                fragment.host_id.as_str() == conduit_r1_network_conformance::R1_STD_HOST_ID
            })
            .cloned()
            .ok_or_else(|| "R1 std bootstrap fragment missing".to_owned())?;
        let lowered = lower_plan_fragment(&fragment).map_err(|error| format!("{error:?}"))?;
        if lowered.nodes.len() != 1
            || lowered.cords.len() != 1
            || lowered.remote_endpoints.len() != 1
            || lowered.remote_endpoints[0].direction != RemoteCordDirection::Egress
            || lowered.host_operations.len() != 1
        {
            return Err("R1 std bootstrap fragment shape changed".to_owned());
        }
        let remote = &lowered.remote_endpoints[0];
        let connection = fragment
            .connections
            .iter()
            .find(|connection| connection.connection_id == remote.connection_id)
            .ok_or_else(|| "R1 bootstrap connection missing".to_owned())?;
        let binding = SessionBinding::from_planned_connection(
            fragment.plan_id.clone(),
            remote.source_fragment_id.clone(),
            remote.sink_fragment_id.clone(),
            connection,
        )
        .map_err(|error| format!("{error:?}"))?;
        let mut values = HostedValueStore::new(
            VALUE_ITEMS,
            conduit_net::MAXIMUM_JOIN_INPUT_BYTES,
            conduit_net::MAXIMUM_JOIN_INPUT_BYTES,
        )
        .map_err(|error| format!("{error:?}"))?;
        let empty = values.store(&[]).map_err(|error| format!("{error:?}"))?;
        let (mut encoded, encoded_len) = credentials.encode_and_clear()?;
        let output = values
            .store(&encoded[..encoded_len])
            .map_err(|error| format!("{error:?}"))?;
        encoded.fill(0);
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
        let output_port = lowered.nodes[0]
            .outputs
            .first()
            .map(|port| port.port)
            .ok_or_else(|| "credential source output missing".to_owned())?;
        let operation = lowered.host_operations[0].binding.operation;
        let driver = OperationDriver::new(CredentialOperation {
            output_port,
            operation,
            pending: false,
            emitted: false,
            empty,
            output,
        })
        .map_err(|error| format!("{error:?}"))?;
        let sign_bytes = u32::from(SIGN_ITEMS)
            .checked_mul(core::mem::size_of::<conduit_kernel::KernelEvent>() as u32)
            .ok_or_else(|| "credential source sign bound overflow".to_owned())?;
        let remote_sign_bytes = conduit_kernel::remote_sign_storage_bytes(SIGN_ITEMS)
            .ok_or_else(|| "credential source remote sign bound overflow".to_owned())?;
        let sign = HostedSignLog::new_with_remote_storage(
            SIGN_ITEMS,
            sign_bytes,
            SIGN_ITEMS,
            remote_sign_bytes,
        )
        .map_err(|error| format!("{error:?}"))?;
        let scheduler = CredentialScheduler::new_with_host_operations(
            lowered
                .node_specs
                .clone()
                .try_into()
                .map_err(|_| "R1 source node width")?,
            lowered
                .cords
                .iter()
                .map(|cord| cord.spec)
                .collect::<Vec<_>>()
                .try_into()
                .map_err(|_| "R1 source cord width")?,
            routes,
            host_bindings,
            [driver],
            values,
            sign,
        )
        .map_err(|error| format!("{error:?}"))?;
        let active = bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
        if active.active_play_id != binding.source_active_play_id {
            return Err("R1 source Play disagrees with session".to_owned());
        }
        let session = SessionMachine::new(binding.clone(), SessionRole::Source)
            .map_err(|error| format!("{error:?}"))?;
        Ok(Self {
            scheduler,
            fragment,
            endpoint: remote.endpoint,
            cord: remote.cord,
            operation_node: lowered.host_operations[0].node,
            operation,
            binding,
            session,
        })
    }

    pub fn binding(&self) -> &SessionBinding {
        &self.binding
    }
    pub fn fragment(&self) -> &PlanFragment {
        &self.fragment
    }
    pub fn is_active(&self) -> bool {
        self.session.is_active()
    }
    pub fn is_terminal(&self) -> bool {
        self.session.is_terminal()
    }

    pub fn observe_sink_boot(&mut self, sink_boot: BootId) -> Result<(), String> {
        self.binding = self
            .binding
            .clone()
            .with_observed_boots(self.binding.source.boot_id.clone(), sink_boot)
            .map_err(|error| format!("{error:?}"))?;
        self.session = SessionMachine::new(self.binding.clone(), SessionRole::Source)
            .map_err(|error| format!("{error:?}"))?;
        Ok(())
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

    pub fn next_offer(
        &mut self,
    ) -> Result<
        Option<(
            u64,
            [u8; conduit_net::MAXIMUM_JOIN_INPUT_BYTES as usize],
            usize,
        )>,
        String,
    > {
        loop {
            if let Some(offer) = self
                .scheduler
                .remote_egress_offer(self.endpoint, self.cord)
                .map_err(|error| format!("{error:?}"))?
            {
                let bytes = self
                    .scheduler
                    .host_value(offer.value)
                    .map_err(|error| format!("{error:?}"))?;
                let mut payload = [0_u8; conduit_net::MAXIMUM_JOIN_INPUT_BYTES as usize];
                payload[..bytes.len()].copy_from_slice(bytes);
                return Ok(Some((offer.sequence, payload, bytes.len())));
            }
            if let Some(request) = self.scheduler.next_host_request() {
                self.complete_credentials(request)?;
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
                    return Err("credential source became idle early".to_owned())
                }
                SchedulerStatus::Cancelled => return Err("credential source cancelled".to_owned()),
            }
        }
    }

    pub fn accepted(&mut self, sequence: u64) -> Result<(), String> {
        self.scheduler
            .remote_egress_accept(self.endpoint, self.cord, sequence)
            .map_err(|error| format!("{error:?}"))
    }

    pub fn delivered(&mut self, sequence: u64) -> Result<(), String> {
        self.scheduler
            .remote_egress_delivered(self.endpoint, self.cord, sequence)
            .map_err(|error| format!("{error:?}"))
    }

    pub fn finish_kernel(&mut self) -> Result<u64, String> {
        if !self
            .scheduler
            .remote_egress_terminal(self.endpoint, self.cord)
            .map_err(|error| format!("{error:?}"))?
        {
            return Err("credential source egress is not terminal".to_owned());
        }
        if !self
            .scheduler
            .signs()
            .contains_kind(KernelEventKind::RemoteValueDelivered)
            || !self
                .scheduler
                .signs()
                .contains_kind(KernelEventKind::OperationCompleted)
        {
            return Err("credential source terminal signs missing".to_owned());
        }
        Ok(1)
    }

    fn complete_credentials(&mut self, request: HostOperationRequest) -> Result<(), String> {
        if request.node != self.operation_node
            || request.operation != self.operation
            || request.request != RequestId(0)
        {
            return Err("credential host operation identity mismatch".to_owned());
        }
        let value = self.scheduler.drivers()[0].operation().output;
        self.scheduler
            .complete_host_operation(
                request.node,
                request.request,
                HostOperationOutcome {
                    disposition: HostOperationDisposition::Completed,
                    output: Some(
                        BoundedValueRef::new(value, conduit_net::MAXIMUM_JOIN_INPUT_BYTES)
                            .expect("credential output is bounded"),
                    ),
                    failure: None,
                },
            )
            .map_err(|error| format!("{error:?}"))
    }
}
