//! Ordinary cross-browser Plan construction for the no-hardware capstone profile.

use std::collections::BTreeMap;

use conduit_core::{ConnectionBase, HostAdvertisement, LinkLimits};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
use conduit_planner::{
    plan_expanded_canonical_with_options, PlacementChoice, PlacementChoices, PlanningOptions,
};

const SOURCE: &str = include_str!("../../../../../examples/webchat.conduit");

pub(super) fn cross_browser_plan(
    source: &HostAdvertisement,
    sink: &HostAdvertisement,
) -> Result<conduit_core::Plan, String> {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_net::install_external_websocket_catalogs(&mut startup, &mut profile)?;
    conduit_chat::install_browser_chat_catalogs(&mut startup, &mut profile)?;
    let checked = check_syntax_document(&parse_syntax_document(SOURCE), &startup)
        .map_err(|error| format!("canonical webchat check: {error:?}"))?;
    let expanded = expand_canonical_form(&checked, "webchat-browser-demo", &profile)
        .map_err(|error| format!("canonical webchat expansion: {error:?}"))?;
    let mut by_gear = BTreeMap::new();
    for gear in &expanded.gears {
        let target = if gear.kind_id.as_str() == conduit_chat::WEB_TEXT_INPUT_KIND {
            source
        } else {
            sink
        };
        let capability = target
            .capabilities
            .iter()
            .find(|offer| offer.kind_id == gear.kind_id)
            .ok_or_else(|| format!("browser offer missing kind {}", gear.kind_id.as_str()))?;
        by_gear.insert(
            gear.gear_id.clone(),
            PlacementChoice {
                host_id: target.host_id.clone(),
                capability_id: capability.capability_id.clone(),
            },
        );
    }
    let cross = expanded
        .connections
        .iter()
        .find(|connection| {
            expanded
                .gears
                .iter()
                .find(|gear| gear.gear_id == connection.source_gear_id)
                .is_some_and(|gear| gear.kind_id.as_str() == conduit_chat::WEB_TEXT_INPUT_KIND)
        })
        .ok_or("expanded webchat has no text-to-socket connection")?;
    let line = conduit_core::process_owned_line_offer_with_limits(
        "browser/capstone/websocket-line",
        "browser/capstone/websocket-binding",
        ConnectionBase::WebSocket,
        "browser/capstone/websocket-instance",
        source,
        sink,
        LinkLimits {
            maximum_in_flight_items: 4,
            maximum_payload_bytes: 1_024,
            maximum_buffered_bytes: 4_096,
            maximum_frame_bytes: 8_192,
        },
    );
    let line_candidates = BTreeMap::from([(
        (cross.source_gear_id.clone(), cross.sink_gear_id.clone()),
        vec![line.line_id.clone()],
    )]);
    plan_expanded_canonical_with_options(
        &expanded,
        &[source.clone(), sink.clone()],
        &PlacementChoices { by_gear },
        &[ConnectionBase::Local, ConnectionBase::WebSocket],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &line_candidates,
            connection_item_capacity: 4,
            connection_byte_capacity: 1_024,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[line],
        },
    )
    .map_err(|error| error.to_string())
}
