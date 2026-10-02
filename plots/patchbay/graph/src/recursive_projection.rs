//! Authoritative collapsed/open projection of one recursively realized Plot gear.

use crate::prelude::*;

use conduit_core::{
    CheckedFront, CheckedPlotId, ExpandedPlotId, GearId, KindId, KindIdentity, SourceDocumentId,
};
use conduit_plot::{CheckedConnection, ExpandedCanonicalPlot};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecursivePlotGearProjection {
    pub invocation_path: String,
    pub kind_id: KindId,
    pub kind_contract_revision: KindIdentity,
    pub source_document_id: SourceDocumentId,
    pub checked_plot_id: CheckedPlotId,
    pub expanded_plot_id: ExpandedPlotId,
    pub front: CheckedFront,
    pub open: bool,
    pub nested_gear_count: u16,
    pub boundary_connections: Vec<CheckedConnection>,
    pub visible_gears: Vec<GearId>,
    pub visible_connections: Vec<CheckedConnection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecursivePlotProjectionError {
    MissingRealizationBack,
    MissingExpandedGears,
    TooManyExpandedGears,
}

/// Projects exact expansion truth already sealed into the expanded Plot.
/// Opening changes visibility only; the front, invocation identity, caller
/// boundary connections, and every realization identity remain unchanged.
pub fn project_recursive_plot_gear(
    plot: &ExpandedCanonicalPlot,
    invocation_path: &str,
    front: CheckedFront,
    open: bool,
) -> Result<RecursivePlotGearProjection, RecursivePlotProjectionError> {
    let back = plot
        .realization_backs
        .iter()
        .find(|back| back.invocation_path == invocation_path)
        .ok_or(RecursivePlotProjectionError::MissingRealizationBack)?;
    let prefix = format!("{invocation_path}/");
    let nested_gears = plot
        .gears
        .iter()
        .filter(|gear| gear.gear_id.as_str().starts_with(&prefix))
        .map(|gear| gear.gear_id.clone())
        .collect::<Vec<_>>();
    if nested_gears.is_empty() {
        return Err(RecursivePlotProjectionError::MissingExpandedGears);
    }
    let nested_gear_count = u16::try_from(nested_gears.len())
        .map_err(|_| RecursivePlotProjectionError::TooManyExpandedGears)?;
    let is_nested = |gear: &GearId| gear.as_str().starts_with(&prefix);
    let boundary_connections = plot
        .connections
        .iter()
        .filter(|connection| {
            is_nested(&connection.source_gear_id) != is_nested(&connection.sink_gear_id)
        })
        .cloned()
        .collect();
    let visible_connections = if open {
        plot.connections
            .iter()
            .filter(|connection| {
                is_nested(&connection.source_gear_id) && is_nested(&connection.sink_gear_id)
            })
            .cloned()
            .collect()
    } else {
        Vec::new()
    };
    let visible_gears = if open { nested_gears } else { Vec::new() };

    Ok(RecursivePlotGearProjection {
        invocation_path: back.invocation_path.clone(),
        kind_id: back.kind_id.clone(),
        kind_contract_revision: back.kind_contract_revision.clone(),
        source_document_id: back.source_document_id.clone(),
        checked_plot_id: back.checked_plot_id.clone(),
        expanded_plot_id: plot.expanded_plot_id.clone(),
        front,
        open,
        nested_gear_count,
        boundary_connections,
        visible_gears,
        visible_connections,
    })
}
