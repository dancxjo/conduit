//! Bounded universal Presentation truth contributed by one ordinary Form play.

use alloc::{string::String, vec::Vec};
use conduit_core::{ActivePlayId, CheckedFormId, PlanId};

use crate::{
    PresentationAction, PresentationDisclosure, PresentationProperty, PresentationRelationship,
    PresentationSubject, PresentationTemporalFact, PresentationText, TemporalReference,
    MAX_PRESENTATION_PROPERTIES, MAX_PRESENTATION_RELATIONSHIPS, MAX_PRESENTATION_SUBJECTS,
    MAX_PRESENTATION_TEXT_ITEMS,
};

/// Exact source and optional Face-context requirement for one contribution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresentationContributionBasis {
    pub checked_form_id: CheckedFormId,
    pub plan_id: PlanId,
    pub active_play_id: ActivePlayId,
    /// An exact context requirement, never an audience role or authority grant.
    pub required_interaction_context: Option<String>,
}

/// A partial universal grammar. The Face owns the outer Body basis, interaction
/// context, canonical identity, and final cross-contribution validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresentationFragment {
    pub basis: PresentationContributionBasis,
    pub subjects: Vec<PresentationSubject>,
    pub relationships: Vec<PresentationRelationship>,
    pub composition: Vec<crate::PresentationCompositionRelation>,
    pub properties: Vec<PresentationProperty>,
    pub text: Vec<PresentationText>,
    pub actions: Vec<PresentationAction>,
    pub disclosures: Vec<PresentationDisclosure>,
    pub temporal_references: Vec<TemporalReference>,
    pub temporal_facts: Vec<PresentationTemporalFact>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentationFragmentError {
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

impl PresentationFragment {
    /// Validate independent capacity and context facts. Exact references and
    /// collisions are validated only after deterministic Face composition,
    /// because a fragment may intentionally relate to another fragment's truth.
    pub fn validate_bounds(&self) -> Result<(), PresentationFragmentError> {
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
            return Err(PresentationFragmentError::Empty);
        }
        if self.subjects.len() > MAX_PRESENTATION_SUBJECTS {
            return Err(PresentationFragmentError::TooManySubjects);
        }
        if self.relationships.len() > MAX_PRESENTATION_RELATIONSHIPS {
            return Err(PresentationFragmentError::TooManyRelationships);
        }
        if self.composition.len() > crate::MAX_PRESENTATION_COMPOSITION_RELATIONS {
            return Err(PresentationFragmentError::TooManyCompositionRelations);
        }
        if self.properties.len() > MAX_PRESENTATION_PROPERTIES {
            return Err(PresentationFragmentError::TooManyProperties);
        }
        if self.text.len() > MAX_PRESENTATION_TEXT_ITEMS {
            return Err(PresentationFragmentError::TooManyTextItems);
        }
        if self.actions.len() > crate::MAX_PRESENTATION_ACTIONS {
            return Err(PresentationFragmentError::TooManyActions);
        }
        if self.disclosures.len() > crate::MAX_PRESENTATION_DISCLOSURES {
            return Err(PresentationFragmentError::TooManyDisclosures);
        }
        if self.temporal_references.len() > crate::MAX_TEMPORAL_REFERENCES {
            return Err(PresentationFragmentError::TooManyTemporalReferences);
        }
        if self.temporal_facts.len() > crate::MAX_PRESENTATION_TEMPORAL_FACTS {
            return Err(PresentationFragmentError::TooManyTemporalFacts);
        }
        if self
            .basis
            .required_interaction_context
            .as_ref()
            .is_some_and(|identity| {
                identity.is_empty() || identity.len() > crate::MAX_PRESENTATION_ID_BYTES
            })
        {
            return Err(PresentationFragmentError::InvalidContextRequirement);
        }
        for (index, subject) in self.subjects.iter().enumerate() {
            if !valid_identity(&subject.identity) {
                return Err(PresentationFragmentError::InvalidIdentity);
            }
            if self.subjects[index + 1..]
                .iter()
                .any(|candidate| candidate.identity == subject.identity)
            {
                return Err(PresentationFragmentError::DuplicateSubject);
            }
        }
        for (index, action) in self.actions.iter().enumerate() {
            if !valid_identity(&action.identity) {
                return Err(PresentationFragmentError::InvalidIdentity);
            }
            if self.actions[index + 1..]
                .iter()
                .any(|candidate| candidate.identity == action.identity)
            {
                return Err(PresentationFragmentError::DuplicateAction);
            }
        }
        for (index, relation) in self.composition.iter().enumerate() {
            if !valid_identity(&relation.identity)
                || !valid_identity(&relation.source)
                || !valid_identity(&relation.target)
            {
                return Err(PresentationFragmentError::InvalidIdentity);
            }
            if self.composition[index + 1..]
                .iter()
                .any(|candidate| candidate.identity == relation.identity)
            {
                return Err(PresentationFragmentError::DuplicateCompositionRelation);
            }
        }
        Ok(())
    }
}

fn valid_identity(value: &str) -> bool {
    !value.is_empty() && value.len() <= crate::MAX_PRESENTATION_ID_BYTES
}
