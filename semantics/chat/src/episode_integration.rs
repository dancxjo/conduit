//! Correlate the ordinary generic Experience reducer with an episode trigger.
use crate::{EpisodeExperience, EpisodeExperienceInput};
use conduit_human::{
    integrate_experience_evidence, project_experience_evidence, ExperienceIntegrationError,
    ExperienceLimits,
};

pub fn integrate_episode_experience(
    input: &EpisodeExperienceInput,
    limits: ExperienceLimits,
) -> Result<EpisodeExperience, ExperienceIntegrationError> {
    let batch = input.integration();
    let current = integrate_experience_evidence(batch, limits)?;
    let selection = batch.evidence().selection();
    let evidence = project_experience_evidence(
        &current,
        *selection.revision(),
        selection.provenance_revision().get(),
    )
    .map_err(ExperienceIntegrationError::Evidence)?;
    EpisodeExperience::new(
        evidence,
        input.observation().clone(),
        batch.reference().clone(),
    )
    .map_err(|_| {
        ExperienceIntegrationError::Evidence(conduit_human::ExperienceEvidenceRefusal::Malformed)
    })
}
