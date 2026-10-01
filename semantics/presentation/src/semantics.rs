//! Bounded semantic actions and progressive disclosure for a Presentation.

use alloc::{string::String, vec::Vec};
use conduit_core::{kind_id, CheckedValueContract, ValueConstraint};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::identity::hash_string;
use crate::presentation::{validate_id, validate_text};
use crate::{Presentation, PresentationDisclosureLevel, PresentationError};

pub const MAX_PRESENTATION_ACTIONS: usize = 1_024;
pub const MAX_PRESENTATION_DISCLOSURES: usize = 1_024;
pub const MAX_PRESENTATION_REASON_BYTES: usize = 1_024;

/// One exact, finite named value committed atomically with its owning action.
///
/// This is semantic participation truth, not a field, widget, or Mask-local
/// edit buffer. Optionality belongs in the checked value contract itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FaceActionArgument {
    pub name: String,
    pub value_name: String,
    pub contract: CheckedValueContract,
}

impl FaceActionArgument {
    pub fn text(
        name: String,
        value_name: String,
        minimum_bytes: u32,
        maximum_bytes: u32,
    ) -> Result<Self, conduit_core::ConstraintDefinitionError> {
        Ok(Self {
            name,
            value_name,
            contract: CheckedValueContract::new(
                kind_id(crate::UTF8_TEXT_VALUE_KIND),
                maximum_bytes,
                alloc::vec![ValueConstraint::ByteLength {
                    minimum: minimum_bytes,
                    maximum: maximum_bytes,
                }],
            )?,
        })
    }
}

/// One ordinary Conduit intent offered by a Presentation.
///
/// This record describes an action. It neither grants authority nor invokes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresentationAction {
    pub identity: String,
    pub intent: String,
    pub target: String,
    /// The one ordinary bounded human name for this semantic action.
    pub name: String,
    /// The complete ordered contract committed by one interaction occurrence.
    pub arguments: Vec<FaceActionArgument>,
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
    /// Resolve an action description at an exact revision without invoking it.
    pub fn resolve_action(
        &self,
        revision: u64,
        identity: &str,
    ) -> Result<&PresentationAction, PresentationActionRefusal> {
        if revision != self.revision {
            return Err(PresentationActionRefusal::StaleRevision);
        }
        let action = self
            .actions
            .iter()
            .find(|action| action.identity == identity)
            .ok_or(PresentationActionRefusal::UnknownAction)?;
        match &action.availability {
            PresentationActionAvailability::Available => Ok(action),
            PresentationActionAvailability::Unavailable { reason_code, .. } => {
                Err(PresentationActionRefusal::Unavailable {
                    reason_code: reason_code.clone(),
                })
            }
            PresentationActionAvailability::Refused { reason_code, .. } => {
                Err(PresentationActionRefusal::Refused {
                    reason_code: reason_code.clone(),
                })
            }
        }
    }

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
            validate_text(&action.name)?;
            if action.arguments.len() > crate::MAX_FACE_ACTION_ARGUMENTS {
                return Err(PresentationError::TooManyInputs);
            }
            for (argument_index, argument) in action.arguments.iter().enumerate() {
                validate_id(&argument.name)?;
                validate_text(&argument.value_name)?;
                if argument.contract.maximum_bytes > crate::MAX_FACE_ACTION_ARGUMENT_BYTES
                    || argument.contract.validate_definition().is_err()
                {
                    return Err(PresentationError::InvalidInputContract);
                }
                if action.arguments[argument_index + 1..]
                    .iter()
                    .any(|candidate| candidate.name == argument.name)
                {
                    return Err(PresentationError::DuplicateInput);
                }
            }
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
                    + action.name.len()
                    + action
                        .arguments
                        .iter()
                        .map(|argument| {
                            argument.name.len()
                                + argument.value_name.len()
                                + argument.contract.identity_bytes().len()
                        })
                        .sum::<usize>()
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
            hash_string(digest, &action.name);
            digest.update((action.arguments.len() as u32).to_le_bytes());
            for argument in &action.arguments {
                hash_string(digest, &argument.name);
                hash_string(digest, &argument.value_name);
                let contract = argument.contract.identity_bytes();
                digest.update((contract.len() as u32).to_le_bytes());
                digest.update(contract);
            }
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
