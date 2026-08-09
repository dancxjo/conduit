use conduit_core::{
    BootId, HostAdvertisement, HostId, HostProfileId, OfferGeneration, PlannerCapabilityOffer,
    PlannerLimits, PlannerProfileId, PROTOCOL_VERSION,
};
use conduit_planner::BROWSER_PLANNER_PROFILE;

pub(super) fn advertisement() -> HostAdvertisement {
    let socket = conduit_net::browser_external_websocket_family();
    let chat = conduit_chat::browser_chat_family();
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("browser-webchat"),
        boot_id: BootId::from("browser-webchat-boot"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("browser-wasm-webchat"),
        resources: vec![socket.resource, chat.resource],
        planner_capabilities: vec![PlannerCapabilityOffer {
            profile_id: PlannerProfileId::from(BROWSER_PLANNER_PROFILE),
            limits: PlannerLimits {
                maximum_host_advertisements: 16,
                maximum_gears: 64,
                maximum_connections: 128,
                maximum_authority_grants: 64,
                maximum_protected_resource_grants: 64,
                maximum_link_bindings: 128,
            },
        }],
        capabilities: vec![
            socket.capability,
            chat.capabilities[0].clone(),
            chat.capabilities[1].clone(),
        ],
    }
}
