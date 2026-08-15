//! Bounded semantic actions and progressive disclosure for a Presentation.

use alloc::string::String;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::identity::hash_string;
use crate::presentation::{validate_id, validate_text};
use crate::{Presentation, PresentationError};

pub const MAX_PRESENTATION_ACTIONS: usize = 1_024;
pub const MAX_PRESENTATION_DISCLOSURES: usize = 1_024;
pub const MAX_PRESENTATION_REASON_BYTES: usize = 1_024;

/// One ordinary Conduit intent offered by a Presentation.
///
/// This record describes an action. It neither grants authority nor invokes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresentationAction {
    pub identity: String,
    pub intent: String,
    pub target: String,
    pub label: String,
    pub disclosure: PresentationDisclosureLevel,
    pub availability: PresentationActionAvailability,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PresentationActionAvailability {
    Available,
    Unavailable {
        reason_code: String,
        explanation: String,
    },
    Refused {
        reason_code: String,
        explanation: String,
    },
}

impl PresentationActionAvailability {
    pub fn is_available(&self) -> bool {
        matches!(self, Self::Available)
    }
}

/// Semantic information priority, independent of visual position or visibility.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PresentationDisclosureLevel {
    Primary,
    CurrentAction,
    Context,
    SelectedDetail,
    ExactProvenance,
}

/// The disclosure level assigned to one exact Presentation subject.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresentationDisclosure {
    pub subject: String,
    pub level: PresentationDisclosureLevel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresentationActionRefusal {
    StaleRevision,
    UnknownAction,
    Unavailable { reason_code: String },
    Refused { reason_code: String },
}

impl Presentation {
    pub(crate) fn validate_semantics(&self) -> Result<(), PresentationError> {
        if self.actions.len() > MAX_PRESENTATION_ACTIONS {
            return Err(PresentationError::TooManyActions);
        }
        if self.disclosures.len() > MAX_PRESENTATION_DISCLOSURES {
            return Err(PresentationError::TooManyDisclosures);
        }
        for index in 0..self.actions.len() {
            let action = &self.actions[index];
            validate_id(&action.identity)?;
            validate_id(&action.intent)?;
            validate_text(&action.label)?;
            if !self.has_subject(&action.target) {
                return Err(PresentationError::UnknownActionTarget);
            }
            if self.actions[index + 1..]
                .iter()
                .any(|candidate| candidate.identity == action.identity)
            {
                return Err(PresentationError::DuplicateAction);
            }
            if let PresentationActionAvailability::Unavailable {
                reason_code,
                explanation,
            }
            | PresentationActionAvailability::Refused {
                reason_code,
                explanation,
            } = &action.availability
            {
                validate_id(reason_code)?;
                if explanation.len() > MAX_PRESENTATION_REASON_BYTES {
                    return Err(PresentationError::ReasonTooLong);
                }
                validate_text(explanation)?;
            }
        }
        for index in 0..self.disclosures.len() {
            let disclosure = &self.disclosures[index];
            if !self.has_subject(&disclosure.subject) {
                return Err(PresentationError::UnknownDisclosureSubject);
            }
            if self.disclosures[index + 1..]
                .iter()
                .any(|candidate| candidate.subject == disclosure.subject)
            {
                return Err(PresentationError::DuplicateDisclosure);
            }
        }
        Ok(())
    }

    pub(crate) fn semantics_len(&self) -> usize {
        self.actions
            .iter()
            .map(|action| {
                action.identity.len()
                    + action.intent.len()
                    + action.target.len()
                    + action.label.len()
                    + 1
                    + availability_len(&action.availability)
            })
            .sum::<usize>()
            + self
                .disclosures
                .iter()
                .map(|disclosure| disclosure.subject.len() + 1)
                .sum::<usize>()
    }

    pub(crate) fn hash_semantics(&self, digest: &mut Sha256) {
        for action in &self.actions {
            hash_string(digest, &action.identity);
            hash_string(digest, &action.intent);
            hash_string(digest, &action.target);
            hash_string(digest, &action.label);
            digest.update([action.disclosure as u8]);
            match &action.availability {
                PresentationActionAvailability::Available => digest.update([0]),
                PresentationActionAvailability::Unavailable {
                    reason_code,
                    explanation,
                } => {
                    digest.update([1]);
                    hash_string(digest, reason_code);
                    hash_string(digest, explanation);
                }
                PresentationActionAvailability::Refused {
                    reason_code,
                    explanation,
                } => {
                    digest.update([2]);
                    hash_string(digest, reason_code);
                    hash_string(digest, explanation);
                }
            }
        }
        for disclosure in &self.disclosures {
            hash_string(digest, &disclosure.subject);
            digest.update([disclosure.level as u8]);
        }
    }
}

fn availability_len(value: &PresentationActionAvailability) -> usize {
    match value {
        PresentationActionAvailability::Available => 1,
        PresentationActionAvailability::Unavailable {
            reason_code,
            explanation,
        }
        | PresentationActionAvailability::Refused {
            reason_code,
            explanation,
        } => reason_code.len() + explanation.len() + 1,
    }
}
