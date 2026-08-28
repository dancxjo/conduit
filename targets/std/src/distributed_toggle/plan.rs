//! Planning for the distributed toggle proof.
//!
//! Resolves the canonical `remote-toggle.conduit` against the std source and browser sink
//! advertisements and returns the exact two-fragment plan.

use conduit_core::{BaseImplementationId, CapabilityId, GearId, HostAdvertisement, Plan};
use conduit_planner::{plan_with_line_offers, PlacementChoice, PlacementChoices};
use conduit_signal::signal_profile_catalog;
use conduit_signal_conformance::{
    distributed_toggle_browser_sink_advertisement, distributed_toggle_std_source_advertisement,
    distributed_toggle_websocket_line_offer, DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DistributedTogglePlan {
    pub source_advertisement: HostAdvertisement,
    pub sink_advertisement: HostAdvertisement,
    pub plan: Plan,
}

pub fn exact_distributed_toggle_plan() -> Result<DistributedTogglePlan, String> {
    let source_advertisement = distributed_toggle_std_source_advertisement();
    let sink_advertisement = distributed_toggle_browser_sink_advertisement();
    let form = conduit_form::parse_with_startup(
        include_str!("../../../../proof/fixtures/forms/remote-toggle.conduit"),
        &conduit_signal::signal_startup_catalog(),
        &signal_profile_catalog(),
    )
    .map_err(|error| error.to_string())?;
    let placements = PlacementChoices {
        by_gear: BTreeMap::from([
            (
                GearId::from("remote-toggle/trigger"),
                PlacementChoice {
                    host_id: source_advertisement.host_id.clone(),
                    capability_id: CapabilityId::from("trigger-1"),
                },
            ),
            (
                GearId::from("remote-toggle/toggle"),
                PlacementChoice {
                    host_id: source_advertisement.host_id.clone(),
                    capability_id: CapabilityId::from("toggle-1"),
                },
            ),
            (
                GearId::from("remote-toggle/show"),
                PlacementChoice {
                    host_id: sink_advertisement.host_id.clone(),
                    capability_id: CapabilityId::from("toggle-dom-show-1"),
                },
            ),
        ]),
    };
    let link = distributed_toggle_websocket_line_offer();
    let plan = plan_with_line_offers(
        &form,
        &[source_advertisement.clone(), sink_advertisement.clone()],
        &placements,
        &[
            BaseImplementationId::from("conduit.base/local@1"),
            BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
        ],
        DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
        conduit_signal::TRIGGER_ENCODED_LEN,
        &[link],
    )
    .map_err(|error| error.to_string())?;
    Ok(DistributedTogglePlan {
        source_advertisement,
        sink_advertisement,
        plan,
    })
}
