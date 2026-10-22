//! Project Source origins only from compiler-retained connection/binding facts.
use crate::PatchbayCordSource;
use conduit_plot::{AuthoringFrontBinding, ExpandedAuthoringPlot, ExpandedConnectionProvenance};

impl From<&ExpandedConnectionProvenance> for PatchbayCordSource {
    fn from(row: &ExpandedConnectionProvenance) -> Self {
        Self {
            source_document_id: row.source_document_id.clone(),
            checked_plot_id: row.checked_plot_id.clone(),
            source_plot: row.source_plot.clone(),
            plot_path: row.plot_path.clone(),
            source_span: row.source_span,
            origin: row.origin,
        }
    }
}

pub(crate) fn boundary_source(
    plot: &ExpandedAuthoringPlot,
    binding: &AuthoringFrontBinding,
) -> Option<PatchbayCordSource> {
    let origin = binding.source_origin.as_ref()?;
    // Root Fore bindings are materialized by the root's checked Cord. Child
    // exports receive that caller Cord's origin when the binding is constructed.
    Some(PatchbayCordSource {
        source_document_id: plot.expanded.source_document_id.clone(),
        checked_plot_id: plot.expanded.checked_plot_id.clone(),
        source_plot: plot.expanded.name.clone(),
        plot_path: vec![plot.expanded.name.clone()],
        source_span: origin.source_span,
        origin: origin.origin,
    })
}
