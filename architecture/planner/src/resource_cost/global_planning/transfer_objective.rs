//! Match incremental edge costs to a complete, independently admitted Plan.
use super::*;
use conduit_plot::CheckedConnection;

#[derive(Clone, Copy)]
pub struct CostCordTransferDemand<'a> {
    pub connection: &'a CheckedConnection,
    pub extent: CostTransferExtent,
}
#[derive(Clone, Copy)]
pub struct GlobalTransferInputs<'a> {
    pub demands: &'a [CostCordTransferDemand<'a>],
    pub forecasts: &'a [CostTransferForecast<'a>],
}
pub(super) fn validate_inputs(
    plot: &CheckedPlot,
    inputs: GlobalTransferInputs<'_>,
) -> Result<(), CostEvidenceRefusal> {
    if inputs.demands.len() != plot.connections.len()
        || inputs.forecasts.len() > crate::MAXIMUM_REALIZATION_DECISION_RECORDS
    {
        return Err(CostEvidenceRefusal::InputLimit);
    }
    for cord in &plot.connections {
        if inputs
            .demands
            .iter()
            .filter(|d| d.connection == cord)
            .count()
            != 1
        {
            return Err(CostEvidenceRefusal::AmbiguousEvidence);
        }
    }
    Ok(())
}

/// Each returned coordinate is either one revalidated duration or unknown.
/// Rejected/absent estimates cannot become zero, even for a local Cord.
pub(super) fn edge_costs(
    plot: &CheckedPlot,
    plan: &Plan,
    transfer: GlobalTransferInputs<'_>,
    inputs: CostPlanningInputs<'_, '_>,
) -> Result<Vec<Option<ExactRational>>, ()> {
    let placements: Vec<_> = plan.fragments.iter().flat_map(|f| &f.placements).collect();
    let mut result = Vec::with_capacity(plot.connections.len());
    for cord in &plot.connections {
        let source = placements
            .iter()
            .find(|p| p.gear_id == cord.source_gear_id)
            .ok_or(())?;
        let sink = placements
            .iter()
            .find(|p| p.gear_id == cord.sink_gear_id)
            .ok_or(())?;
        let mut connections = plan
            .fragments
            .iter()
            .flat_map(|f| &f.connections)
            .filter(|c| {
                c.source_placement_id == source.placement_id
                    && c.sink_placement_id == sink.placement_id
                    && c.source_port_id == cord.source_port_id
                    && c.sink_port_id == cord.sink_port_id
                    && c.value_kind == cord.value_kind
                    && c.track == cord.track
                    && c.temporal == cord.temporal
            });
        let connection = connections.next().ok_or(())?;
        // Identical copies in endpoint fragments are the same immutable Cord.
        if connections.any(|other| other != connection) {
            return Err(());
        }
        let extent = transfer
            .demands
            .iter()
            .find(|d| d.connection == cord)
            .ok_or(())?
            .extent;
        if extent.items == 0
            || extent.items > connection.item_capacity
            || extent.encoded_bytes > connection.byte_capacity
        {
            return Err(());
        }
        let mut forecasts = transfer
            .forecasts
            .iter()
            .filter(|f| f.connection() == connection && f.extent() == extent);
        let estimate = match (forecasts.next(), forecasts.next()) {
            (Some(forecast), None) => {
                let context = inputs
                    .contexts
                    .iter()
                    .find(|c| c.host.host_id == source.host_id);
                let sink_host = inputs.hosts.iter().find(|h| h.host_id == sink.host_id);
                context
                    .zip(sink_host)
                    .and_then(|(context, sink_host)| {
                        forecast
                            .validate_at(connection, extent, context, sink_host)
                            .ok()
                    })
                    .map(|range| range.upper().seconds())
            }
            _ => None,
        };
        result.push(estimate);
    }
    Ok(result)
}
