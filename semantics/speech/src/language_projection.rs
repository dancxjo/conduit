//! ASR/provider language evidence consumes an exact Language-owned mapping.
//! This creates a hypothesis, never coverage, commitment or route selection.
use crate::semantic::{ListeningConfidence, ListeningLanguageHypothesis};
use conduit_core::projection::{ProjectionRefusal, ProjectionReport};
use conduit_language::LanguageMappingProjection;
use conduit_plot::rust_binding::NativeBindingRefusal;

#[derive(Debug)]
pub enum ListeningLanguageProjectionRefusal {
    Mapping(ProjectionRefusal),
    Native(NativeBindingRefusal),
}

pub fn project_language_hypothesis(
    report: &ProjectionReport<'_, LanguageMappingProjection<'_>>,
    confidence: Option<ListeningConfidence>,
) -> Result<ListeningLanguageHypothesis, ListeningLanguageProjectionRefusal> {
    let target = report
        .require_exact()
        .map_err(ListeningLanguageProjectionRefusal::Mapping)?;
    ListeningLanguageHypothesis::new(
        confidence,
        target.language().clone(),
        target.variety().map(|v| v.identity().clone()),
    )
    .map_err(ListeningLanguageProjectionRefusal::Native)
}
