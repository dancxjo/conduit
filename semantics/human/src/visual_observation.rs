//! Bounded typed visual observations over an exact image resource generation.

use alloc::{string::String, vec::Vec};
use conduit_core::{
    ArtifactId, BaseImplementationId, BaseInstanceId, KindId, SignId, TemporalInstant,
};

use crate::{ImageObservationReference, ImageObservationRefusal};

pub const MAXIMUM_VISUAL_LABEL_BYTES: usize = 128;
pub const MAXIMUM_VISIBLE_TEXT_BYTES: usize = 512;
pub const MAXIMUM_VISUAL_IDENTITY_BYTES: usize = 128;
pub const MAXIMUM_TRACK_OBSERVATIONS: usize = 16;
pub const MAXIMUM_CONFIDENCE_PERMILLE: u16 = 1_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImageRegion {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl ImageRegion {
    pub fn validate(
        self,
        image: &ImageObservationReference,
    ) -> Result<(), VisualObservationRefusal> {
        if self.width == 0
            || self.height == 0
            || u32::from(self.x) + u32::from(self.width) > u32::from(image.width)
            || u32::from(self.y) + u32::from(self.height) > u32::from(image.height)
        {
            return Err(VisualObservationRefusal::InvalidRegion);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VisualEvidenceClass {
    DeterministicDerived,
    StatisticalCandidate,
    ModelDerived,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VisualObservationProvenance {
    pub evidence_class: VisualEvidenceClass,
    pub observation_sign_id: SignId,
    pub observed_at: TemporalInstant,
    pub implementation_id: BaseImplementationId,
    pub provider_instance_id: BaseInstanceId,
    pub artifact_id: ArtifactId,
    pub run_id: String,
}

impl VisualObservationProvenance {
    pub fn validate(&self) -> Result<(), VisualObservationRefusal> {
        self.observed_at
            .validate()
            .map_err(|_| VisualObservationRefusal::InvalidObservationTime)?;
        for value in [
            self.observation_sign_id.as_str(),
            self.implementation_id.as_str(),
            self.provider_instance_id.as_str(),
            self.artifact_id.as_str(),
            &self.run_id,
        ] {
            validate_identity(value)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectObservation {
    pub source_image: ImageObservationReference,
    pub candidate_label: String,
    pub region: ImageRegion,
    pub confidence_permille: u16,
    pub provenance: VisualObservationProvenance,
}

impl ObjectObservation {
    pub fn validate(&self, image_profile: &KindId) -> Result<(), VisualObservationRefusal> {
        validate_image(&self.source_image, image_profile)?;
        validate_text(&self.candidate_label, MAXIMUM_VISUAL_LABEL_BYTES)?;
        self.region.validate(&self.source_image)?;
        validate_confidence(self.confidence_permille)?;
        if !matches!(
            self.provenance.evidence_class,
            VisualEvidenceClass::DeterministicDerived | VisualEvidenceClass::StatisticalCandidate
        ) {
            return Err(VisualObservationRefusal::WrongEvidenceClass);
        }
        self.provenance.validate()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VisibleTextObservation {
    pub source_image: ImageObservationReference,
    pub text: String,
    pub region: Option<ImageRegion>,
    pub confidence_permille: u16,
    pub provenance: VisualObservationProvenance,
}

impl VisibleTextObservation {
    pub fn validate(&self, image_profile: &KindId) -> Result<(), VisualObservationRefusal> {
        validate_image(&self.source_image, image_profile)?;
        validate_text(&self.text, MAXIMUM_VISIBLE_TEXT_BYTES)?;
        if let Some(region) = self.region {
            region.validate(&self.source_image)?;
        }
        validate_confidence(self.confidence_permille)?;
        if self.provenance.evidence_class != VisualEvidenceClass::StatisticalCandidate {
            return Err(VisualObservationRefusal::WrongEvidenceClass);
        }
        self.provenance.validate()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MotionObservation {
    pub source_image: ImageObservationReference,
    pub changed_region: ImageRegion,
    pub change_permille: u16,
    pub provenance: VisualObservationProvenance,
}

impl MotionObservation {
    pub fn validate(&self, image_profile: &KindId) -> Result<(), VisualObservationRefusal> {
        validate_image(&self.source_image, image_profile)?;
        self.changed_region.validate(&self.source_image)?;
        validate_confidence(self.change_permille)?;
        if self.provenance.evidence_class != VisualEvidenceClass::DeterministicDerived {
            return Err(VisualObservationRefusal::WrongEvidenceClass);
        }
        self.provenance.validate()
    }
}

/// One continuity hypothesis local to one admitted tracking context.
///
/// `track_id` is not a physical, legal, biometric, or cross-context identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrackObservation {
    pub source_image: ImageObservationReference,
    pub tracking_context_id: String,
    pub track_id: String,
    pub contributing_observation_sign_ids: Vec<SignId>,
    pub current_region: ImageRegion,
    pub continuity_confidence_permille: u16,
    pub provenance: VisualObservationProvenance,
}

impl TrackObservation {
    pub fn validate(&self, image_profile: &KindId) -> Result<(), VisualObservationRefusal> {
        validate_image(&self.source_image, image_profile)?;
        validate_identity(&self.tracking_context_id)?;
        validate_identity(&self.track_id)?;
        self.current_region.validate(&self.source_image)?;
        validate_confidence(self.continuity_confidence_permille)?;
        if self.contributing_observation_sign_ids.is_empty()
            || self.contributing_observation_sign_ids.len() > MAXIMUM_TRACK_OBSERVATIONS
        {
            return Err(VisualObservationRefusal::ObservationRefBound);
        }
        for (index, sign) in self.contributing_observation_sign_ids.iter().enumerate() {
            validate_identity(sign.as_str())?;
            if self.contributing_observation_sign_ids[..index].contains(sign) {
                return Err(VisualObservationRefusal::DuplicateObservationRef);
            }
        }
        if self.provenance.evidence_class != VisualEvidenceClass::StatisticalCandidate {
            return Err(VisualObservationRefusal::WrongEvidenceClass);
        }
        self.provenance.validate()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VisualObservationRefusal {
    InvalidImage,
    WrongImageProfile,
    InvalidImageDimensions,
    ImageTooLarge,
    InvalidRegion,
    EmptyText,
    TextBound,
    ConfidenceBound,
    EmptyIdentity,
    IdentityBound,
    InvalidObservationTime,
    WrongEvidenceClass,
    ObservationRefBound,
    DuplicateObservationRef,
}

fn validate_image(
    image: &ImageObservationReference,
    profile: &KindId,
) -> Result<(), VisualObservationRefusal> {
    image.validate(profile).map_err(|refusal| match refusal {
        ImageObservationRefusal::InvalidResource => VisualObservationRefusal::InvalidImage,
        ImageObservationRefusal::WrongProfile => VisualObservationRefusal::WrongImageProfile,
        ImageObservationRefusal::InvalidDimensions => {
            VisualObservationRefusal::InvalidImageDimensions
        }
        ImageObservationRefusal::ContentTooLarge => VisualObservationRefusal::ImageTooLarge,
    })
}

fn validate_text(value: &str, maximum: usize) -> Result<(), VisualObservationRefusal> {
    if value.is_empty() {
        return Err(VisualObservationRefusal::EmptyText);
    }
    if value.len() > maximum {
        return Err(VisualObservationRefusal::TextBound);
    }
    Ok(())
}

fn validate_identity(value: &str) -> Result<(), VisualObservationRefusal> {
    if value.is_empty() {
        return Err(VisualObservationRefusal::EmptyIdentity);
    }
    if value.len() > MAXIMUM_VISUAL_IDENTITY_BYTES {
        return Err(VisualObservationRefusal::IdentityBound);
    }
    Ok(())
}

fn validate_confidence(value: u16) -> Result<(), VisualObservationRefusal> {
    if value > MAXIMUM_CONFIDENCE_PERMILLE {
        return Err(VisualObservationRefusal::ConfidenceBound);
    }
    Ok(())
}
