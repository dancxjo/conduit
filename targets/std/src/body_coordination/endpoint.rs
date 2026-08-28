use conduit_core::{bind_active_play, ConfigurationValue, HostId, PlanFragment};
use conduit_kernel::scheduler::{FixedScheduler, OperationDriver, SchedulerStatus};
use conduit_kernel::{
    BoundedValueRef, CordId, Failure, FailureCode, FixedHostOperationBindings, FixedRoutes,
    HostOperationDisposition, HostOperationId, HostOperationOutcome, HostedSignLog,
    HostedValueStore, KernelEventKind, Operation, OperationAction, OperationInput, PortId,
    RemoteEndpointId, RequestId, SignQuery, ValueRef, ValueStorage,
};
use conduit_plan_lowering::lowering::{
    lower_plan_fragment, KernelExecutionIdentityMap, LoweredPlanFragment, RemoteCordDirection,
    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
};
use conduit_semantic_catalog::{BodyCoordinationPlan, TEXT_PRESENTATION_KIND};
use conduit_text::{MAX_TEXT_BYTES, TEXT_LITERAL_KIND};
use conduit_wire::{SessionBinding, SessionMachine, SessionRole};

const PORTS: usize = FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const ROUTE_SLOTS: usize = 2 * PORTS;
const SIGN_ITEMS: u16 = 128;
const VALUE_ITEMS: u16 = 4;
const VALUE_BYTES: u32 = MAX_TEXT_BYTES * VALUE_ITEMS as u32;

type CoordinationScheduler = FixedScheduler<
    OperationDriver<CoordinationOperation, PORTS>,
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

enum CoordinationOperation {
    Literal { value: ValueRef, emitted: bool },
    Presentation { pending: Option<RequestId> },
}

impl CoordinationOperation {
    fn fail(detail: u16) -> OperationAction {
        OperationAction::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            detail,
        })
    }
}

