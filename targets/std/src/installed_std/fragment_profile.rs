//! Finite installed-fragment admission before operation preparation.
use super::{
    supports_with_adapters, HOST_BINDING_SLOTS, MAX_CORDS, MAX_NODES, MAX_QUEUE_SLOTS, ROUTE_SLOTS,
    ROUTE_TARGETS,
};
use conduit_core::PlanFragment;
use conduit_plan_lowering::lowering::LoweredPlanFragment;

pub(super) fn validate(
    fragment: &PlanFragment,
    lowered: &LoweredPlanFragment,
    source: Option<&crate::selected_native_source::InitializedNativeSource>,
    frame: Option<&crate::HostedFrameBack>,
) -> Result<(), String> {
    let active_nodes = lowered.nodes.len();
    let active_cords = lowered.cords.len();
    if !supports_with_adapters(fragment, source, frame)
        || active_nodes == 0
        || active_nodes > MAX_NODES
        || active_cords == 0
        || active_cords > MAX_CORDS
        || lowered.cord_value_slots as usize > MAX_QUEUE_SLOTS
        || lowered.routes.len() > ROUTE_SLOTS
        || lowered
            .routes
            .iter()
            .map(|route| route.targets.len())
            .sum::<usize>()
            > ROUTE_TARGETS
        || !lowered.remote_endpoints.is_empty()
        || lowered.host_calls.len() > HOST_BINDING_SLOTS
    {
        return Err("fragment exceeds the installed std kernel profile".to_string());
    }
    Ok(())
}
