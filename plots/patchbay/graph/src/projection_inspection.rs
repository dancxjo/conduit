//! Fidelity at one exact selected boundary, borrowed from authoritative results.
use crate::{PatchbayGraph, PatchbayGraphError, PatchbaySubjectRef};
use conduit_core::projection::{ProjectionDomain, ProjectionReport, ProjectionSummary};

pub struct PatchbayProjectionInspection<'r, 'a, D: ProjectionDomain> {
    pub report: &'r ProjectionReport<'a, D>,
    pub summary: ProjectionSummary,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchbayProjectionRefusal {
    Graph(PatchbayGraphError),
    MissingBoundary,
    DifferentBoundary,
}
impl PatchbayGraph {
    pub fn inspect_projection<'r, 'a, D: ProjectionDomain>(
        &self,
        subject: &PatchbaySubjectRef,
        report: &'r ProjectionReport<'a, D>,
    ) -> Result<PatchbayProjectionInspection<'r, 'a, D>, PatchbayProjectionRefusal> {
        self.resolve_subject_ref(subject)
            .map_err(PatchbayProjectionRefusal::Graph)?;
        let boundary = report
            .basis()
            .boundary
            .ok_or(PatchbayProjectionRefusal::MissingBoundary)?;
        if boundary.expanded_form != &subject.expanded_plot_id
            || boundary.subject.as_str() != subject.subject_identity
        {
            return Err(PatchbayProjectionRefusal::DifferentBoundary);
        }
        Ok(PatchbayProjectionInspection {
            report,
            summary: report.summary(),
        })
    }
}
