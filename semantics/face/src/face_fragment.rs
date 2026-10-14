//! Bounded universal Face truth contributed by one ordinary Plot play.

use alloc::{string::String, vec::Vec};
use conduit_core::{ActivePlayId, CheckedPlotId, PlanId};

use crate::{
    FaceAction, FaceDisclosure, FaceProperty, FaceRelationship, FaceSubject, FaceTemporalFact,
    FaceText, TemporalReference, MAX_FACE_PROPERTIES, MAX_FACE_RELATIONSHIPS, MAX_FACE_SUBJECTS,
    MAX_FACE_TEXT_ITEMS,
};

/// Exact source and optional Face-context requirement for one contribution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceContributionBasis {
    pub checked_plot_id: CheckedPlotId,
    pub plan_id: PlanId,
    pub active_play_id: ActivePlayId,
    /// An exact context requirement, never an audience role or authority grant.
    pub required_interaction_context: Option<String>,
}

/// A partial universal grammar. The Face owns the outer Body basis, interaction
/// context, canonical identity, and final cross-contribution validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceFragment {
    pub basis: FaceContributionBasis,
    pub subjects: Vec<FaceSubject>,
    pub relationships: Vec<FaceRelationship>,
    pub composition: Vec<crate::FaceCompositionRelation>,
    pub properties: Vec<FaceProperty>,
    pub text: Vec<FaceText>,
    pub actions: Vec<FaceAction>,
    pub disclosures: Vec<FaceDisclosure>,
    pub temporal_references: Vec<TemporalReference>,
    pub temporal_facts: Vec<FaceTemporalFact>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaceFragmentError {
    Empty,
    TooManySubjects,
    TooManyRelationships,
    TooManyCompositionRelations,
    TooManyProperties,
    TooManyTextItems,
    TooManyActions,
    TooManyDisclosures,
    TooManyTemporalReferences,
    TooManyTemporalFacts,
    InvalidContextRequirement,
    InvalidIdentity,
    DuplicateSubject,
    DuplicateAction,
    DuplicateCompositionRelation,
}

impl FaceFragment {
    /// Validate independent capacity and context facts. Exact references and
    /// collisions are validated only after deterministic Face composition,
    /// because a fragment may intentionally relate to another fragment's truth.
    pub fn validate_bounds(&self) -> Result<(), FaceFragmentError> {
        if self.subjects.is_empty()
            && self.relationships.is_empty()
            && self.composition.is_empty()
            && self.properties.is_empty()
            && self.text.is_empty()
            && self.actions.is_empty()
            && self.disclosures.is_empty()
            && self.temporal_references.is_empty()
            && self.temporal_facts.is_empty()
        {
            return Err(FaceFragmentError::Empty);
        }
        if self.subjects.len() > MAX_FACE_SUBJECTS {
            return Err(FaceFragmentError::TooManySubjects);
        }
        if self.relationships.len() > MAX_FACE_RELATIONSHIPS {
            return Err(FaceFragmentError::TooManyRelationships);
        }
        if self.composition.len() > crate::MAX_FACE_COMPOSITION_RELATIONS {
            return Err(FaceFragmentError::TooManyCompositionRelations);
        }
        if self.properties.len() > MAX_FACE_PROPERTIES {
            return Err(FaceFragmentError::TooManyProperties);
        }
        if self.text.len() > MAX_FACE_TEXT_ITEMS {
            return Err(FaceFragmentError::TooManyTextItems);
        }
        if self.actions.len() > crate::MAX_FACE_ACTIONS {
            return Err(FaceFragmentError::TooManyActions);
        }
        if self.disclosures.len() > crate::MAX_FACE_DISCLOSURES {
            return Err(FaceFragmentError::TooManyDisclosures);
        }
        if self.temporal_references.len() > crate::MAX_TEMPORAL_REFERENCES {
            return Err(FaceFragmentError::TooManyTemporalReferences);
        }
        if self.temporal_facts.len() > crate::MAX_FACE_TEMPORAL_FACTS {
            return Err(FaceFragmentError::TooManyTemporalFacts);
        }
        if self
            .basis
            .required_interaction_context
            .as_ref()
            .is_some_and(|identity| {
                identity.is_empty() || identity.len() > crate::MAX_FACE_ID_BYTES
            })
        {
            return Err(FaceFragmentError::InvalidContextRequirement);
        }
        for (index, subject) in self.subjects.iter().enumerate() {
            if !valid_identity(&subject.identity) {
                return Err(FaceFragmentError::InvalidIdentity);
            }
            if self.subjects[index + 1..]
                .iter()
                .any(|candidate| candidate.identity == subject.identity)
            {
                return Err(FaceFragmentError::DuplicateSubject);
            }
        }
        for (index, action) in self.actions.iter().enumerate() {
            if !valid_identity(&action.identity) {
                return Err(FaceFragmentError::InvalidIdentity);
            }
            if self.actions[index + 1..]
                .iter()
                .any(|candidate| candidate.identity == action.identity)
            {
                return Err(FaceFragmentError::DuplicateAction);
            }
        }
        for (index, relation) in self.composition.iter().enumerate() {
            if !valid_identity(&relation.identity)
                || !valid_identity(&relation.source)
                || !valid_identity(&relation.target)
            {
                return Err(FaceFragmentError::InvalidIdentity);
            }
            if self.composition[index + 1..]
                .iter()
                .any(|candidate| candidate.identity == relation.identity)
            {
                return Err(FaceFragmentError::DuplicateCompositionRelation);
            }
        }
        Ok(())
    }
}

fn valid_identity(value: &str) -> bool {
    !value.is_empty() && value.len() <= crate::MAX_FACE_ID_BYTES
}
