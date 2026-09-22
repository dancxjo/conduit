//! Finite optional purpose and deterministic Fulfillment readiness.
//!
//! These are semantic facts for a body application. They grant no lifecycle
//! authority, contain no Presenter policy, and name no realizing Host.

use alloc::{string::String, vec::Vec};
use conduit_core::SignId;
use serde::{Deserialize, Serialize};

pub const MAX_PURPOSE_ID_BYTES: usize = 128;
pub const MAX_PURPOSE_SUMMARY_BYTES: usize = 512;
pub const MAX_PURPOSE_OBLIGATIONS: usize = 32;
pub const MAX_OBLIGATION_EVIDENCE_SIGNS: usize = 8;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PurposeCompletionPolicy {
    ExplicitFulfillmentReadiness,
    NoFulfillmentCondition,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PurposeObligationState {
    Pending,
    Satisfied { evidence_sign_ids: Vec<SignId> },
    RepairRequired { failure_sign_id: SignId },
    CompletionEvidenceMissing,
    Uncertain { evidence_sign_ids: Vec<SignId> },
    Disputed { evidence_sign_ids: Vec<SignId> },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PurposeObligation {
    pub obligation_id: String,
    pub summary: String,
    pub state: PurposeObligationState,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PurposeState {
    pub purpose_id: String,
    pub revision: u64,
    pub summary: String,
    pub completion_policy: PurposeCompletionPolicy,
    pub obligations: Vec<PurposeObligation>,
}

impl PurposeState {
    pub fn validate(&self) -> Result<(), PurposeRefusal> {
        validate_identity(&self.purpose_id)?;
        validate_summary(&self.summary)?;
        if self.revision == 0 {
            return Err(PurposeRefusal::InvalidRevision);
        }
        if self.obligations.is_empty() || self.obligations.len() > MAX_PURPOSE_OBLIGATIONS {
            return Err(PurposeRefusal::ObligationBound);
        }
        for (index, obligation) in self.obligations.iter().enumerate() {
            validate_identity(&obligation.obligation_id)?;
            validate_summary(&obligation.summary)?;
            if self.obligations[..index]
                .iter()
                .any(|prior| prior.obligation_id == obligation.obligation_id)
            {
                return Err(PurposeRefusal::DuplicateObligation);
            }
            validate_obligation_state(&obligation.state)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FulfillmentReadinessReasonKind {
    Unfinished,
    RepairRequired,
    CompletionEvidenceMissing,
    CompletionUncertain,
    CompletionDisputed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FulfillmentReadinessReason {
    pub obligation_id: String,
    pub kind: FulfillmentReadinessReasonKind,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FulfillmentReadiness {
    Ready {
        purpose_id: String,
        purpose_revision: u64,
    },
    NotReady {
        purpose_id: String,
        purpose_revision: u64,
        reasons: Vec<FulfillmentReadinessReason>,
    },
    Unavailable {
        purpose_id: String,
        purpose_revision: u64,
    },
}

pub fn derive_fulfillment_readiness(
    purpose: &PurposeState,
) -> Result<FulfillmentReadiness, PurposeRefusal> {
    purpose.validate()?;
    if purpose.completion_policy == PurposeCompletionPolicy::NoFulfillmentCondition {
        return Ok(FulfillmentReadiness::Unavailable {
            purpose_id: purpose.purpose_id.clone(),
            purpose_revision: purpose.revision,
        });
    }
    let reasons = purpose
        .obligations
        .iter()
        .filter_map(|obligation| {
            let kind = match obligation.state {
                PurposeObligationState::Pending => FulfillmentReadinessReasonKind::Unfinished,
                PurposeObligationState::RepairRequired { .. } => {
                    FulfillmentReadinessReasonKind::RepairRequired
                }
                PurposeObligationState::CompletionEvidenceMissing => {
                    FulfillmentReadinessReasonKind::CompletionEvidenceMissing
                }
                PurposeObligationState::Uncertain { .. } => {
                    FulfillmentReadinessReasonKind::CompletionUncertain
                }
                PurposeObligationState::Disputed { .. } => {
                    FulfillmentReadinessReasonKind::CompletionDisputed
                }
                PurposeObligationState::Satisfied { .. } => return None,
            };
            Some(FulfillmentReadinessReason {
                obligation_id: obligation.obligation_id.clone(),
                kind,
            })
        })
        .collect::<Vec<_>>();
    if reasons.is_empty() {
        Ok(FulfillmentReadiness::Ready {
            purpose_id: purpose.purpose_id.clone(),
            purpose_revision: purpose.revision,
        })
    } else {
        Ok(FulfillmentReadiness::NotReady {
            purpose_id: purpose.purpose_id.clone(),
            purpose_revision: purpose.revision,
            reasons,
        })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PurposeRefusal {
    EmptyIdentity,
    IdentityBound,
    InvalidRevision,
    EmptySummary,
    SummaryBound,
    ObligationBound,
    DuplicateObligation,
    EvidenceBound,
    MissingEvidence,
    DuplicateEvidence,
}

fn validate_identity(value: &str) -> Result<(), PurposeRefusal> {
    if value.is_empty() {
        return Err(PurposeRefusal::EmptyIdentity);
    }
    if value.len() > MAX_PURPOSE_ID_BYTES {
        return Err(PurposeRefusal::IdentityBound);
    }
    Ok(())
}

fn validate_summary(value: &str) -> Result<(), PurposeRefusal> {
    if value.is_empty() {
        return Err(PurposeRefusal::EmptySummary);
    }
    if value.len() > MAX_PURPOSE_SUMMARY_BYTES {
        return Err(PurposeRefusal::SummaryBound);
    }
    Ok(())
}

fn validate_obligation_state(state: &PurposeObligationState) -> Result<(), PurposeRefusal> {
    let evidence = match state {
        PurposeObligationState::Satisfied { evidence_sign_ids }
        | PurposeObligationState::Uncertain { evidence_sign_ids }
        | PurposeObligationState::Disputed { evidence_sign_ids } => evidence_sign_ids,
        PurposeObligationState::RepairRequired { failure_sign_id } => {
            return validate_identity(failure_sign_id.as_str());
        }
        PurposeObligationState::Pending | PurposeObligationState::CompletionEvidenceMissing => {
            return Ok(());
        }
    };
    if evidence.is_empty() {
        return Err(PurposeRefusal::MissingEvidence);
    }
    if evidence.len() > MAX_OBLIGATION_EVIDENCE_SIGNS {
        return Err(PurposeRefusal::EvidenceBound);
    }
    for (index, sign) in evidence.iter().enumerate() {
        validate_identity(sign.as_str())?;
        if evidence[..index].contains(sign) {
            return Err(PurposeRefusal::DuplicateEvidence);
        }
    }
    if matches!(state, PurposeObligationState::Disputed { .. }) && evidence.len() < 2 {
        return Err(PurposeRefusal::MissingEvidence);
    }
    Ok(())
}
