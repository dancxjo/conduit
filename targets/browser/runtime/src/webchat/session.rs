use super::BrowserChatBack;
use conduit_core::{bind_active_play, BaseImplementationId, BootId, HostId};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_kernel::scheduler::{
    CordSpec, FixedScheduler, HostCallRequest, NodeSpec, RemoteValueOffer,
};
use conduit_kernel::{
    FixedHostCallBindings, FixedRoutes, HostedSignLog, HostedValueStore, ValueStorage,
};
use conduit_plan_lowering::lowering::{
    lower_plan_fragment, KernelExecutionIdentityMap, KernelIdentityMap, LoweredForePort,
    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
};
use conduit_planner::{plan_expanded_authoring_with_options, PlanningOptions};
use conduit_presentation::{
    FaceInteractionArgument, FaceInteractionLedger, Presentation, PresentationBasis,
};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fmt::Write as _;

#[derive(Deserialize)]
pub(super) struct InteractionFrame {
    pub(super) presentation_id: String,
    pub(super) presentation_revision: u64,
    pub(super) show_id: String,
    pub(super) action_id: String,
    pub(super) target: String,
    pub(super) arguments: Vec<FaceInteractionArgument>,
    pub(super) sequence: u64,
}

const SOURCE: &str = include_str!("../../../../../forms/webchat/main.conduit");
const NODES: usize = 6;
const CORDS: usize = 8;
const PORTS: usize = FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const QUEUE_SLOTS: usize = 44;
const ROUTE_SLOTS: usize = NODES * PORTS;
const ROUTE_TARGETS: usize = CORDS;
const ACTIVE_HOST_CALLS: usize = 10;
const HOST_BINDINGS: usize = NODES * 4;
const PENDING_REQUESTS: usize = 8;
const VALUE_ITEMS: u16 = 64;
const VALUE_BYTES: u32 = 512 * 1024;
const SIGN_ITEMS: u16 = 1_024;
const REMOTE_SIGN_ITEMS: u16 = 32;
const REQUEST_IDENTITIES: usize = 64;

pub(super) type ChatScheduler = FixedScheduler<
    BrowserChatBack,
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
    pub(super) current: Option<HostCallRequest>,
    pub(super) interaction_boundary: LoweredForePort,
    pub(super) face_boundary: LoweredForePort,
    pub(super) pending_face: Option<RemoteValueOffer>,
    pub(super) parked_receive: Option<HostCallRequest>,
    pub(super) complete: bool,
    pub(super) disconnected: bool,
    pub(super) error: i32,
    pub(super) identity_text: Vec<u8>,
    pub(super) value_capacity: (usize, usize),
    pub(super) identity_capacity: (usize, usize, usize),
    pub(super) chat_state: conduit_chat::ChatPresentationState,
    pub(super) active_play: conduit_core::ActivePlayIdentity,
    pub(super) presentation: Presentation,
    pub(super) body_id: conduit_body::BodyId,
    pub(super) wake: conduit_body::Wake,
    pub(super) body_plan: conduit_body::BodyPlan,
    pub(super) mask: Option<crate::workspace_mask::BrowserMaskRuntime>,
    pub(super) interaction_ledger: FaceInteractionLedger,
    pub(super) interaction_text: Vec<u8>,
    pub(super) evidence_text: Vec<u8>,
}

