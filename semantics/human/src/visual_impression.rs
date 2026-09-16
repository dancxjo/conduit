//! A bounded model-derived interpretation of an exact image generation.

use alloc::{string::String, vec::Vec};
use conduit_core::{KindId, SignId};

use crate::{
    ImageObservationReference, VisualEvidenceClass, VisualObservationProvenance,
    VisualObservationRefusal, MAXIMUM_VISUAL_IDENTITY_BYTES,
};

pub const MAXIMUM_VISUAL_IMPRESSION_BYTES: usize = 1_024;
pub const MAXIMUM_IMPRESSION_OBSERVATION_REFS: usize = 24;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VisualImpressionDisposition {
    Complete,
    Truncated { original_bytes: u32 },
}

/// One model interpretation, kept distinct from deterministic and detector observations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VisualImpression {
    pub source_image: ImageObservationReference,
    pub selected_observation_sign_ids: Vec<SignId>,
    pub text: String,
    pub model_id: String,
    pub prompt_contract_revision: String,
    pub disposition: VisualImpressionDisposition,
    pub provenance: VisualObservationProvenance,
}

impl VisualImpression {
    pub fn validate(&self, image_profile: &KindId) -> Result<(), VisualImpressionRefusal> {
        self.source_image
            .validate(image_profile)
            .map_err(|_| VisualImpressionRefusal::InvalidSourceImage)?;
        validate_text(&self.text)?;
        validate_identity(&self.model_id)?;
        validate_identity(&self.prompt_contract_revision)?;
        if self.selected_observation_sign_ids.len() > MAXIMUM_IMPRESSION_OBSERVATION_REFS {
            return Err(VisualImpressionRefusal::ObservationRefBound);
        }
        for (index, sign_id) in self.selected_observation_sign_ids.iter().enumerate() {
            validate_identity(sign_id.as_str())?;
            if self.selected_observation_sign_ids[..index].contains(sign_id) {
                return Err(VisualImpressionRefusal::DuplicateObservationRef);
            }
        }
        match self.disposition {
            VisualImpressionDisposition::Complete => {}
            VisualImpressionDisposition::Truncated { original_bytes }
                if usize::try_from(original_bytes).unwrap_or(usize::MAX) > self.text.len() => {}
            VisualImpressionDisposition::Truncated { .. } => {
                return Err(VisualImpressionRefusal::InvalidTruncation)
            }
        }
        if self.provenance.evidence_class != VisualEvidenceClass::ModelDerived {
            return Err(VisualImpressionRefusal::WrongEvidenceClass);
        }
        self.provenance
            .validate()
            .map_err(VisualImpressionRefusal::InvalidProvenance)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VisualImpressionRefusal {
    InvalidSourceImage,
    EmptyText,
    TextBound,
    EmptyIdentity,
    IdentityBound,
    ObservationRefBound,
    DuplicateObservationRef,
    InvalidTruncation,
    WrongEvidenceClass,
    InvalidProvenance(VisualObservationRefusal),
}

fn validate_text(value: &str) -> Result<(), VisualImpressionRefusal> {
    if value.is_empty() {
        return Err(VisualImpressionRefusal::EmptyText);
    }
    if value.len() > MAXIMUM_VISUAL_IMPRESSION_BYTES {
        return Err(VisualImpressionRefusal::TextBound);
    }
    Ok(())
}

fn validate_identity(value: &str) -> Result<(), VisualImpressionRefusal> {
    if value.is_empty() {
        return Err(VisualImpressionRefusal::EmptyIdentity);
    }
    if value.len() > MAXIMUM_VISUAL_IDENTITY_BYTES {
        return Err(VisualImpressionRefusal::IdentityBound);
    }
    Ok(())
}
