//! Bounded natural-language interpretation at the ordinary interaction boundary.
//!
//! An interpreter may propose an exact interaction or report that it could not.
//! It never queues, executes, authorizes, or records semantic success.

use alloc::{string::String, vec::Vec};
use serde::{Deserialize, Serialize};

use crate::{
    presentation::validate_id, FaceInteraction, FaceInteractionArgument, FaceInteractionRefusal,
    MaskShow, Presentation,
};

pub const GENERATIVE_INTERACTION_PROPOSAL_KIND: &str =
    "conduit.presentation/generative-interaction-proposal@1";
pub const MAX_GENERATIVE_INTERACTION_REASON_BYTES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProposedFaceInteraction {
    pub action_id: String,
    pub target: String,
    pub arguments: Vec<FaceInteractionArgument>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GenerativeInteractionDisposition {
    Proposed(ProposedFaceInteraction),
    ClarificationRequired { reason_code: String },
    Refused { reason_code: String },
    Failed { reason_code: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenerativeInteractionProposal {
    pub proposal_identity: String,
    pub interpretation_run_identity: String,
    pub source_face_identity: String,
    pub source_face_revision: u64,
    pub show_identity: String,
    pub interpreter_implementation_identity: String,
    pub provider_identity: String,
    pub model_identity: String,
    pub disposition: GenerativeInteractionDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedGenerativeInteraction {
    Interaction(FaceInteraction),
    ClarificationRequired { reason_code: String },
    Refused { reason_code: String },
    Failed { reason_code: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenerativeInteractionRefusal {
    InvalidIdentity,
    InvalidReason,
    StaleFace,
    StaleShow,
    Interaction(FaceInteractionRefusal),
}

impl GenerativeInteractionProposal {
    /// Resolves a model-produced candidate through the ordinary Mask interaction
    /// contract. Returning `Interaction` does not enqueue or execute it.
    pub fn resolve(
        &self,
        face: &Presentation,
        show: &MaskShow,
        sequence: u64,
    ) -> Result<ResolvedGenerativeInteraction, GenerativeInteractionRefusal> {
        self.validate_identities()?;
        if self.source_face_identity != face.identity.as_str()
            || self.source_face_revision != face.revision
        {
            return Err(GenerativeInteractionRefusal::StaleFace);
        }
        if self.show_identity != show.show_id.as_str() {
            return Err(GenerativeInteractionRefusal::StaleShow);
        }
        match &self.disposition {
            GenerativeInteractionDisposition::Proposed(proposal) => {
                for value in [&proposal.action_id, &proposal.target] {
                    validate_id(value)
                        .map_err(|_| GenerativeInteractionRefusal::InvalidIdentity)?;
                }
                FaceInteraction::new(
                    face,
                    show,
                    &proposal.action_id,
                    &proposal.target,
                    proposal.arguments.clone(),
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
            &self.source_face_identity,
            &self.show_identity,
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
