//! Exact finite browser envelope and unsupported physical profiles.
use super::*;

pub(in crate::plot_runner) fn validate_envelope(
    fragment: &PlanFragment,
    lowered: &LoweredPlanFragment,
    allow_one_remote_endpoint: bool,
) -> Result<(), String> {
    if !lowered.fusions.is_empty() {
        return Err("physical fusion is unsupported by the installed browser/WASM profile".into());
    }
    for placement in &fragment.placements {
        if conduit_semantic_catalog::face_data_label_required(placement.kind_id.as_str()) {
            conduit_semantic_catalog::face_data_label(&placement.configuration)?;
        }
    }
    let route_targets = lowered
        .routes
        .iter()
        .map(|route| route.targets.len())
        .sum::<usize>();
    if lowered.nodes.is_empty()
        || lowered.nodes.len() > MAXIMUM_BROWSER_PLOT_GEARS
        || lowered.cords.is_empty()
        || lowered.cords.len() > MAXIMUM_BROWSER_PLOT_CORDS
        || lowered.cord_value_slots as usize > BROWSER_QUEUE_SLOTS
        || lowered.routes.len() > BROWSER_ROUTE_SLOTS
        || route_targets > BROWSER_ROUTE_TARGETS
        || if allow_one_remote_endpoint {
            lowered.remote_endpoints.len() != 1
        } else {
            !lowered.remote_endpoints.is_empty()
        }
        || lowered.host_calls.len() > BROWSER_HOST_CALL_BINDINGS
        || fragment
            .placements
            .iter()
            .any(|placement| factory(&placement.implementation_id).is_none())
    {
        return Err("Plot exceeds the installed finite browser execution envelope".into());
    }
    Ok(())
}
