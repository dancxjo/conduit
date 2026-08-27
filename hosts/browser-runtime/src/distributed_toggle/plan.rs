//! Exact two-Host plan reconstruction for the browser half of the toggle proof.

use super::ERROR_PREPARE;
use conduit_core::{BaseImplementationId, CapabilityId, GearId, Plan};
use conduit_planner::{plan_with_line_offers, PlacementChoice, PlacementChoices};
use conduit_signal::signal_profile_catalog;
use conduit_signal_conformance::{
    distributed_toggle_browser_sink_advertisement, distributed_toggle_std_source_advertisement,
    distributed_toggle_websocket_line_offer, DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
};
use std::collections::BTreeMap;

pub(super) fn exact_toggle_plan() -> Result<Plan, i32> {
    let source = distributed_toggle_std_source_advertisement();
    let sink = distributed_toggle_browser_sink_advertisement();
    let form = conduit_form::parse_with_startup(
        include_str!("../../../../fixtures/forms/remote-toggle.conduit"),
        &conduit_signal::signal_startup_catalog(),
        &signal_profile_catalog(),
    )
    .map_err(|_| ERROR_PREPARE)?;
    let placements = PlacementChoices {
        by_gear: BTreeMap::from([
            (
                GearId::from("remote-toggle/trigger"),
                PlacementChoice {
                    host_id: source.host_id.clone(),
                    capability_id: CapabilityId::from("trigger-1"),
                },
            ),
            (
                GearId::from("remote-toggle/toggle"),
                PlacementChoice {
                    host_id: source.host_id.clone(),
                    capability_id: CapabilityId::from("toggle-1"),
                },
            ),
            (
                GearId::from("remote-toggle/show"),
                PlacementChoice {
                    host_id: sink.host_id.clone(),
                    capability_id: CapabilityId::from("toggle-dom-show-1"),
                },
            ),
        ]),
    };
    plan_with_line_offers(
        &form,
        &[source, sink],
        &placements,
        &[
            BaseImplementationId::from("conduit.base/local@1"),
            BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
        ],
        DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
        conduit_signal::TRIGGER_ENCODED_LEN,
        &[distributed_toggle_websocket_line_offer()],
    )
    .map_err(|_| ERROR_PREPARE)
}
