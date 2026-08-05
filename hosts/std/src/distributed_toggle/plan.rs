//! Planning for the S4 toggle-demo distributed proof.
//!
//! Resolves the `remote-toggle.form` against the std source and browser sink
//! advertisements and returns the exact two-fragment plan.

use conduit_core::{CapabilityId, ConnectionProvider, HostAdvertisement, OperationId, Plan};
use conduit_planner::{plan_with_link_bindings, PlacementChoice, PlacementChoices};
use conduit_signal::{
    distributed_toggle_browser_sink_advertisement, distributed_toggle_std_source_advertisement,
    distributed_toggle_websocket_link_binding, signal_profile_catalog,
    DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS, SIGNAL_ENCODED_LEN,
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
    let form = conduit_form::parse(
        include_str!("../../../../examples/remote-toggle.form"),
        &signal_profile_catalog(),
    )
    .map_err(|error| error.to_string())?;
    let placements = PlacementChoices {
        by_operation: BTreeMap::from([
            (
                OperationId::from("activate"),
                PlacementChoice {
                    host_id: source_advertisement.host_id.clone(),
                    capability_id: CapabilityId::from("activate-1"),
                },
            ),
            (
                OperationId::from("toggle"),
                PlacementChoice {
                    host_id: source_advertisement.host_id.clone(),
                    capability_id: CapabilityId::from("toggle-1"),
                },
            ),
            (
                OperationId::from("show"),
                PlacementChoice {
                    host_id: sink_advertisement.host_id.clone(),
                    capability_id: CapabilityId::from("toggle-dom-show-1"),
                },
            ),
        ]),
    };
    let link = distributed_toggle_websocket_link_binding();
    let plan = plan_with_link_bindings(
        &form,
        &[source_advertisement.clone(), sink_advertisement.clone()],
        &placements,
        &[ConnectionProvider::Local, ConnectionProvider::WebSocket],
        DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
        SIGNAL_ENCODED_LEN,
        &[link],
    )
    .map_err(|error| error.to_string())?;
    Ok(DistributedTogglePlan {
        source_advertisement,
        sink_advertisement,
        plan,
    })
}
