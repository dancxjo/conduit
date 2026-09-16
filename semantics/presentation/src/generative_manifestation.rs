//! Correlated, bounded output from one generative Presenter request.

use alloc::{string::String, vec::Vec};
use serde::{Deserialize, Serialize};

use crate::{GenerativePresenterRefusal, GenerativePresenterRequest, PresentationActionRefusal};

pub const MAX_GENERATED_CONTENT_SEGMENTS: usize = 8;
pub const MAX_GENERATED_AFFORDANCES: usize = 32;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum GeneratedManifestationDisposition {
    Produced,
    Truncated,
    Refused,
    Failed,
    Cancelled,
    ProviderLost,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedActionAffordance {
    pub action_identity: String,
    pub source_presentation_revision: u64,
}

/// An intentionally exposed Manifestation role, never private model reasoning.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum GeneratedContentRole {
    /// First-person Body voice intended for outward text, speech, or TTS.
    Speech,
    /// First-person interior narration explicitly made part of the Manifestation.
    PresentedThought,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedContentSegment {
    pub role: GeneratedContentRole,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedManifestation {
    pub manifestation_identity: String,
    pub request_identity: String,
    pub source_presentation_identity: String,
    pub source_presentation_revision: u64,
    pub presenter_implementation_identity: String,
    pub provider_identity: String,
    pub model_identity: String,
    pub template_contract_revision: String,
    pub generation_run_identity: String,
    pub disposition: GeneratedManifestationDisposition,
    pub content: Vec<GeneratedContentSegment>,
    pub affordances: Vec<GeneratedActionAffordance>,
}

pub(crate) fn validate_generated_manifestation(
    request: &GenerativePresenterRequest,
    manifestation: &GeneratedManifestation,
) -> Result<(), GenerativePresenterRefusal> {
    for identity in [
        &manifestation.manifestation_identity,
        &manifestation.presenter_implementation_identity,
        &manifestation.provider_identity,
        &manifestation.model_identity,
        &manifestation.generation_run_identity,
    ] {
        super::generative_presenter::validate_identity(identity)?;
    }
    if manifestation.request_identity != request.request_identity {
        return Err(GenerativePresenterRefusal::RequestMismatch);
    }
    if manifestation.source_presentation_identity
        != request.semantic_data.source_presentation_identity
        || manifestation.source_presentation_revision
            != request.semantic_data.source_presentation_revision
    {
        return Err(GenerativePresenterRefusal::SourcePresentationMismatch);
    }
    if manifestation.template_contract_revision != request.policy.template_contract_revision {
        return Err(GenerativePresenterRefusal::TemplateMismatch);
    }
    if manifestation.content.len() > MAX_GENERATED_CONTENT_SEGMENTS {
        return Err(GenerativePresenterRefusal::TooManyContentSegments);
    }
    let mut output_bytes = 0_usize;
    for segment in &manifestation.content {
        if segment.bytes.is_empty() {
            return Err(GenerativePresenterRefusal::EmptyGeneratedContent);
        }
        output_bytes = output_bytes
            .checked_add(segment.bytes.len())
            .ok_or(GenerativePresenterRefusal::OutputBoundExceeded)?;
    }
    if output_bytes > request.bounds.maximum_output_bytes as usize {
        return Err(GenerativePresenterRefusal::OutputBoundExceeded);
    }
    if manifestation.affordances.len() > MAX_GENERATED_AFFORDANCES {
        return Err(GenerativePresenterRefusal::TooManyAffordances);
    }
    let produced = matches!(
        manifestation.disposition,
        GeneratedManifestationDisposition::Produced | GeneratedManifestationDisposition::Truncated
    );
    if produced && manifestation.content.is_empty() {
        return Err(GenerativePresenterRefusal::EmptyGeneratedContent);
    }
    if !produced && (!manifestation.content.is_empty() || !manifestation.affordances.is_empty()) {
        return Err(GenerativePresenterRefusal::OutputForTerminalDisposition);
    }
    for affordance in &manifestation.affordances {
        if affordance.source_presentation_revision
            != request.semantic_data.source_presentation_revision
        {
            return Err(GenerativePresenterRefusal::StaleAction);
        }
        match request.semantic_data.presentation.resolve_action(
            affordance.source_presentation_revision,
            &affordance.action_identity,
        ) {
            Ok(_) => {}
            Err(PresentationActionRefusal::StaleRevision) => {
                return Err(GenerativePresenterRefusal::StaleAction);
            }
            Err(PresentationActionRefusal::UnknownAction) => {
                return Err(GenerativePresenterRefusal::UnknownAction);
            }
            Err(
                PresentationActionRefusal::Unavailable { .. }
                | PresentationActionRefusal::Refused { .. },
            ) => return Err(GenerativePresenterRefusal::UnavailableAction),
        }
    }
    Ok(())
}
