//! Bounded natural-language interpretation at the ordinary interaction boundary.
//!
//! An interpreter may propose an exact interaction or report that it could not.
//! It never queues, executes, authorizes, or records semantic success.

use alloc::{string::String, vec::Vec};
use serde::{Deserialize, Serialize};

use crate::{
    presentation::validate_id, Manifestation, Presentation, PresentationInteraction,
    PresentationInteractionRefusal,
};

pub const GENERATIVE_INTERACTION_PROPOSAL_KIND: &str =
    "conduit.presentation/generative-interaction-proposal@1";
pub const MAX_GENERATIVE_INTERACTION_REASON_BYTES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProposedPresentationInteraction {
    pub input_id: String,
    pub action_id: String,
    pub target: String,
    pub value_kind: String,
    pub value: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GenerativeInteractionDisposition {
    Proposed(ProposedPresentationInteraction),
    ClarificationRequired { reason_code: String },
    Refused { reason_code: String },
    Failed { reason_code: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenerativeInteractionProposal {
    pub proposal_identity: String,
    pub interpretation_run_identity: String,
    pub source_presentation_identity: String,
    pub source_presentation_revision: u64,
    pub manifestation_identity: String,
    pub interpreter_implementation_identity: String,
    pub provider_identity: String,
    pub model_identity: String,
    pub disposition: GenerativeInteractionDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedGenerativeInteraction {
    Interaction(PresentationInteraction),
    ClarificationRequired { reason_code: String },
    Refused { reason_code: String },
    Failed { reason_code: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenerativeInteractionRefusal {
    InvalidIdentity,
    InvalidReason,
    StalePresentation,
    StaleManifestation,
    Interaction(PresentationInteractionRefusal),
}

impl GenerativeInteractionProposal {
    /// Resolves a model-produced candidate through the ordinary Mask interaction
    /// contract. Returning `Interaction` does not enqueue or execute it.
    pub fn resolve(
        &self,
        presentation: &Presentation,
        manifestation: &Manifestation,
        sequence: u64,
    ) -> Result<ResolvedGenerativeInteraction, GenerativeInteractionRefusal> {
        self.validate_identities()?;
        if self.source_presentation_identity != presentation.identity.as_str()
            || self.source_presentation_revision != presentation.revision
        {
            return Err(GenerativeInteractionRefusal::StalePresentation);
        }
        if self.manifestation_identity != manifestation.manifestation_id.as_str() {
            return Err(GenerativeInteractionRefusal::StaleManifestation);
        }
        match &self.disposition {
            GenerativeInteractionDisposition::Proposed(proposal) => {
                for value in [
                    &proposal.input_id,
                    &proposal.action_id,
                    &proposal.target,
                    &proposal.value_kind,
                ] {
                    validate_id(value)
                        .map_err(|_| GenerativeInteractionRefusal::InvalidIdentity)?;
                }
                PresentationInteraction::new(
                    presentation,
                    manifestation,
                    &proposal.input_id,
                    &proposal.action_id,
                    &proposal.target,
                    &proposal.value_kind,
                    &proposal.value,
                    sequence,
                )
                .map(ResolvedGenerativeInteraction::Interaction)
                .map_err(GenerativeInteractionRefusal::Interaction)
            }
            GenerativeInteractionDisposition::ClarificationRequired { reason_code } => {
                validate_reason(reason_code)?;
                Ok(ResolvedGenerativeInteraction::ClarificationRequired {
                    reason_code: reason_code.clone(),
                })
            }
            GenerativeInteractionDisposition::Refused { reason_code } => {
                validate_reason(reason_code)?;
                Ok(ResolvedGenerativeInteraction::Refused {
                    reason_code: reason_code.clone(),
                })
            }
            GenerativeInteractionDisposition::Failed { reason_code } => {
                validate_reason(reason_code)?;
                Ok(ResolvedGenerativeInteraction::Failed {
                    reason_code: reason_code.clone(),
                })
            }
        }
    }

    fn validate_identities(&self) -> Result<(), GenerativeInteractionRefusal> {
        for value in [
            &self.proposal_identity,
            &self.interpretation_run_identity,
            &self.source_presentation_identity,
            &self.manifestation_identity,
            &self.interpreter_implementation_identity,
            &self.provider_identity,
            &self.model_identity,
        ] {
            validate_id(value).map_err(|_| GenerativeInteractionRefusal::InvalidIdentity)?;
        }
        Ok(())
    }
}

fn validate_reason(reason: &str) -> Result<(), GenerativeInteractionRefusal> {
    if reason.is_empty() || reason.len() > MAX_GENERATIVE_INTERACTION_REASON_BYTES {
        return Err(GenerativeInteractionRefusal::InvalidReason);
    }
    validate_id(reason).map_err(|_| GenerativeInteractionRefusal::InvalidReason)
}
