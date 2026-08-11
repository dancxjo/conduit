//! Exact two-Host plan reconstruction for the browser half of the toggle proof.

use super::ERROR_PREPARE;
use conduit_core::{CapabilityId, ConnectionBase, GearId, Plan};
use conduit_planner::{plan_with_line_offers, PlacementChoice, PlacementChoices};
use conduit_signal::{
    distributed_toggle_browser_sink_advertisement, distributed_toggle_std_source_advertisement,
    distributed_toggle_websocket_line_offer, signal_profile_catalog,
    DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
};
use std::collections::BTreeMap;

pub(super) fn exact_toggle_plan() -> Result<Plan, i32> {
    let source = distributed_toggle_std_source_advertisement();
    let sink = distributed_toggle_browser_sink_advertisement();
    let form = conduit_form::parse(
        include_str!("../../../../examples/remote-toggle.form"),
        &signal_profile_catalog(),
    )
    .map_err(|_| ERROR_PREPARE)?;
    let placements = PlacementChoices {
        by_gear: BTreeMap::from([
            (
                GearId::from("trigger"),
                PlacementChoice {
                    host_id: source.host_id.clone(),
                    capability_id: CapabilityId::from("trigger-1"),
                },
            ),
            (
                GearId::from("toggle"),
                PlacementChoice {
                    host_id: source.host_id.clone(),
                    capability_id: CapabilityId::from("toggle-1"),
                },
            ),
            (
                GearId::from("show"),
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
        &[ConnectionBase::Local, ConnectionBase::WebSocket],
        DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
        conduit_signal::TRIGGER_ENCODED_LEN,
        &[distributed_toggle_websocket_line_offer()],
    )
    .map_err(|_| ERROR_PREPARE)
}
