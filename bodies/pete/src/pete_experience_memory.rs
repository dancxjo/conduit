//! Explicit selected autobiography adapter; no ambient read or new memory store.

use conduit_core::{TemporalInstant, TemporalScale, UNIX_UTC_CLOCK_BASIS};
use conduit_human::{
    recollected_experience, ExperienceCertainty, ExperienceItem, ExperienceSourceRef,
    SelectedRecollection,
};

use crate::{ExperienceKind, PeteExperienceRefusal, RetainedExperience};

/// The caller supplies a selected record obtained under its existing read
/// authority. Keep the complete candidate (including original kind, Body,
/// Host, Boot, Plan and Play provenance) in remembered content. A remembered
/// model response never becomes a present observation or a successful effect.
pub fn pete_retained_experience_input(
    record: &RetainedExperience,
) -> Result<ExperienceItem, PeteExperienceRefusal> {
    let candidate = &record.candidate;
    crate::pete_memory::validate_experience_candidate(candidate)
        .map_err(|_| PeteExperienceRefusal::InvalidFact)?;
    let source = candidate
        .provenance
        .first()
        .ok_or(PeteExperienceRefusal::InvalidFact)?;
    if source.event_at_millis > source.recorded_at_millis {
        return Err(PeteExperienceRefusal::InvalidFact);
    }
    let content_kind = match candidate.kind {
        ExperienceKind::ObservedSign => "pete/remembered-observation@1",
        ExperienceKind::HumanStatement => "pete/remembered-human-statement@1",
        ExperienceKind::ModelDerived => "pete/remembered-model-result@1",
        ExperienceKind::ActionRequest => "pete/remembered-action-request@1",
        ExperienceKind::EffectResult => "pete/remembered-effect-result@1",
        ExperienceKind::BodyBiography => "pete/remembered-body-biography@1",
        ExperienceKind::OperatorCorrection => "pete/remembered-correction@1",
    };
    let canonical_content =
        serde_json::to_vec(candidate).map_err(|_| PeteExperienceRefusal::Encoding)?;
    let mut original_sources = Vec::with_capacity(candidate.provenance.len());
    for source in &candidate.provenance {
        let reference = ExperienceSourceRef::Source {
            source_id: source.source_identity.clone(),
        };
        if !original_sources.contains(&reference) {
            original_sources.push(reference);
        }
    }
    let instant = |ticks| TemporalInstant {
        ticks,
        scale: TemporalScale::Milliseconds,
        clock_basis: UNIX_UTC_CLOCK_BASIS.into(),
        resolution_ticks: 1,
        uncertainty_ticks: 0,
    };
    recollected_experience(
        format!("pete/memory/{}", candidate.identity),
        &SelectedRecollection {
            record_id: candidate.identity.clone(),
            content_kind: content_kind.into(),
            canonical_content,
            occurred_at: instant(source.event_at_millis),
            recorded_at: instant(source.recorded_at_millis),
            original_sources,
            certainty: ExperienceCertainty::Uncertain,
        },
    )
    .map_err(|_| PeteExperienceRefusal::InvalidFact)
}
