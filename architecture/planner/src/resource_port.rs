use crate::PlannerError;
use conduit_core::{PlannedGear, PlannedResourceConnection};
use conduit_form::{CheckedConnection, CheckedGear};

pub(crate) fn plan_resource_connection(
    connection: &CheckedConnection,
    source_gear: &CheckedGear,
    sink_gear: &CheckedGear,
    source_plan: &PlannedGear,
    sink_plan: &PlannedGear,
) -> Result<Option<PlannedResourceConnection>, PlannerError> {
    let source = source_gear
        .resource_ports
        .iter()
        .find(|port| port.port_id == connection.source_port_id);
    let sink = sink_gear
        .resource_ports
        .iter()
        .find(|port| port.port_id == connection.sink_port_id);
    match (source, sink) {
        (None, None) => Ok(None),
        (Some(source), Some(sink))
            if source.class_id == sink.class_id
                && source.ownership == sink.ownership
                && source.lifecycle == sink.lifecycle
                && source.mobility == sink.mobility =>
        {
            if source_plan.host_id != sink_plan.host_id || source_plan.boot_id != sink_plan.boot_id {
                return Err(PlannerError::UnsupportedResourcePortTransfer(format!(
                    "resource '{}' from '{}:{}' cannot cross to '{}:{}' before an exact issuer transfer mechanism is admitted",
                    source.class_id.as_str(),
                    source_plan.host_id.as_str(),
                    source_plan.boot_id.as_str(),
                    sink_plan.host_id.as_str(),
                    sink_plan.boot_id.as_str()
                )));
            }
            let mut bindings = source_plan
                .resources
                .iter()
                .filter(|binding| binding.class_id == source.class_id);
            let Some(binding) = bindings.next() else {
                return Err(PlannerError::InvalidResourcePort(format!(
                    "source gear '{}' has no admitted '{}' binding",
                    source_plan.gear_id.as_str(),
                    source.class_id.as_str()
                )));
            };
            if bindings.next().is_some() {
                return Err(PlannerError::InvalidResourcePort(format!(
                    "source gear '{}' has ambiguous '{}' bindings",
                    source_plan.gear_id.as_str(),
                    source.class_id.as_str()
                )));
            }
            Ok(Some(PlannedResourceConnection {
                contract: source.clone(),
                owner_placement_id: source_plan.placement_id.clone(),
                source_binding: binding.clone(),
            }))
        }
        _ => Err(PlannerError::InvalidResourcePort(format!(
            "connection '{}:{}' to '{}:{}' does not have the same exact resource contract at both endpoints",
            source_plan.gear_id.as_str(),
            connection.source_port_id.as_str(),
            sink_plan.gear_id.as_str(),
            connection.sink_port_id.as_str()
        ))),
    }
}
