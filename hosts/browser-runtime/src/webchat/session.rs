use super::BrowserChatOperation;
use conduit_core::{bind_active_play, BootId, ConnectionBase, HostId};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
use conduit_kernel::scheduler::{
    CordSpec, FixedScheduler, HostOperationRequest, NodeSpec, OperationDriver, SchedulerStatus,
};
use conduit_kernel::{
    BoundedValueRef, CordEndpoint, CordId, Failure, FailureCode, FixedHostOperationBindings,
    FixedRoutes, HostOperationDisposition, HostOperationOutcome, HostedSignLog, HostedValueStore,
    NodeId, PortId, ValueStorage,
};
use conduit_planner::{plan_expanded_canonical_with_options, PlanningOptions};
use conduit_runtime::lowering::{
    lower_plan_fragment, KernelExecutionIdentityMap, KernelIdentityMap,
    MAXIMUM_KERNEL_PORTS_PER_NODE,
};
use std::collections::BTreeMap;
use std::fmt::Write as _;

const SOURCE: &str = include_str!("../../../../examples/webchat.conduit");
const NODES: usize = 3;
const CORDS: usize = 2;
const PORTS: usize = MAXIMUM_KERNEL_PORTS_PER_NODE;
const QUEUE_SLOTS: usize = 8;
const ROUTE_SLOTS: usize = NODES * PORTS;
const ROUTE_TARGETS: usize = 2;
const ACTIVE_HOST_OPERATIONS: usize = 6;
const HOST_BINDINGS: usize = NODES * 4;
const PENDING_REQUESTS: usize = 3;
const VALUE_ITEMS: u16 = 32;
const VALUE_BYTES: u32 = 8_192;
const SIGN_ITEMS: u16 = 1_024;
const REQUEST_IDENTITIES: usize = 64;

type ChatScheduler = FixedScheduler<
    OperationDriver<BrowserChatOperation, PORTS>,
    HostedValueStore,
    HostedSignLog,
    NODES,
    CORDS,
    PORTS,
    QUEUE_SLOTS,
    ROUTE_SLOTS,
    ROUTE_TARGETS,
    HOST_BINDINGS,
    PENDING_REQUESTS,
>;

#[repr(i32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BrowserChatEffect {
    None,
    SocketOpen,
    SocketReceive,
    SocketSend,
    SocketClose,
    ListAppend,
}

pub(crate) struct BrowserChatSession {
    scheduler: ChatScheduler,
    lowered_identity: KernelIdentityMap,
    identity: KernelExecutionIdentityMap,
    current: Option<HostOperationRequest>,
    parked_input: Option<HostOperationRequest>,
    parked_receive: Option<HostOperationRequest>,
    complete: bool,
    disconnected: bool,
    error: i32,
    identity_text: Vec<u8>,
    value_capacity: (usize, usize),
    identity_capacity: (usize, usize, usize),
}

