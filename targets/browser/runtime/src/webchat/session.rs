use super::BrowserChatOperation;
use conduit_core::{bind_active_play, BaseImplementationId, BootId, HostId};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
use conduit_kernel::scheduler::{
    CordSpec, FixedScheduler, HostOperationRequest, NodeSpec, OperationDriver,
};
use conduit_kernel::{
    CordEndpoint, CordId, FixedHostOperationBindings, FixedRoutes, HostedSignLog, HostedValueStore,
    NodeId, PortId, ValueStorage,
};
use conduit_plan_lowering::lowering::{
    lower_plan_fragment, KernelExecutionIdentityMap, KernelIdentityMap,
    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
};
use conduit_planner::{plan_expanded_canonical_with_options, PlanningOptions};
use conduit_presentation::{Manifestation, Presentation, PresentationInteractionLedger};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fmt::Write as _;

#[derive(Deserialize)]
pub(super) struct InteractionFrame {
    pub(super) presentation_id: String,
    pub(super) presentation_revision: u64,
    pub(super) manifestation_id: String,
    pub(super) input_id: String,
    pub(super) action_id: String,
    pub(super) target: String,
    pub(super) value_kind: String,
    pub(super) sequence: u64,
    pub(super) value: String,
}

const SOURCE: &str = include_str!("../../../../../forms/webchat/main.conduit");
const NODES: usize = 6;
const CORDS: usize = 8;
const PORTS: usize = FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const QUEUE_SLOTS: usize = 32;
const ROUTE_SLOTS: usize = NODES * PORTS;
const ROUTE_TARGETS: usize = CORDS;
const ACTIVE_HOST_OPERATIONS: usize = 9;
const HOST_BINDINGS: usize = NODES * 4;
const PENDING_REQUESTS: usize = 6;
const VALUE_ITEMS: u16 = 64;
const VALUE_BYTES: u32 = 512 * 1024;
const SIGN_ITEMS: u16 = 1_024;
const REQUEST_IDENTITIES: usize = 64;

pub(super) type ChatScheduler = FixedScheduler<
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
    Present,
}

pub(crate) struct BrowserChatSession {
    pub(super) scheduler: ChatScheduler,
    pub(super) lowered_identity: KernelIdentityMap,
    pub(super) identity: KernelExecutionIdentityMap,
    pub(super) current: Option<HostOperationRequest>,
    pub(super) parked_input: Option<HostOperationRequest>,
    pub(super) parked_receive: Option<HostOperationRequest>,
    pub(super) complete: bool,
    pub(super) disconnected: bool,
    pub(super) error: i32,
    pub(super) identity_text: Vec<u8>,
    pub(super) value_capacity: (usize, usize),
    pub(super) identity_capacity: (usize, usize, usize),
    pub(super) chat_state: conduit_chat::ChatPresentationState,
    pub(super) plan: conduit_core::Plan,
    pub(super) active_play: conduit_core::ActivePlayIdentity,
    pub(super) renderer_placement: conduit_core::PlacementId,
    pub(super) presentation: Presentation,
    pub(super) manifestation: Option<Manifestation>,
    pub(super) interaction_ledger: PresentationInteractionLedger,
    pub(super) interaction_text: Vec<u8>,
    pub(super) evidence_text: Vec<u8>,
}

impl BrowserChatSession {
    #[cfg(test)]
    pub(crate) fn prepare(url: &str, host_id: HostId, boot_id: BootId) -> Result<Self, i32> {
        Self::prepare_form(url, "webchat-browser-demo", host_id, boot_id)
    }

