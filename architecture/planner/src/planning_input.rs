//! Borrow validated plot content during Plan assembly without copying programs.
use conduit_core::{CheckedPlotId, ExpandedPlotId, PlotIdentity, SourceDocumentId};
use conduit_plot::{
    CheckedConnection, CheckedGear, CheckedPlot, ExpandedCanonicalPlot, PlotCompletionPolicy,
};

pub(crate) struct PlanningInput<'a> {
    pub source_document_id: &'a SourceDocumentId,
    pub checked_plot_id: &'a CheckedPlotId,
    pub expanded_plot_id: &'a ExpandedPlotId,
    pub name: &'a str,
    pub completion: PlotCompletionPolicy,
    pub gears: &'a [CheckedGear],
    pub connections: &'a [CheckedConnection],
}

impl PlanningInput<'_> {
    pub fn identity(&self) -> PlotIdentity {
        PlotIdentity {
            source_document_id: self.source_document_id.clone(),
            checked_plot_id: self.checked_plot_id.clone(),
            expanded_plot_id: self.expanded_plot_id.clone(),
        }
    }
}

impl<'a> From<&'a CheckedPlot> for PlanningInput<'a> {
    fn from(plot: &'a CheckedPlot) -> Self {
        Self {
            source_document_id: &plot.source_document_id,
            checked_plot_id: &plot.checked_plot_id,
            expanded_plot_id: &plot.expanded_plot_id,
            name: &plot.name,
            completion: plot.completion,
            gears: &plot.gears,
            connections: &plot.connections,
        }
    }
}

impl<'a> From<&'a ExpandedCanonicalPlot> for PlanningInput<'a> {
    fn from(plot: &'a ExpandedCanonicalPlot) -> Self {
        Self {
            source_document_id: &plot.source_document_id,
            checked_plot_id: &plot.checked_plot_id,
            expanded_plot_id: &plot.expanded_plot_id,
            name: &plot.name,
            completion: plot.completion,
            gears: &plot.gears,
            connections: &plot.connections,
        }
    }
}