impl BrowserChatSession {
    pub(crate) fn prepare(url: &str, host_id: HostId, boot_id: BootId) -> Result<Self, i32> {
        if url.len() > 256 || !url.starts_with("ws://") {
            return Err(-201);
        }
        if host_id.as_str().is_empty()
            || boot_id.as_str().is_empty()
            || host_id.as_str().len() > 128
            || boot_id.as_str().len() > 128
        {
            return Err(-201);
        }
        let source = SOURCE.replace("ws://127.0.0.1:4178", url);
        let mut startup = StartupCatalog::new();
        let mut profile = ProfileCatalog::new();
        conduit_net::install_external_websocket_catalogs(&mut startup, &mut profile)
            .map_err(|_| -202)?;
        conduit_chat::install_browser_chat_catalogs(&mut startup, &mut profile)
            .map_err(|_| -202)?;
        let checked =
            check_syntax_document(&parse_syntax_document(&source), &startup).map_err(|_| -203)?;
        let expanded =
            expand_canonical_form(&checked, "webchat-browser-demo", &profile).map_err(|_| -204)?;
        let advertisement = super::catalog::advertisement(host_id, boot_id);
        let hosts = [advertisement.clone()];
        let placements =
            conduit_planner::default_expanded_placements(&expanded, &hosts).map_err(|_| -205)?;
        let connection_bases = BTreeMap::new();
        let line_candidates = BTreeMap::new();
        let plan = plan_expanded_canonical_with_options(
            &expanded,
            &hosts,
            &placements,
            &[ConnectionBase::Local],
            PlanningOptions {
                connection_bases: &connection_bases,
                line_candidates: &line_candidates,
                connection_item_capacity: 4,
                connection_byte_capacity: 1_024,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
        )
        .map_err(|_| -206)?;
        let fragment = plan.fragments.into_iter().next().ok_or(-207)?;
        let lowered = lower_plan_fragment(&fragment).map_err(|_| -208)?;
        if lowered.nodes.len() != NODES
            || lowered.cords.len() != CORDS
            || lowered.cord_value_slots as usize > QUEUE_SLOTS
            || lowered.host_operations.len() != ACTIVE_HOST_OPERATIONS
        {
            return Err(-209);
        }

        let mut values = HostedValueStore::new(
            VALUE_ITEMS,
            conduit_net::MAXIMUM_EXTERNAL_WEBSOCKET_MESSAGE_BYTES,
            VALUE_BYTES,
        )
        .map_err(|_| -210)?;
        let mut operations = Vec::with_capacity(NODES);
        for node in &lowered.nodes {
            let placement = &fragment.placements[usize::from(node.node.0)];
            let operation = match placement.kind_id.as_str() {
                conduit_chat::WEB_TEXT_INPUT_KIND => {
                    let mut tokens =
                        Vec::with_capacity(conduit_chat::MAXIMUM_CHAT_INPUT_ITEMS.into());
                    for _ in 0..conduit_chat::MAXIMUM_CHAT_INPUT_ITEMS {
                        tokens.push(values.store(&[0]).map_err(|_| -211)?);
                    }
                    BrowserChatOperation::text_input(tokens)
                }
                conduit_net::EXTERNAL_WEBSOCKET_CLIENT_KIND => {
                    let url = placement
                        .configuration
                        .iter()
                        .find_map(|entry| match (entry.key.as_str(), &entry.value) {
                            ("url", conduit_core::ConfigurationValue::Text(value)) => {
                                Some(value.as_bytes())
                            }
                            _ => None,
                        })
                        .ok_or(-212)?;
                    BrowserChatOperation::socket(
                        values.store(url).map_err(|_| -211)?,
                        values.store(&[0]).map_err(|_| -211)?,
                        values.store(&[0]).map_err(|_| -211)?,
                    )
                }
                conduit_chat::WEB_LIST_KIND => BrowserChatOperation::list(),
                _ => return Err(-213),
            };
            operations.push(operation);
        }
        let drivers: [OperationDriver<BrowserChatOperation, PORTS>; NODES] = operations
            .into_iter()
            .map(|operation| OperationDriver::new(operation).map_err(|_| -214))
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| -214)?;

        let inactive_node = NodeSpec {
            input_cords: [None; PORTS],
            maximum_step_work: 1,
        };
        let mut node_specs = [inactive_node; NODES];
        node_specs.copy_from_slice(&lowered.node_specs);
        let inactive_cord = CordSpec {
            cord: CordId(u16::MAX),
            source: CordEndpoint::local(NodeId(u16::MAX), PortId(u16::MAX)),
            sink: CordEndpoint::local(NodeId(u16::MAX), PortId(u16::MAX)),
            slot_start: u16::MAX,
            item_capacity: 0,
            byte_capacity: 0,
        };
        let mut cord_specs = [inactive_cord; CORDS];
        for (target, cord) in cord_specs.iter_mut().zip(&lowered.cords) {
            *target = cord.spec;
        }
        let mut routes = FixedRoutes::<ROUTE_SLOTS, ROUTE_TARGETS>::new(PORTS as u16);
        for route in &lowered.routes {
            routes
                .install(
                    route.source_node,
                    route.source_port,
                    route.range,
                    &route.targets,
                )
                .map_err(|_| -215)?;
        }
        routes.seal().map_err(|_| -215)?;
        let mut bindings = FixedHostOperationBindings::<HOST_BINDINGS>::new(4);
        for operation in &lowered.host_operations {
            bindings
                .install(operation.node, operation.binding)
                .map_err(|_| -216)?;
        }
        bindings.seal().map_err(|_| -216)?;
        let sign_bytes = u32::from(SIGN_ITEMS)
            .checked_mul(core::mem::size_of::<conduit_kernel::KernelEvent>() as u32)
            .ok_or(-217)?;
        let sign = HostedSignLog::new(SIGN_ITEMS, sign_bytes).map_err(|_| -217)?;
        let scheduler = ChatScheduler::new_with_active_counts_and_host_operations(
            NODES, CORDS, node_specs, cord_specs, routes, bindings, drivers, values, sign,
        )
        .map_err(|_| -218)?;
        let active_play =
            bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
        let identity = KernelExecutionIdentityMap::new(
            &lowered.identity,
            &active_play,
            REQUEST_IDENTITIES,
            0,
            0,
        )
        .map_err(|_| -219)?;
        let mut identity_text = format!(
            "source={} checked={} expanded={} plan={} fragment={} play={} host={} boot={}",
            fragment.source_document_id.as_str(),
            fragment.checked_form_id.as_str(),
            fragment.expanded_form_id.as_str(),
            fragment.plan_id.as_str(),
            fragment.fragment_id.as_str(),
            active_play.active_play_id.as_str(),
            fragment.host_id.as_str(),
            fragment.boot_id.as_str(),
        );
        for placement in &fragment.placements {
            write!(
                identity_text,
                " placement={}:operation={}:implementation={}",
                placement.placement_id.as_str(),
                placement.gear_id.as_str(),
                placement.implementation_id.as_str(),
            )
            .map_err(|_| -234)?;
            for requirement in &placement.host_operations {
                write!(
                    identity_text,
                    ":host-operation={}",
                    requirement.contract_id.as_str(),
                )
                .map_err(|_| -234)?;
            }
        }
        let identity_text = identity_text.into_bytes();
        let value_capacity = scheduler.values().allocation_capacities();
        let identity_capacity = identity.allocation_capacities();
        let mut session = Self {
            scheduler,
            lowered_identity: lowered.identity,
            identity,
            current: None,
            parked_input: None,
            parked_receive: None,
            complete: false,
            disconnected: false,
            error: 0,
            identity_text,
            value_capacity,
            identity_capacity,
        };
        session.drive()?;
        Ok(session)
    }

