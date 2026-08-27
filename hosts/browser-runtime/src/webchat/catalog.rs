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
            .chain(crate::presentation_nucleus::human_io_advertisement_offers())
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_browser_advertisement_is_finite_truth_without_acquired_camera_use() {
        let advertisement = advertisement(HostId::from("browser/one"), BootId::from("boot/one"));
        let encoded = serde_json::to_vec(&advertisement).unwrap();
        assert!(
            encoded.len() <= conduit_body::MAX_CANDIDATE_ADVERTISEMENT_BYTES as usize,
            "encoded browser advertisement is {} bytes",
            encoded.len()
        );
        for kind in [
            conduit_std_catalog::CAMERA_ACQUIRE_KIND,
            conduit_std_catalog::MICROPHONE_ACQUIRE_KIND,
            conduit_presentation::INTERACTION_KIND,
            conduit_presentation::RENDERER_KIND,
            conduit_std_catalog::TEXT_PRESENTATION_KIND,
            conduit_std_catalog::GRAPHICS_RECT_KIND,
            conduit_std_catalog::CAMERA_FRAME_SINK_KIND,
        ] {
            assert!(advertisement
                .capabilities
                .iter()
                .any(|offer| offer.kind_id.as_str() == kind));
        }
        assert!(advertisement
            .capabilities
            .iter()
            .all(|offer| offer.kind_id.as_str() != conduit_std_catalog::CAMERA_SOURCE_KIND));
    }
}
