//! Admit a sourced batch atomically through the existing CurrentExperience reducer.
use crate::*;
use alloc::vec::Vec;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExperienceIntegrationError {
    Evidence(ExperienceEvidenceRefusal),
    Admission(ExperienceRefusal),
}

/// The selected revision is complete. Unselected or missing evidence is refused.
pub fn integrate_experience_evidence(
    input: &ExperienceIntegrationInput,
    limits: ExperienceLimits,
) -> Result<CurrentExperience, ExperienceIntegrationError> {
    use ExperienceIntegrationError::{Admission, Evidence};
    let request = input.evidence();
    let selection = request.selection();
    if request.evidence().get().len() != selection.item_identities().get().len() {
        return Err(Evidence(ExperienceEvidenceRefusal::UnselectedEvidence));
    }
    let reference = conduit_core::TemporalInstant::try_from(input.reference().clone())
        .map_err(|_| Admission(ExperienceRefusal::InvalidTime))?;
    let mut current =
        CurrentExperience::new(limits, reference, *input.policy()).map_err(Admission)?;
    for identity in selection.item_identities().get().iter() {
        let item = request
            .evidence()
            .get()
            .iter()
            .find(|item| item.identity() == identity)
            .ok_or(Evidence(ExperienceEvidenceRefusal::MissingEvidence))?;
        if item.provenance_revision() != selection.provenance_revision() {
            return Err(Evidence(ExperienceEvidenceRefusal::ProvenanceMismatch));
        }
        if item.revision() != selection.revision() {
            return Err(Evidence(if item.revision() < selection.revision() {
                ExperienceEvidenceRefusal::StaleRevision
            } else {
                ExperienceEvidenceRefusal::FutureRevision
            }));
        }
        let mut sources = Vec::with_capacity(item.sources().get().len());
        for source in item.sources().get().iter() {
            sources.push(match source {
                ExperienceEvidenceSource::Sign(value) => {
                    ExperienceSourceRef::Sign(value.get().as_str().into())
                }
                ExperienceEvidenceSource::Source(value) => ExperienceSourceRef::Source {
                    source_id: value.get().clone(),
                },
                ExperienceEvidenceSource::HumanStatement(value) => {
                    ExperienceSourceRef::HumanStatement {
                        statement_id: value.get().clone(),
                    }
                }
                ExperienceEvidenceSource::MemoryRecord(value) => {
                    ExperienceSourceRef::MemoryRecord {
                        record_id: value.get().clone(),
                    }
                }
                ExperienceEvidenceSource::Resource(value) => {
                    ExperienceSourceRef::Resource(value.clone())
                }
                ExperienceEvidenceSource::ImplementationRun(value) => {
                    ExperienceSourceRef::ImplementationRun {
                        implementation_id: value.implementation().get().as_str().into(),
                        provider_instance_id: value.provider_instance().get().as_str().into(),
                        artifact_id: value.artifact().get().as_str().into(),
                        run_id: value.run().get().clone(),
                    }
                }
            });
        }
        current
            .try_admit(ExperienceItem {
                id: item.identity().get().clone(),
                domain: *item.domain(),
                content_kind: item.content_kind().get().as_str().into(),
                encoded_content: item.encoded_content().get().as_slice().to_vec(),
                origin: *item.origin(),
                temporal_role: *item.temporal_role(),
                availability: *item.availability(),
                certainty: *item.certainty(),
                observed_at: instant(item.observed_at()).map_err(Admission)?,
                recorded_at: instant(item.recorded_at()).map_err(Admission)?,
                sources,
            })
            .map_err(|error| Admission(error.refusal))?;
    }
    for relation in selection.relationships().get().iter() {
        current
            .relate(ExperienceRelation {
                subject_id: relation.subject().get().clone(),
                object_id: relation.object().get().clone(),
                kind: *relation.kind(),
            })
            .map_err(Admission)?;
    }
    Ok(current)
}

fn instant(
    value: &ExperienceEvidenceInstant,
) -> Result<Option<conduit_core::TemporalInstant>, ExperienceRefusal> {
    match value {
        ExperienceEvidenceInstant::Unknown => Ok(None),
        ExperienceEvidenceInstant::Known(value) => {
            conduit_core::TemporalInstant::try_from(value.instant().clone())
                .map(Some)
                .map_err(|_| ExperienceRefusal::InvalidTime)
        }
    }
}