    pub(crate) fn effect(&self) -> BrowserChatEffect {
        self.current
            .and_then(|request| self.contract(request).ok())
            .map_or(BrowserChatEffect::None, |contract| match contract {
                conduit_net::EXTERNAL_WEBSOCKET_CLIENT_OPEN_HOST_OPERATION => {
                    BrowserChatEffect::SocketOpen
                }
                conduit_net::EXTERNAL_WEBSOCKET_CLIENT_RECEIVE_HOST_OPERATION => {
                    BrowserChatEffect::SocketReceive
                }
                conduit_net::EXTERNAL_WEBSOCKET_CLIENT_SEND_HOST_OPERATION => {
                    BrowserChatEffect::SocketSend
                }
                conduit_net::EXTERNAL_WEBSOCKET_CLIENT_CLOSE_HOST_OPERATION => {
                    BrowserChatEffect::SocketClose
                }
                conduit_chat::WEB_LIST_HOST_OPERATION => BrowserChatEffect::ListAppend,
                _ => BrowserChatEffect::None,
            })
    }

    pub(crate) fn effect_bytes(&self) -> &[u8] {
        self.current
            .and_then(|request| self.scheduler.host_value(request.input.value).ok())
            .unwrap_or(&[])
    }

    pub(crate) fn identity_text(&self) -> &[u8] {
        &self.identity_text
    }

    pub(crate) fn status(&self) -> i32 {
        if self.error < 0 {
            self.error
        } else if self.complete {
            1
        } else {
            0
        }
    }

    pub(crate) fn disconnected(&self) -> bool {
        self.disconnected
    }

    pub(crate) fn capacity_stable(&self) -> bool {
        self.scheduler.values().allocation_capacities() == self.value_capacity
            && self.identity.allocation_capacities() == self.identity_capacity
    }

    pub(crate) fn request_count(&self) -> usize {
        self.identity.lengths().0
    }

    pub(crate) fn complete_simple(&mut self, effect: BrowserChatEffect) -> Result<(), i32> {
        if self.effect() != effect {
            return Err(-220);
        }
        let request = self.current.take().ok_or(-220)?;
        let output = (effect == BrowserChatEffect::SocketSend).then_some(request.input);
        self.complete_request(request, HostOperationDisposition::Completed, output, None)?;
        self.drive()
    }