    pub(crate) fn prepare_form(
        url: &str,
        form_name: &str,
        host_id: HostId,
        boot_id: BootId,
    ) -> Result<Self, i32> {
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
        let expanded = expand_canonical_form(&checked, form_name, &profile).map_err(|_| -204)?;
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
            &[BaseImplementationId::from("conduit.base/local@1")],
            PlanningOptions {
                connection_bases: &connection_bases,
                line_candidates: &line_candidates,
                connection_item_capacity: 4,
                connection_byte_capacity: 16 * 1024,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
        )
        .map_err(|_| -206)?;
        let plan_record = plan.clone();
        let fragment = plan.fragments.into_iter().next().ok_or(-207)?;
        let lowered = lower_plan_fragment(&fragment).map_err(|_| -208)?;
        if lowered.nodes.len() != NODES
            || lowered.cords.len() != CORDS
            || lowered.cord_value_slots as usize > QUEUE_SLOTS
            || lowered.host_operations.len() != ACTIVE_HOST_OPERATIONS
        {
            return Err(-209);
        }

        let mut values =
            HostedValueStore::new(VALUE_ITEMS, 64 * 1024, VALUE_BYTES).map_err(|_| -210)?;
        let state_placement = fragment
            .placements
            .iter()
            .find(|placement| placement.kind_id.as_str() == conduit_chat::CHAT_STATE_KIND)
            .ok_or(-212)?;
        let text = |key: &str| {
            state_placement
                .configuration
                .iter()
                .find_map(|entry| match (entry.key.as_str(), &entry.value) {
                    (name, conduit_core::ConfigurationValue::Text(value)) if name == key => {
                        Some(value.clone())
                    }
                    _ => None,
                })
                .ok_or(-212)
        };
        let count = |key: &str| {
            state_placement
                .configuration
                .iter()
                .find_map(|entry| match (entry.key.as_str(), &entry.value) {
                    (name, conduit_core::ConfigurationValue::U64(value)) if name == key => {
                        Some(*value)
                    }
                    _ => None,
                })
                .ok_or(-212)
        };
        let chat_state =
            conduit_chat::ChatPresentationState::new(conduit_chat::ChatPresentationConfiguration {
                title: text("title")?,
                history_label: text("history-label")?,
                input_label: text("input-label")?,
                submit_label: text("submit-label")?,
                status_label: text("status-label")?,
                maximum_history_items: count("maximum-history-items")? as usize,
                maximum_message_bytes: count("maximum-message-bytes")? as u32,
            })
            .map_err(|_| -212)?;
        let presentation = chat_state.presentation().map_err(|_| -212)?;
        let initial_presentation = serde_json::to_vec(&presentation).map_err(|_| -212)?;
        let mut operations = Vec::with_capacity(NODES);
        for node in &lowered.nodes {
            let placement = &fragment.placements[usize::from(node.node.0)];
            let operation = match placement.kind_id.as_str() {
                conduit_chat::CHAT_STATE_KIND => BrowserChatOperation::state(
                    values.store(&initial_presentation).map_err(|_| -211)?,
                ),
                conduit_presentation::PRESENTATION_TEE_KIND => BrowserChatOperation::tee(),
                conduit_presentation::RENDERER_KIND => BrowserChatOperation::renderer(),
                conduit_presentation::INTERACTION_KIND => {
                    BrowserChatOperation::interaction(values.store(&[]).map_err(|_| -211)?)
                }
                conduit_chat::CHAT_SUBMIT_KIND => BrowserChatOperation::submit(),
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
                        values.store(&[1]).map_err(|_| -211)?,
                    )
                }
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
        let renderer_placement = fragment
            .placements
            .iter()
            .find(|placement| placement.kind_id.as_str() == conduit_presentation::RENDERER_KIND)
            .map(|placement| placement.placement_id.clone())
            .ok_or(-219)?;
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
            chat_state,
            plan: plan_record,
            active_play,
            renderer_placement,
            presentation,
            manifestation: None,
            interaction_ledger: PresentationInteractionLedger::new(8, 32).map_err(|_| -219)?,
            interaction_text: Vec::with_capacity(16 * 1024),
            evidence_text: Vec::with_capacity(16 * 1024),
        };
        session.drive()?;
        Ok(session)
    }
}
