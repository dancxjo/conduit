//! Borrow reusable checked content, or transfer retiring configuration buffers.
use alloc::{borrow::Cow, vec::Vec};
mod connection_gear;
mod gears;
use conduit_core::{CheckedPlotId, ExpandedPlotId, SourceDocumentId};
use conduit_plot::{
    CheckedConnection, CheckedGear, CheckedPlot, ExpandedCanonicalPlot, PlotCompletionPolicy,
};
#[cfg(test)]
pub(crate) use connection_gear::minimum_at as minimum_value_bytes;
pub(crate) use connection_gear::ConnectionGear;
pub(crate) use gears::PlanningGears;

pub(crate) struct PlanningInput<'a> {
    pub source_document_id: &'a SourceDocumentId,
    pub checked_plot_id: &'a CheckedPlotId,
    pub expanded_plot_id: &'a ExpandedPlotId,
    pub name: &'a str,
    pub completion: PlotCompletionPolicy,
    pub gears: PlanningGears<'a>,
    pub connections: Cow<'a, [CheckedConnection]>,
}

impl PlanningInput<'_> {
    pub fn consuming(plot: &ExpandedCanonicalPlot, gears: Vec<CheckedGear>) -> PlanningInput<'_> {
        let mut input = PlanningInput::from(plot);
        input.gears = PlanningGears::owned(gears);
        input
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
            gears: PlanningGears::borrowed(&plot.gears),
            connections: Cow::Borrowed(&plot.connections),
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
            gears: PlanningGears::borrowed(&plot.gears),
            connections: Cow::Borrowed(&plot.connections),
        }
    }
}