impl BrowserChatSession {
    #[cfg(test)]
    pub(crate) fn prepare(url: &str, host_id: HostId, boot_id: BootId) -> Result<Self, i32> {
        Self::prepare_form(url, "chat/browser-client", host_id, boot_id)
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
        let authoring =
            expand_canonical_form_for_authoring(&checked, form_name, &profile).map_err(|_| -204)?;
        let advertisement = super::catalog::advertisement(host_id, boot_id);
        let hosts = [advertisement.clone()];
        let placements = conduit_planner::default_expanded_placements(&authoring.expanded, &hosts)
            .map_err(|_| -205)?;
        let connection_bases = BTreeMap::new();
        let line_candidates = BTreeMap::new();
        let boundary_limits = authoring
            .front
            .inputs()
            .iter()
            .map(|port| (conduit_core::PortDirection::Input, port))
            .chain(
                authoring
                    .front
                    .outputs()
                    .iter()
                    .map(|port| (conduit_core::PortDirection::Output, port)),
            )
            .map(|(direction, port)| {
                (
                    conduit_planner::ForeBoundaryKey {
                        direction,
                        front_port_id: port.port_id.clone(),
                        track: conduit_core::ConnectionTrack::Payload,
                    },
                    conduit_planner::ConnectionQueueLimits {
                        item_capacity: conduit_presentation::MAX_QUEUED_FACE_INTERACTIONS as u16,
                        byte_capacity: if direction == conduit_core::PortDirection::Input {
                            conduit_presentation::MAX_FACE_INTERACTION_BYTES as u32
                                * conduit_presentation::MAX_QUEUED_FACE_INTERACTIONS as u32
                        } else {
                            conduit_presentation::MAX_PRESENTATION_TOTAL_BYTES as u32 * 2
                        },
                    },
                )
            })
            .collect();
        let plan = plan_expanded_authoring_with_options(
            &authoring,
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
            &boundary_limits,
        )
        .map_err(|_| -206)?;
        let plan_record = plan.clone();
        let fragment = plan.fragments.into_iter().next().ok_or(-207)?;
        let active_play =
            bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
        let lowered = lower_plan_fragment(&fragment).map_err(|_| -208)?;
        let interaction_boundary = lowered
            .fore_ports
            .iter()
            .find(|port| {
                port.direction == conduit_core::PortDirection::Input
                    && port.track == conduit_core::ConnectionTrack::Payload
                    && port.front_port_id.as_str() == "interaction"
            })
            .cloned()
            .ok_or(-208)?;
        let face_boundary = lowered
            .fore_ports
            .iter()
            .find(|port| {
                port.direction == conduit_core::PortDirection::Output
                    && port.track == conduit_core::ConnectionTrack::Payload
                    && port.front_port_id.as_str() == "face"
            })
            .cloned()
            .ok_or(-208)?;
        if lowered.nodes.len() != NODES
            || lowered.cords.len() != CORDS
            || lowered.cord_value_slots as usize > QUEUE_SLOTS
            || lowered.host_calls.len() != ACTIVE_HOST_CALLS
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
        let resident = conduit_body::ResidentForm::new(
            fragment.source_document_id.clone(),
            fragment.checked_form_id.clone(),
        );
        let born = conduit_body::Body::born(
            resident.source_document_id.clone(),
            resident.checked_form_id.clone(),
            1,
            conduit_core::SignId::from("sign/webchat-body-born"),
        )
        .map_err(|_| -212)?;
        let body_id = born.body_id.clone();
        let (_, wake) = born
            .wake(1, conduit_core::SignId::from("sign/webchat-body-woke"))
            .map_err(|_| -212)?;
        let body_plan = conduit_body::BodyPlan::seal(
            &wake,
            vec![conduit_body::BodyFormPlan {
                form: resident,
                plan: plan_record.clone(),
            }],
        )
        .map_err(|_| -212)?;
        let presentation = chat_state
            .presentation()
            .map_err(|_| -212)?
            .with_basis(PresentationBasis {
                body_id: Some(body_id.clone()),
                wake_id: Some(wake.wake_id.clone()),
                source_document_id: Some(fragment.source_document_id.clone()),
                checked_form_id: Some(fragment.checked_form_id.clone()),
                expanded_form_id: Some(fragment.expanded_form_id.clone()),
                plan_id: Some(fragment.plan_id.clone()),
                active_play_id: Some(active_play.active_play_id.clone()),
                sign_ids: vec![conduit_core::SignId::from("sign/webchat-face")],
            })
            .map_err(|_| -212)?;
        let mut backs = Vec::with_capacity(NODES);
        for node in &lowered.nodes {
            let placement = &fragment.placements[usize::from(node.node.0)];
            let back = match placement.kind_id.as_str() {
                conduit_chat::CHAT_STATE_KIND => BrowserChatBack::state(),
                conduit_chat::CHAT_SUBMIT_KIND => BrowserChatBack::submit(),
                conduit_chat::CHAT_FROM_WEBSOCKET_KIND
                | conduit_chat::CHAT_TO_WEBSOCKET_KIND
                | conduit_chat::CHAT_CONNECTION_FROM_WEBSOCKET_KIND
                | conduit_chat::CHAT_CURRENT_CONNECTION_KIND => BrowserChatBack::adapter(),
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
                    BrowserChatBack::socket(
                        values.store(url).map_err(|_| -211)?,
                        values.store(&[0]).map_err(|_| -211)?,
                        values.store(&[0]).map_err(|_| -211)?,
                        values.store(&[1]).map_err(|_| -211)?,
                    )
                }
                _ => return Err(-213),
            };
            backs.push(back);
        }
        let backs: [BrowserChatBack; NODES] = backs.try_into().map_err(|_| -214)?;

        let inactive_node = NodeSpec {
            input_cords: [None; PORTS],
            maximum_step_fuel: 1,
        };
        let mut node_specs = [inactive_node; NODES];
        node_specs.copy_from_slice(&lowered.node_specs);
        let inactive_cord = CordSpec::inactive();
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
        let mut bindings = FixedHostCallBindings::<HOST_BINDINGS>::new(4);
        for operation in &lowered.host_calls {
            bindings
                .install(operation.node, operation.binding)
                .map_err(|_| -216)?;
        }
        bindings.seal().map_err(|_| -216)?;
        let sign_bytes = u32::from(SIGN_ITEMS)
            .checked_mul(core::mem::size_of::<conduit_kernel::KernelEvent>() as u32)
            .ok_or(-217)?;
        let remote_sign_bytes =
            conduit_kernel::remote_sign_storage_bytes(REMOTE_SIGN_ITEMS).ok_or(-217)?;
        let sign = HostedSignLog::new_with_remote_storage(
            SIGN_ITEMS,
            sign_bytes,
            REMOTE_SIGN_ITEMS,
            remote_sign_bytes,
        )
        .map_err(|_| -217)?;
        let scheduler = ChatScheduler::new_with_active_counts_and_host_calls(
            NODES, CORDS, node_specs, cord_specs, routes, bindings, backs, values, sign,
        )
        .map_err(|_| -218)?;
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
            for requirement in &placement.host_calls {
                write!(
                    identity_text,
                    ":host-call={}",
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
            interaction_boundary,
            face_boundary,
            pending_face: None,
            parked_receive: None,
            complete: false,
            disconnected: false,
            error: 0,
            identity_text,
            value_capacity,
            identity_capacity,
            chat_state,
            active_play,
            presentation,
            body_id,
            wake,
            body_plan,
            mask: None,
            interaction_ledger: FaceInteractionLedger::new(8, 32).map_err(|_| -219)?,
            interaction_text: Vec::with_capacity(16 * 1024),
            evidence_text: Vec::with_capacity(16 * 1024),
        };
        session.drive()?;
        Ok(session)
    }
}
