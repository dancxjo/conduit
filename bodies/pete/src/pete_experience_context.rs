//! Explicit typed projection of admitted Experience; identities authorize no lookup.

use conduit_human::*;
use conduit_plot::rust_binding::{BoundedBytes, BoundedSequence};

/// Build the request for the ordinary `experience/project-evidence` Gear.
/// No truncation, identity resolution, model call, or new execution loop.
pub fn project_pete_experience_evidence(
    experience: &CurrentExperience,
    revision: u64,
    provenance_revision: &str,
) -> Result<ExperienceEvidenceRequest, ExperienceEvidenceRefusal> {
    project_pete_experience_items(experience.items(), revision, provenance_revision)
}

pub(crate) fn project_pete_experience_items(
    items: &[ExperienceItem],
    revision: u64,
    provenance_revision: &str,
) -> Result<ExperienceEvidenceRequest, ExperienceEvidenceRefusal> {
    use ExperienceEvidenceRefusal as R;
    let identity =
        |value: &str| ExperienceEvidenceIdentity::new(value.into()).map_err(|_| R::IdentityBound);
    if items.len() > conduit_semantic_catalog::EXPERIENCE_EVIDENCE_MAX_ITEMS {
        return Err(R::ItemBound);
    }
    let provenance = identity(provenance_revision)?;
    let mut evidence = Vec::with_capacity(items.len());
    let mut identities = Vec::with_capacity(items.len());
    for item in items {
        let mut sources = Vec::with_capacity(item.sources.len());
        for source in &item.sources {
            sources.push(
                match source {
                    ExperienceSourceRef::Sign(value) => {
                        ExperienceEvidenceSource::sign(identity(value.as_str())?)
                    }
                    ExperienceSourceRef::Source { source_id } => {
                        ExperienceEvidenceSource::source(identity(source_id)?)
                    }
                    ExperienceSourceRef::HumanStatement { statement_id } => {
                        ExperienceEvidenceSource::human_statement(identity(statement_id)?)
                    }
                    ExperienceSourceRef::MemoryRecord { record_id } => {
                        ExperienceEvidenceSource::memory_record(identity(record_id)?)
                    }
                    ExperienceSourceRef::Resource(value) => {
                        ExperienceEvidenceSource::resource(value.clone())
                    }
                    ExperienceSourceRef::ImplementationRun {
                        implementation_id,
                        provider_instance_id,
                        artifact_id,
                        run_id,
                    } => ExperienceEvidenceSource::implementation_run(
                        identity(artifact_id.as_str())?,
                        identity(implementation_id.as_str())?,
                        identity(provider_instance_id.as_str())?,
                        identity(run_id)?,
                    ),
                }
                .map_err(|_| R::Malformed)?,
            );
        }
        let id = identity(&item.id)?;
        identities.push(id.clone());
        evidence.push(
            ExperienceEvidenceItem::new(
                item.availability,
                item.certainty,
                identity(item.content_kind.as_str())?,
                item.domain,
                ExperienceEvidenceContent::new(
                    BoundedBytes::new(&item.encoded_content).ok_or(R::ByteBound)?,
                )
                .map_err(|_| R::ByteBound)?,
                id,
                item.origin,
                provenance.clone(),
                revision,
                ExperienceEvidenceSources::new(
                    BoundedSequence::try_from_iter(sources).map_err(|_| R::SourceBound)?,
                )
                .map_err(|_| R::SourceBound)?,
                item.temporal_role,
            )
            .map_err(|_| R::Malformed)?,
        );
    }
    let selection = ExperienceEvidenceSelection::new(
        ExperienceEvidenceIdentities::new(
            BoundedSequence::try_from_iter(identities).map_err(|_| R::ItemBound)?,
        )
        .map_err(|_| R::ItemBound)?,
        provenance,
        revision,
    )
    .map_err(|_| R::Malformed)?;
    ExperienceEvidenceRequest::new(
        ExperienceEvidenceItems::new(
            BoundedSequence::try_from_iter(evidence).map_err(|_| R::ItemBound)?,
        )
        .map_err(|_| R::ItemBound)?,
        selection,
    )
    .map_err(|_| R::Malformed)
}