    pub(crate) fn receive(&mut self, bytes: &[u8]) -> Result<(), i32> {
        if self.effect() != BrowserChatEffect::SocketReceive
            || bytes.len() > conduit_net::MAXIMUM_EXTERNAL_WEBSOCKET_MESSAGE_BYTES as usize
        {
            return Err(-221);
        }
        let value = self.scheduler.store_host_value(bytes).map_err(|_| -222)?;
        let output =
            BoundedValueRef::new(value, conduit_net::MAXIMUM_EXTERNAL_WEBSOCKET_MESSAGE_BYTES)
                .map_err(|_| -222)?;
        let request = self.current.take().ok_or(-221)?;
        self.complete_request(
            request,
            HostOperationDisposition::Completed,
            Some(output),
            None,
        )?;
        self.drive()
    }

    pub(crate) fn submit(&mut self, bytes: &[u8]) -> Result<(), i32> {
        if bytes.is_empty()
            || bytes.len() > conduit_net::MAXIMUM_EXTERNAL_WEBSOCKET_MESSAGE_BYTES as usize
            || self.effect() != BrowserChatEffect::SocketReceive
        {
            return Err(-223);
        }
        let input_request = self.parked_input.take().ok_or(-224)?;
        let value = self.scheduler.store_host_value(bytes).map_err(|_| -225)?;
        let output =
            BoundedValueRef::new(value, conduit_net::MAXIMUM_EXTERNAL_WEBSOCKET_MESSAGE_BYTES)
                .map_err(|_| -225)?;
        self.complete_request(
            input_request,
            HostOperationDisposition::Completed,
            Some(output),
            None,
        )?;
        let receive = self.current.take().ok_or(-223)?;
        self.complete_request(
            receive,
            HostOperationDisposition::Cancelled,
            None,
            Some(Failure {
                code: FailureCode::Cancelled,
                detail: 1,
            }),
        )?;
        self.drive()
    }

    pub(crate) fn disconnect(&mut self) -> Result<(), i32> {
        if self.effect() != BrowserChatEffect::SocketReceive {
            return Err(-226);
        }
        let receive = self.current.take().ok_or(-226)?;
        self.complete_request(
            receive,
            HostOperationDisposition::Cancelled,
            None,
            Some(Failure {
                code: FailureCode::Cancelled,
                detail: 2,
            }),
        )?;
        if let Some(input) = self.parked_input.take() {
            self.complete_request(
                input,
                HostOperationDisposition::Cancelled,
                None,
                Some(Failure {
                    code: FailureCode::Cancelled,
                    detail: 2,
                }),
            )?;
        }
        self.disconnected = true;
        self.drive()
    }

    fn drive(&mut self) -> Result<(), i32> {
        loop {
            while let Some(request) = self.scheduler.next_host_request() {
                self.identity
                    .bind_request(
                        &self.lowered_identity,
                        request.node,
                        request.request,
                        request.operation,
                    )
                    .map_err(|_| -227)?;
                if self.contract(request)? == conduit_chat::WEB_TEXT_INPUT_HOST_OPERATION {
                    if self.parked_input.replace(request).is_some() {
                        return Err(-228);
                    }
                    continue;
                }
                if self.contract(request)?
                    == conduit_net::EXTERNAL_WEBSOCKET_CLIENT_RECEIVE_HOST_OPERATION
                {
                    if self.parked_receive.replace(request).is_some() {
                        return Err(-233);
                    }
                    continue;
                }
                self.current = Some(request);
                return Ok(());
            }
            match self.scheduler.step().map_err(|_| -229)? {
                SchedulerStatus::Progress { .. } => {}
                SchedulerStatus::Idle => {
                    if let Some(receive) = self.parked_receive.take() {
                        self.current = Some(receive);
                    }
                    return Ok(());
                }
                SchedulerStatus::Complete => {
                    self.complete = true;
                    return Ok(());
                }
                SchedulerStatus::Cancelled => return Err(-230),
            }
        }
    }

    fn contract(&self, request: HostOperationRequest) -> Result<&str, i32> {
        self.lowered_identity
            .host_operation_contract(request.node, request.operation)
            .map(|contract| contract.as_str())
            .ok_or(-231)
    }

    fn complete_request(
        &mut self,
        request: HostOperationRequest,
        disposition: HostOperationDisposition,
        output: Option<BoundedValueRef>,
        failure: Option<Failure>,
    ) -> Result<(), i32> {
        self.scheduler
            .complete_host_operation(
                request.node,
                request.request,
                HostOperationOutcome {
                    disposition,
                    output,
                    failure,
                },
            )
            .map_err(|_| -232)
    }
}
