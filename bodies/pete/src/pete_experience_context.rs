//! Pete's public adapter delegates the generic CurrentExperience projection.
use conduit_human::{CurrentExperience, ExperienceEvidenceRefusal, ExperienceEvidenceRequest};

pub fn project_pete_experience_evidence(
    experience: &CurrentExperience,
    revision: u64,
    provenance_revision: &str,
) -> Result<ExperienceEvidenceRequest, ExperienceEvidenceRefusal> {
    conduit_human::project_experience_evidence(experience, revision, provenance_revision)
}
