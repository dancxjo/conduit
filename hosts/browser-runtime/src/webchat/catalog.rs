use conduit_core::{
    BootId, HostAdvertisement, HostId, HostProfileId, OfferGeneration, PlannerCapabilityOffer,
    PlannerLimits, PlannerProfileId, PROTOCOL_VERSION,
};
use conduit_planner::BROWSER_PLANNER_PROFILE;

pub(crate) fn advertisement(host_id: HostId, boot_id: BootId) -> HostAdvertisement {
    let mut socket = conduit_net::browser_external_websocket_family();
    socket.capability.limits.max_queue_items = 4;
    socket.capability.limits.max_queue_bytes = 16 * 1024;
    let chat = conduit_chat::browser_chat_family();
    let mut resources: Vec<_> = core::iter::once(socket.resource)
        .chain(chat.resources)
        .collect();
    resources.sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id,
        boot_id,
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("browser-wasm-webchat"),
        resources,
        planner_capabilities: vec![PlannerCapabilityOffer {
            profile_id: PlannerProfileId::from(BROWSER_PLANNER_PROFILE),
            limits: PlannerLimits {
                maximum_host_advertisements: 16,
                maximum_gears: 64,
                maximum_connections: 128,
                maximum_authority_grants: 64,
                maximum_protected_resource_grants: 64,
                maximum_line_offers: 128,
            },
        }],
        capabilities: core::iter::once(socket.capability)
            .chain(chat.capabilities)
            .collect(),
    }
}
