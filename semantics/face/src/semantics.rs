//! Bounded semantic actions and progressive disclosure for a Face.

use alloc::{string::String, vec::Vec};
use conduit_core::{kind_id, CheckedValueContract, ValueConstraint};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::face::{validate_id, validate_text};
use crate::identity::hash_string;
use crate::{Face, FaceDisclosureLevel, FaceError};

pub const MAX_FACE_ACTIONS: usize = 1_024;
pub const MAX_FACE_DISCLOSURES: usize = 1_024;
pub const MAX_FACE_REASON_BYTES: usize = 1_024;

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

/// One ordinary Conduit intent offered by a Face.
///
/// This record describes an action. It neither grants authority nor invokes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FaceAction {
    pub identity: String,
    pub intent: String,
    pub target: String,
    /// The one ordinary bounded human name for this semantic action.
    pub name: String,
    /// The complete ordered contract committed by one interaction occurrence.
    pub arguments: Vec<FaceActionArgument>,
    pub disclosure: FaceDisclosureLevel,
    pub availability: FaceActionAvailability,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FaceActionAvailability {
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

impl FaceActionAvailability {
    pub fn is_available(&self) -> bool {
        matches!(self, Self::Available)
    }
}

/// The disclosure level assigned to one exact Face subject.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FaceDisclosure {
    pub subject: String,
    pub level: FaceDisclosureLevel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FaceActionRefusal {
    StaleRevision,
    UnknownAction,
    Unavailable { reason_code: String },
    Refused { reason_code: String },
}

impl Face {
    /// Resolve an action description at an exact revision without invoking it.
    pub fn resolve_action(
        &self,
        revision: u64,
        identity: &str,
    ) -> Result<&FaceAction, FaceActionRefusal> {
        if revision != self.revision {
            return Err(FaceActionRefusal::StaleRevision);
        }
        let action = self
            .actions
            .iter()
            .find(|action| action.identity == identity)
            .ok_or(FaceActionRefusal::UnknownAction)?;
        match &action.availability {
            FaceActionAvailability::Available => Ok(action),
            FaceActionAvailability::Unavailable { reason_code, .. } => {
                Err(FaceActionRefusal::Unavailable {
                    reason_code: reason_code.clone(),
                })
            }
            FaceActionAvailability::Refused { reason_code, .. } => {
                Err(FaceActionRefusal::Refused {
                    reason_code: reason_code.clone(),
                })
            }
        }
    }

    pub(crate) fn validate_semantics(&self) -> Result<(), FaceError> {
        if self.actions.len() > MAX_FACE_ACTIONS {
            return Err(FaceError::TooManyActions);
        }
        if self.disclosures.len() > MAX_FACE_DISCLOSURES {
            return Err(FaceError::TooManyDisclosures);
        }
        for index in 0..self.actions.len() {
            let action = &self.actions[index];
            validate_id(&action.identity)?;
            validate_id(&action.intent)?;
            validate_text(&action.name)?;
            if action.arguments.len() > crate::MAX_FACE_ACTION_ARGUMENTS {
                return Err(FaceError::TooManyInputs);
            }
            for (argument_index, argument) in action.arguments.iter().enumerate() {
                validate_id(&argument.name)?;
                validate_text(&argument.value_name)?;
                if argument.contract.maximum_bytes > crate::MAX_FACE_ACTION_ARGUMENT_BYTES
                    || argument.contract.validate_definition().is_err()
                {
                    return Err(FaceError::InvalidInputContract);
                }
                if action.arguments[argument_index + 1..]
                    .iter()
                    .any(|candidate| candidate.name == argument.name)
                {
                    return Err(FaceError::DuplicateInput);
                }
            }
            if !self.has_subject(&action.target) {
                return Err(FaceError::UnknownActionTarget);
            }
            if self.actions[index + 1..]
                .iter()
                .any(|candidate| candidate.identity == action.identity)
            {
                return Err(FaceError::DuplicateAction);
            }
            if let FaceActionAvailability::Unavailable {
                reason_code,
                explanation,
            }
            | FaceActionAvailability::Refused {
                reason_code,
                explanation,
            } = &action.availability
            {
                validate_id(reason_code)?;
                if explanation.len() > MAX_FACE_REASON_BYTES {
                    return Err(FaceError::ReasonTooLong);
                }
                validate_text(explanation)?;
            }
        }
        for index in 0..self.disclosures.len() {
            let disclosure = &self.disclosures[index];
            if !self.has_subject(&disclosure.subject) {
                return Err(FaceError::UnknownDisclosureSubject);
            }
            if self.disclosures[index + 1..]
                .iter()
                .any(|candidate| candidate.subject == disclosure.subject)
            {
                return Err(FaceError::DuplicateDisclosure);
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
                                + argument.contract.identity_byte_len()
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
                crate::identity::hash_contract(digest, &argument.contract);
            }
            digest.update([action.disclosure as u8]);
            match &action.availability {
                FaceActionAvailability::Available => digest.update([0]),
                FaceActionAvailability::Unavailable {
                    reason_code,
                    explanation,
                } => {
                    digest.update([1]);
                    hash_string(digest, reason_code);
                    hash_string(digest, explanation);
                }
                FaceActionAvailability::Refused {
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

fn availability_len(value: &FaceActionAvailability) -> usize {
    match value {
        FaceActionAvailability::Available => 1,
        FaceActionAvailability::Unavailable {
            reason_code,
            explanation,
        }
        | FaceActionAvailability::Refused {
            reason_code,
            explanation,
        } => reason_code.len() + explanation.len() + 1,
    }
}
