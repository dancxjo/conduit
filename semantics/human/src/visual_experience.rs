//! Bounded relation of typed visual observations from one exact image generation.

use alloc::{boxed::Box, vec::Vec};
use conduit_core::{KindId, SignId};

use crate::{
    ImageObservationReference, MotionObservation, ObjectObservation, TrackObservation,
    VisibleTextObservation, VisualEvidenceClass, VisualImpression, VisualImpressionRefusal,
    VisualObservationRefusal,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VisualExperienceLimits {
    pub maximum_observations: usize,
    pub maximum_objects: usize,
    pub maximum_visible_texts: usize,
    pub maximum_motions: usize,
    pub maximum_tracks: usize,
    pub maximum_impressions: usize,
    pub maximum_relations: usize,
    pub maximum_text_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VisualExperienceObservation {
    Object(Box<ObjectObservation>),
    VisibleText(Box<VisibleTextObservation>),
    Motion(Box<MotionObservation>),
    Track(Box<TrackObservation>),
    Impression(Box<VisualImpression>),
}

impl VisualExperienceObservation {
    pub fn source_image(&self) -> &ImageObservationReference {
        match self {
            Self::Object(value) => &value.source_image,
            Self::VisibleText(value) => &value.source_image,
            Self::Motion(value) => &value.source_image,
            Self::Track(value) => &value.source_image,
            Self::Impression(value) => &value.source_image,
        }
    }

    pub fn sign_id(&self) -> &SignId {
        match self {
            Self::Object(value) => &value.provenance.observation_sign_id,
            Self::VisibleText(value) => &value.provenance.observation_sign_id,
            Self::Motion(value) => &value.provenance.observation_sign_id,
            Self::Track(value) => &value.provenance.observation_sign_id,
            Self::Impression(value) => &value.provenance.observation_sign_id,
        }
    }

    pub fn evidence_class(&self) -> VisualEvidenceClass {
        match self {
            Self::Object(value) => value.provenance.evidence_class,
            Self::VisibleText(value) => value.provenance.evidence_class,
            Self::Motion(value) => value.provenance.evidence_class,
            Self::Track(value) => value.provenance.evidence_class,
            Self::Impression(value) => value.provenance.evidence_class,
        }
    }

    fn text_bytes(&self) -> usize {
        match self {
            Self::Object(value) => value.candidate_label.len(),
            Self::VisibleText(value) => value.text.len(),
            Self::Impression(value) => value.text.len(),
            Self::Motion(_) | Self::Track(_) => 0,
        }
    }

    fn validate(&self, profile: &KindId) -> Result<(), VisualExperienceRefusal> {
        match self {
            Self::Object(value) => value
                .validate(profile)
                .map_err(VisualExperienceRefusal::InvalidObservation),
            Self::VisibleText(value) => value
                .validate(profile)
                .map_err(VisualExperienceRefusal::InvalidObservation),
            Self::Motion(value) => value
                .validate(profile)
                .map_err(VisualExperienceRefusal::InvalidObservation),
            Self::Track(value) => value
                .validate(profile)
                .map_err(VisualExperienceRefusal::InvalidObservation),
            Self::Impression(value) => value
                .validate(profile)
                .map_err(VisualExperienceRefusal::InvalidImpression),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VisualExperienceRelationKind {
    Relates,
    Supports,
    Contradicts,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VisualExperienceRelation {
    pub subject_sign_id: SignId,
    pub object_sign_id: SignId,
    pub kind: VisualExperienceRelationKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VisualExperienceRefusal {
    InvalidLimits,
    InvalidSourceImage,
    WrongSourceImage,
    ObservationCapacity,
    ObservationKindCapacity,
    TextCapacity,
    DuplicateObservation,
    InvalidObservation(VisualObservationRefusal),
    InvalidImpression(VisualImpressionRefusal),
    RelationCapacity,
    UnknownRelationEndpoint,
    SelfRelation,
    DuplicateRelation,
    ArithmeticOverflow,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RejectedVisualObservation {
    pub refusal: VisualExperienceRefusal,
    pub observation: Box<VisualExperienceObservation>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VisualExperience {
    source_image: ImageObservationReference,
    image_profile: KindId,
    limits: VisualExperienceLimits,
    observations: Vec<VisualExperienceObservation>,
    relations: Vec<VisualExperienceRelation>,
    text_bytes: usize,
}

impl VisualExperience {
    pub fn new(
        source_image: ImageObservationReference,
        image_profile: KindId,
        limits: VisualExperienceLimits,
    ) -> Result<Self, VisualExperienceRefusal> {
        if limits.maximum_observations == 0
            || limits.maximum_objects == 0
            || limits.maximum_visible_texts == 0
            || limits.maximum_motions == 0
            || limits.maximum_tracks == 0
            || limits.maximum_impressions == 0
            || limits.maximum_relations == 0
            || limits.maximum_text_bytes == 0
            || limits.maximum_objects > limits.maximum_observations
            || limits.maximum_visible_texts > limits.maximum_observations
            || limits.maximum_motions > limits.maximum_observations
            || limits.maximum_tracks > limits.maximum_observations
            || limits.maximum_impressions > limits.maximum_observations
        {
            return Err(VisualExperienceRefusal::InvalidLimits);
        }
        source_image
            .validate(&image_profile)
            .map_err(|_| VisualExperienceRefusal::InvalidSourceImage)?;
        Ok(Self {
            source_image,
            image_profile,
            observations: Vec::with_capacity(limits.maximum_observations),
            relations: Vec::with_capacity(limits.maximum_relations),
            text_bytes: 0,
            limits,
        })
    }

    pub fn source_image(&self) -> &ImageObservationReference {
        &self.source_image
    }

    pub fn observations(&self) -> &[VisualExperienceObservation] {
        &self.observations
    }

    pub fn relations(&self) -> &[VisualExperienceRelation] {
        &self.relations
    }

    pub fn text_bytes(&self) -> usize {
        self.text_bytes
    }

    pub fn try_admit(
        &mut self,
        observation: VisualExperienceObservation,
    ) -> Result<(), RejectedVisualObservation> {
        if let Err(refusal) = self.validate_admission(&observation) {
            return Err(RejectedVisualObservation {
                refusal,
                observation: Box::new(observation),
            });
        }
        self.text_bytes += observation.text_bytes();
        self.observations.push(observation);
        Ok(())
    }

    pub fn relate(
        &mut self,
        relation: VisualExperienceRelation,
    ) -> Result<(), VisualExperienceRefusal> {
        if relation.subject_sign_id == relation.object_sign_id {
            return Err(VisualExperienceRefusal::SelfRelation);
        }
        if !self.contains_sign(&relation.subject_sign_id)
            || !self.contains_sign(&relation.object_sign_id)
        {
            return Err(VisualExperienceRefusal::UnknownRelationEndpoint);
        }
        if self.relations.contains(&relation) {
            return Err(VisualExperienceRefusal::DuplicateRelation);
        }
        if self.relations.len() == self.limits.maximum_relations {
            return Err(VisualExperienceRefusal::RelationCapacity);
        }
        self.relations.push(relation);
        Ok(())
    }

    fn validate_admission(
        &self,
        observation: &VisualExperienceObservation,
    ) -> Result<(), VisualExperienceRefusal> {
        observation.validate(&self.image_profile)?;
        if observation.source_image() != &self.source_image {
            return Err(VisualExperienceRefusal::WrongSourceImage);
        }
        if self.contains_sign(observation.sign_id()) {
            return Err(VisualExperienceRefusal::DuplicateObservation);
        }
        if self.observations.len() == self.limits.maximum_observations {
            return Err(VisualExperienceRefusal::ObservationCapacity);
        }
        let (count, maximum) = match observation {
            VisualExperienceObservation::Object(_) => (
                self.count_kind(|value| matches!(value, VisualExperienceObservation::Object(_))),
                self.limits.maximum_objects,
            ),
            VisualExperienceObservation::VisibleText(_) => (
                self.count_kind(|value| {
                    matches!(value, VisualExperienceObservation::VisibleText(_))
                }),
                self.limits.maximum_visible_texts,
            ),
            VisualExperienceObservation::Motion(_) => (
                self.count_kind(|value| matches!(value, VisualExperienceObservation::Motion(_))),
                self.limits.maximum_motions,
            ),
            VisualExperienceObservation::Track(_) => (
                self.count_kind(|value| matches!(value, VisualExperienceObservation::Track(_))),
                self.limits.maximum_tracks,
            ),
            VisualExperienceObservation::Impression(_) => (
                self.count_kind(|value| {
                    matches!(value, VisualExperienceObservation::Impression(_))
                }),
                self.limits.maximum_impressions,
            ),
        };
        if count == maximum {
            return Err(VisualExperienceRefusal::ObservationKindCapacity);
        }
        let next_text_bytes = self
            .text_bytes
            .checked_add(observation.text_bytes())
            .ok_or(VisualExperienceRefusal::ArithmeticOverflow)?;
        if next_text_bytes > self.limits.maximum_text_bytes {
            return Err(VisualExperienceRefusal::TextCapacity);
        }
        Ok(())
    }

    fn contains_sign(&self, sign_id: &SignId) -> bool {
        self.observations
            .iter()
            .any(|observation| observation.sign_id() == sign_id)
    }

    fn count_kind(&self, predicate: impl Fn(&VisualExperienceObservation) -> bool) -> usize {
        self.observations
            .iter()
            .filter(|value| predicate(value))
            .count()
    }
}