impl Operation for CoordinationOperation {
    fn start(&mut self) -> OperationAction {
        match self {
            Self::Literal { value, .. } => OperationAction::Emit {
                port: PortId(0),
                value: *value,
            },
            Self::Presentation { .. } => OperationAction::Await,
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match (self, input) {
            (
                Self::Presentation { pending },
                OperationInput::Value {
                    port: PortId(0),
                    value,
                },
            ) if pending.is_none() => {
                *pending = Some(RequestId(0));
                OperationAction::RequestHostOperation {
                    request: RequestId(0),
                    operation: HostOperationId(0),
                    input: match BoundedValueRef::new(value, MAX_TEXT_BYTES) {
                        Ok(value) => value,
                        Err(_) => return Self::fail(1),
                    },
                }
            }
            (
                Self::Presentation { pending },
                OperationInput::HostOperationCompleted { request, outcome },
            ) if *pending == Some(request)
                && outcome.disposition == HostOperationDisposition::Completed
                && outcome.output.is_none()
                && outcome.failure.is_none() =>
            {
                *pending = None;
                OperationAction::Await
            }
            (Self::Presentation { pending }, OperationInput::Closed { port: PortId(0) })
                if pending.is_none() =>
            {
                OperationAction::Complete
            }
            _ => Self::fail(2),
        }
    }

    fn advance(&mut self) -> OperationAction {
        match self {
            Self::Literal { emitted, .. } if !*emitted => {
                *emitted = true;
                OperationAction::Complete
            }
            _ => OperationAction::Await,
        }
    }
}

struct EndpointSession {
    direction: RemoteCordDirection,
    endpoint: RemoteEndpointId,
    cord: CordId,
    binding: SessionBinding,
    machine: SessionMachine,
}

pub struct CoordinationOffer {
    pub sequence: u64,
    pub bytes: Vec<u8>,
}

pub struct CoordinationEndpoint {
    scheduler: CoordinationScheduler,
    fragment: PlanFragment,
    lowered: LoweredPlanFragment,
    sessions: Vec<EndpointSession>,
    identity: KernelExecutionIdentityMap,
    received: String,
}

impl CoordinationEndpoint {
    pub fn prepare(exact: &BodyCoordinationPlan, host_id: &HostId) -> Result<Self, String> {
        let fragment = exact
            .plan
            .fragments
            .iter()
            .find(|fragment| &fragment.host_id == host_id)
            .cloned()
            .ok_or_else(|| format!("coordination fragment missing for {}", host_id.as_str()))?;
        let lowered = lower_plan_fragment(&fragment).map_err(|error| format!("{error:?}"))?;
        if lowered.nodes.len() != 2
            || lowered.cords.len() != 2
            || lowered.remote_endpoints.len() != 2
            || lowered.host_operations.len() != 1
        {
            return Err(format!(
                "coordination fragment shape nodes={} cords={} remote={} host_ops={}",
                lowered.nodes.len(),
                lowered.cords.len(),
                lowered.remote_endpoints.len(),
                lowered.host_operations.len()
            ));
        }
        let mut values = HostedValueStore::new(VALUE_ITEMS, MAX_TEXT_BYTES, VALUE_BYTES)
            .map_err(|error| format!("{error:?}"))?;
        let mut drivers = Vec::with_capacity(2);
        for placement in &fragment.placements {
            let operation = match placement.kind_id.as_str() {
                TEXT_LITERAL_KIND => {
                    let text = placement
                        .configuration
                        .iter()
                        .find_map(|entry| match (&*entry.key, &entry.value) {
                            ("value", ConfigurationValue::Text(value))
                                if value.len() <= MAX_TEXT_BYTES as usize =>
                            {
                                Some(value.as_str())
                            }
                            _ => None,
                        })
                        .ok_or("coordination literal is missing or oversized")?;
                    let value = values
                        .store(text.as_bytes())
                        .map_err(|error| format!("{error:?}"))?;
                    CoordinationOperation::Literal {
                        value,
                        emitted: false,
                    }
                }
                TEXT_PRESENTATION_KIND => CoordinationOperation::Presentation { pending: None },
                kind => return Err(format!("unsupported coordination Kind {kind}")),
            };
            drivers.push(OperationDriver::new(operation).map_err(|error| format!("{error:?}"))?);
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
        let host_operation = &lowered.host_operations[0];
        host_bindings
            .install(host_operation.node, host_operation.binding)
            .map_err(|error| format!("{error:?}"))?;
        host_bindings.seal().map_err(|error| format!("{error:?}"))?;
        let sign_bytes = u32::from(SIGN_ITEMS)
            .checked_mul(core::mem::size_of::<conduit_kernel::KernelEvent>() as u32)
            .ok_or("coordination Sign budget overflow")?;
        let scheduler = CoordinationScheduler::new_with_host_operations(
            lowered
                .node_specs
                .clone()
                .try_into()
                .map_err(|_| "coordination node width")?,
            lowered
                .cords
                .iter()
                .map(|cord| cord.spec)
                .collect::<Vec<_>>()
                .try_into()
                .map_err(|_| "coordination Cord width")?,
            routes,
            host_bindings,
            drivers
                .try_into()
                .map_err(|_| "coordination driver width")?,
            values,
            HostedSignLog::new_with_remote_storage(
                SIGN_ITEMS,
                sign_bytes,
                SIGN_ITEMS,
                conduit_kernel::remote_sign_storage_bytes(SIGN_ITEMS)
                    .ok_or("coordination remote Sign byte overflow")?,
            )
            .map_err(|error| format!("{error:?}"))?,
        )
        .map_err(|error| format!("{error:?}"))?;
        let active_play =
            bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
        let identity = KernelExecutionIdentityMap::new(&lowered.identity, &active_play, 1, 0, 1)
            .map_err(|error| format!("{error:?}"))?;
        let mut sessions = Vec::with_capacity(2);
        for remote in &lowered.remote_endpoints {
            let connection = fragment
                .connections
                .iter()
                .find(|connection| connection.connection_id == remote.connection_id)
                .ok_or("coordination remote connection missing")?;
            let binding = SessionBinding::from_planned_connection(
                fragment.plan_id.clone(),
                remote.source_fragment_id.clone(),
                remote.sink_fragment_id.clone(),
                connection,
            )
            .map_err(|error| format!("{error:?}"))?;
            let role = match remote.direction {
                RemoteCordDirection::Egress => SessionRole::Source,
                RemoteCordDirection::Ingress => SessionRole::Sink,
            };
            let machine =
                SessionMachine::new(binding.clone(), role).map_err(|error| format!("{error:?}"))?;
            sessions.push(EndpointSession {
                direction: remote.direction,
                endpoint: remote.endpoint,
                cord: remote.cord,
                binding,
                machine,
            });
        }
        Ok(Self {
            scheduler,
            fragment,
            lowered,
            sessions,
            identity,
            received: String::with_capacity(MAX_TEXT_BYTES as usize),
        })
    }

    pub fn binding(&self, direction: RemoteCordDirection) -> &SessionBinding {
        &self.session(direction).binding
    }

    pub fn cord(&self, direction: RemoteCordDirection) -> CordId {
        self.session(direction).cord
    }

    pub fn session_mut(&mut self, direction: RemoteCordDirection) -> &mut SessionMachine {
        &mut self.endpoint_session_mut(direction).machine
    }

    pub fn next_offer(&mut self) -> Result<CoordinationOffer, String> {
        loop {
            self.complete_presentation_request()?;
            let (endpoint, cord) = self.remote(RemoteCordDirection::Egress);
            if let Some(offer) = self
                .scheduler
                .remote_egress_offer(endpoint, cord)
                .map_err(|error| format!("{error:?}"))?
            {
                return Ok(CoordinationOffer {
                    sequence: offer.sequence,
                    bytes: self
                        .scheduler
                        .host_value(offer.value)
                        .map_err(|error| format!("{error:?}"))?
                        .to_vec(),
                });
            }
            match self
                .scheduler
                .step()
                .map_err(|error| format!("{error:?}"))?
            {
                SchedulerStatus::Progress { .. } => {}
                SchedulerStatus::Idle => {
                    return Err("coordination endpoint idle before offer".into())
                }
                SchedulerStatus::Complete => {
                    return Err("coordination endpoint completed before offer".into())
                }
                SchedulerStatus::Cancelled => return Err("coordination endpoint cancelled".into()),
            }
        }
    }

    pub fn accept_offer(&mut self, sequence: u64) -> Result<(), String> {
        let (endpoint, cord) = self.remote(RemoteCordDirection::Egress);
        self.scheduler
            .remote_egress_accept(endpoint, cord, sequence)
            .map_err(|error| format!("{error:?}"))
    }

    pub fn deliver_offer(&mut self, sequence: u64) -> Result<(), String> {
        let (endpoint, cord) = self.remote(RemoteCordDirection::Egress);
        self.scheduler
            .remote_egress_delivered(endpoint, cord, sequence)
            .map_err(|error| format!("{error:?}"))
    }

    pub fn admit_input(&mut self, sequence: u64, bytes: &[u8]) -> Result<(), String> {
        let (endpoint, cord) = self.remote(RemoteCordDirection::Ingress);
        self.scheduler
            .admit_remote_input(endpoint, cord, sequence, bytes)
            .map(|_| ())
            .map_err(|error| format!("{error:?}"))?;
        self.drive_until_presented(bytes.len())
    }

    pub fn received(&self) -> &str {
        &self.received
    }

    pub fn close_input(&mut self) -> Result<(), String> {
        let (endpoint, cord) = self.remote(RemoteCordDirection::Ingress);
        self.scheduler
            .close_remote_input(endpoint, cord)
            .map_err(|error| format!("{error:?}"))
    }

    pub fn finish(&mut self) -> Result<(), String> {
        loop {
            if self.complete_presentation_request()? {
                continue;
            }
            match self
                .scheduler
                .step()
                .map_err(|error| format!("{error:?}"))?
            {
                SchedulerStatus::Progress { .. } => {}
                SchedulerStatus::Complete => break,
                SchedulerStatus::Idle => return Err("coordination kernel idle at terminal".into()),
                SchedulerStatus::Cancelled => return Err("coordination kernel cancelled".into()),
            }
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
            return Err("coordination kernel lacks delivered/terminal Signs".into());
        }
        Ok(())
    }

    pub fn fragment(&self) -> &PlanFragment {
        &self.fragment
    }

    pub fn identity(&self) -> &KernelExecutionIdentityMap {
        &self.identity
    }

    pub fn lowered(&self) -> &LoweredPlanFragment {
        &self.lowered
    }

    fn complete_presentation_request(&mut self) -> Result<bool, String> {
        let Some(request) = self.scheduler.next_host_request() else {
            return Ok(false);
        };
        let bytes = self
            .scheduler
            .host_value(request.input.value)
            .map_err(|error| format!("{error:?}"))?;
        let text = core::str::from_utf8(bytes).map_err(|error| error.to_string())?;
        if self.received.len() + text.len() > MAX_TEXT_BYTES as usize {
            return Err("coordination presentation exceeds admitted bytes".into());
        }
        self.received.push_str(text);
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
            .map_err(|error| format!("{error:?}"))?;
        Ok(true)
    }

    fn drive_until_presented(&mut self, expected: usize) -> Result<(), String> {
        let target = self.received.len() + expected;
        while self.received.len() < target {
            if self.complete_presentation_request()? {
                continue;
            }
            match self
                .scheduler
                .step()
                .map_err(|error| format!("{error:?}"))?
            {
                SchedulerStatus::Progress { .. } => {}
                SchedulerStatus::Idle => return Err("coordination presentation idle".into()),
                SchedulerStatus::Complete => {
                    return Err("coordination presentation completed too early".into())
                }
                SchedulerStatus::Cancelled => return Err("coordination endpoint cancelled".into()),
            }
        }
        Ok(())
    }

    fn remote(&self, direction: RemoteCordDirection) -> (RemoteEndpointId, CordId) {
        let session = self.session(direction);
        (session.endpoint, session.cord)
    }

    fn session(&self, direction: RemoteCordDirection) -> &EndpointSession {
        self.sessions
            .iter()
            .find(|session| session.direction == direction)
            .expect("exact coordination direction exists")
    }

    fn endpoint_session_mut(&mut self, direction: RemoteCordDirection) -> &mut EndpointSession {
        self.sessions
            .iter_mut()
            .find(|session| session.direction == direction)
            .expect("exact coordination direction exists")
    }
}
