//! Exact typed source adapters for [`CurrentExperience`](crate::CurrentExperience).

use alloc::{string::String, vec, vec::Vec};
use conduit_core::{KindId, SignId, TemporalInstant};

use crate::{
    ExperienceAvailability, ExperienceCertainty, ExperienceDomain, ExperienceItem,
    ExperienceOrigin, ExperienceSourceRef, ExperienceTemporalRole, ObjectObservation,
    VisualImpression, VisualImpressionRefusal, VisualObservationRefusal,
};

pub const MAXIMUM_UTTERANCE_BYTES: usize = 2_048;
pub const MAXIMUM_BODY_SELF_STATE_BYTES: usize = 4_096;
pub const MAXIMUM_RECOLLECTION_BYTES: usize = 4_096;
pub const MAXIMUM_RECOLLECTION_SOURCE_REFS: usize = 32;
pub const MAXIMUM_EXPERIENCE_SOURCE_IDENTITY_BYTES: usize = 128;

const VISUAL_OBJECT_KIND: &str = "experience/visual-object@1";
const VISUAL_IMPRESSION_KIND: &str = "experience/visual-impression@1";
const HUMAN_UTTERANCE_KIND: &str = "experience/human-utterance@1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HumanUtteranceObservation {
    pub text: String,
    pub speaker_id: String,
    pub statement_id: String,
    pub observation_sign_id: SignId,
    pub observed_at: TemporalInstant,
    pub certainty: ExperienceCertainty,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodySelfObservation {
    pub state_kind: KindId,
    pub canonical_state: Vec<u8>,
    pub observation_sign_id: SignId,
    pub observed_at: TemporalInstant,
    pub certainty: ExperienceCertainty,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedRecollection {
    pub record_id: String,
    pub content_kind: KindId,
    pub canonical_content: Vec<u8>,
    pub occurred_at: TemporalInstant,
    pub recorded_at: TemporalInstant,
    pub original_sources: Vec<ExperienceSourceRef>,
    pub certainty: ExperienceCertainty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExperienceSourceRefusal {
    InvalidVisualObservation(VisualObservationRefusal),
    InvalidVisualImpression(VisualImpressionRefusal),
    EmptyValue,
    ValueBound,
    InvalidIdentity,
    InvalidTime,
    SourceBound,
}

pub fn visual_object_experience(
    item_id: impl Into<String>,
    observation: &ObjectObservation,
    image_profile: &KindId,
    temporal_role: ExperienceTemporalRole,
) -> Result<ExperienceItem, ExperienceSourceRefusal> {
    observation
        .validate(image_profile)
        .map_err(ExperienceSourceRefusal::InvalidVisualObservation)?;
    let mut content = Vec::new();
    field(&mut content, observation.candidate_label.as_bytes());
    content.extend_from_slice(&observation.region.x.to_le_bytes());
    content.extend_from_slice(&observation.region.y.to_le_bytes());
    content.extend_from_slice(&observation.region.width.to_le_bytes());
    content.extend_from_slice(&observation.region.height.to_le_bytes());
    content.extend_from_slice(&observation.confidence_permille.to_le_bytes());
    Ok(ExperienceItem {
        id: item_id.into(),
        domain: ExperienceDomain::Visual,
        content_kind: KindId::from(VISUAL_OBJECT_KIND),
        encoded_content: content,
        origin: ExperienceOrigin::Observation,
        temporal_role,
        availability: ExperienceAvailability::Present,
        certainty: ExperienceCertainty::Uncertain,
        observed_at: Some(observation.provenance.observed_at.clone()),
        recorded_at: None,
        sources: vec![
            ExperienceSourceRef::Sign(observation.provenance.observation_sign_id.clone()),
            ExperienceSourceRef::Resource(observation.source_image.content.clone()),
            implementation_run(&observation.provenance),
        ],
    })
}

pub fn visual_impression_experience(
    item_id: impl Into<String>,
    impression: &VisualImpression,
    image_profile: &KindId,
    temporal_role: ExperienceTemporalRole,
) -> Result<ExperienceItem, ExperienceSourceRefusal> {
    impression
        .validate(image_profile)
        .map_err(ExperienceSourceRefusal::InvalidVisualImpression)?;
    let mut content = Vec::new();
    field(&mut content, impression.text.as_bytes());
    field(&mut content, impression.model_id.as_bytes());
    field(&mut content, impression.prompt_contract_revision.as_bytes());
    match impression.disposition {
        crate::VisualImpressionDisposition::Complete => content.push(0),
        crate::VisualImpressionDisposition::Truncated { original_bytes } => {
            content.push(1);
            content.extend_from_slice(&original_bytes.to_le_bytes());
        }
    }
    let mut sources = Vec::with_capacity(impression.selected_observation_sign_ids.len() + 3);
    sources.push(ExperienceSourceRef::Sign(
        impression.provenance.observation_sign_id.clone(),
    ));
    sources.push(ExperienceSourceRef::Resource(
        impression.source_image.content.clone(),
    ));
    sources.push(implementation_run(&impression.provenance));
    sources.extend(
        impression
            .selected_observation_sign_ids
            .iter()
            .cloned()
            .map(ExperienceSourceRef::Sign),
    );
    Ok(ExperienceItem {
        id: item_id.into(),
        domain: ExperienceDomain::Visual,
        content_kind: KindId::from(VISUAL_IMPRESSION_KIND),
        encoded_content: content,
        origin: ExperienceOrigin::ModelDerived,
        temporal_role,
        availability: ExperienceAvailability::Present,
        certainty: ExperienceCertainty::Uncertain,
        observed_at: Some(impression.provenance.observed_at.clone()),
        recorded_at: None,
        sources,
    })
}

pub fn human_utterance_experience(
    item_id: impl Into<String>,
    observation: &HumanUtteranceObservation,
    temporal_role: ExperienceTemporalRole,
) -> Result<ExperienceItem, ExperienceSourceRefusal> {
    validate_text(&observation.text, MAXIMUM_UTTERANCE_BYTES)?;
    validate_identity(&observation.speaker_id)?;
    validate_identity(&observation.statement_id)?;
    validate_identity(observation.observation_sign_id.as_str())?;
    validate_time(&observation.observed_at)?;
    let mut content = Vec::new();
    field(&mut content, observation.speaker_id.as_bytes());
    field(&mut content, observation.text.as_bytes());
    Ok(ExperienceItem {
        id: item_id.into(),
        domain: ExperienceDomain::HumanUtterance,
        content_kind: KindId::from(HUMAN_UTTERANCE_KIND),
        encoded_content: content,
        origin: ExperienceOrigin::HumanReported,
        temporal_role,
        availability: ExperienceAvailability::Present,
        certainty: observation.certainty,
        observed_at: Some(observation.observed_at.clone()),
        recorded_at: None,
        sources: vec![
            ExperienceSourceRef::Sign(observation.observation_sign_id.clone()),
            ExperienceSourceRef::HumanStatement {
                statement_id: observation.statement_id.clone(),
            },
        ],
    })
}

pub fn body_self_experience(
    item_id: impl Into<String>,
    observation: &BodySelfObservation,
    temporal_role: ExperienceTemporalRole,
) -> Result<ExperienceItem, ExperienceSourceRefusal> {
    validate_identity(observation.state_kind.as_str())?;
    validate_identity(observation.observation_sign_id.as_str())?;
    validate_time(&observation.observed_at)?;
    if observation.canonical_state.is_empty() {
        return Err(ExperienceSourceRefusal::EmptyValue);
    }
    if observation.canonical_state.len() > MAXIMUM_BODY_SELF_STATE_BYTES {
        return Err(ExperienceSourceRefusal::ValueBound);
    }
    Ok(ExperienceItem {
        id: item_id.into(),
        domain: ExperienceDomain::BodyState,
        content_kind: observation.state_kind.clone(),
        encoded_content: observation.canonical_state.clone(),
        origin: ExperienceOrigin::Observation,
        temporal_role,
        availability: ExperienceAvailability::Present,
        certainty: observation.certainty,
        observed_at: Some(observation.observed_at.clone()),
        recorded_at: None,
        sources: vec![ExperienceSourceRef::Sign(
            observation.observation_sign_id.clone(),
        )],
    })
}

/// Relates selected retained evidence without promoting it to current truth.
pub fn recollected_experience(
    item_id: impl Into<String>,
    recollection: &SelectedRecollection,
) -> Result<ExperienceItem, ExperienceSourceRefusal> {
    validate_identity(&recollection.record_id)?;
    validate_identity(recollection.content_kind.as_str())?;
    validate_time(&recollection.occurred_at)?;
    validate_time(&recollection.recorded_at)?;
    if recollection.canonical_content.is_empty() {
        return Err(ExperienceSourceRefusal::EmptyValue);
    }
    if recollection.canonical_content.len() > MAXIMUM_RECOLLECTION_BYTES {
        return Err(ExperienceSourceRefusal::ValueBound);
    }
    if recollection.original_sources.is_empty()
        || recollection.original_sources.len() > MAXIMUM_RECOLLECTION_SOURCE_REFS
    {
        return Err(ExperienceSourceRefusal::SourceBound);
    }
    let mut sources = Vec::with_capacity(recollection.original_sources.len() + 1);
    sources.push(ExperienceSourceRef::MemoryRecord {
        record_id: recollection.record_id.clone(),
    });
    sources.extend(recollection.original_sources.iter().cloned());
    Ok(ExperienceItem {
        id: item_id.into(),
        domain: ExperienceDomain::Recollection,
        content_kind: recollection.content_kind.clone(),
        encoded_content: recollection.canonical_content.clone(),
        origin: ExperienceOrigin::Remembered,
        temporal_role: ExperienceTemporalRole::Historical,
        availability: ExperienceAvailability::Present,
        certainty: recollection.certainty,
        observed_at: Some(recollection.occurred_at.clone()),
        recorded_at: Some(recollection.recorded_at.clone()),
        sources,
    })
}

fn validate_text(value: &str, maximum: usize) -> Result<(), ExperienceSourceRefusal> {
    if value.is_empty() {
        return Err(ExperienceSourceRefusal::EmptyValue);
    }
    if value.len() > maximum {
        return Err(ExperienceSourceRefusal::ValueBound);
    }
    Ok(())
}

fn validate_identity(value: &str) -> Result<(), ExperienceSourceRefusal> {
    if value.is_empty() || value.len() > MAXIMUM_EXPERIENCE_SOURCE_IDENTITY_BYTES {
        return Err(ExperienceSourceRefusal::InvalidIdentity);
    }
    Ok(())
}

fn validate_time(value: &TemporalInstant) -> Result<(), ExperienceSourceRefusal> {
    value
        .validate()
        .map_err(|_| ExperienceSourceRefusal::InvalidTime)
}

fn field(output: &mut Vec<u8>, value: &[u8]) {
    output.extend_from_slice(&(value.len() as u32).to_le_bytes());
    output.extend_from_slice(value);
}

fn implementation_run(provenance: &crate::VisualObservationProvenance) -> ExperienceSourceRef {
    ExperienceSourceRef::ImplementationRun {
        implementation_id: provenance.implementation_id.clone(),
        provider_instance_id: provenance.provider_instance_id.clone(),
        artifact_id: provenance.artifact_id.clone(),
        run_id: provenance.run_id.clone(),
    }
}
