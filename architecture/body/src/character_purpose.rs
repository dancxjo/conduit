//! Finite Body character, purpose, and deterministic Fulfillment readiness.
//!
//! These are semantic facts for a Body/application. They grant no lifecycle
//! authority, contain no Presenter prompt, and name no realizing Host.

use alloc::{string::String, vec::Vec};
use conduit_core::SignId;
use serde::{Deserialize, Serialize};

pub const MAX_CHARACTER_PURPOSE_ID_BYTES: usize = 128;
pub const MAX_CHARACTER_PURPOSE_SUMMARY_BYTES: usize = 512;
pub const MAX_PURPOSE_OBLIGATIONS: usize = 32;
pub const MAX_OBLIGATION_EVIDENCE_SIGNS: usize = 8;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkOrientation {
    UsefulCompletion,
    ContinuingService,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum UncertaintyOrientation {
    InvestigateBeforeClaimingCompletion,
    PreserveAsUnresolved,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FaultOrientation {
    RepairWhilePurposeRemains,
    ReportWithoutInferringCompletion,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FulfillmentOrientation {
    WelcomeWhenExactlyReady,
    NoDeclaredOrientation,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CharacterProfile {
    pub profile_id: String,
    pub revision: u64,
    pub work: WorkOrientation,
    pub uncertainty: UncertaintyOrientation,
    pub faults: FaultOrientation,
    pub fulfillment: FulfillmentOrientation,
}

impl CharacterProfile {
    pub fn purposeful_completion(profile_id: String, revision: u64) -> Self {
        Self {
            profile_id,
            revision,
            work: WorkOrientation::UsefulCompletion,
            uncertainty: UncertaintyOrientation::InvestigateBeforeClaimingCompletion,
            faults: FaultOrientation::RepairWhilePurposeRemains,
            fulfillment: FulfillmentOrientation::WelcomeWhenExactlyReady,
        }
    }

    pub fn validate(&self) -> Result<(), CharacterPurposeRefusal> {
        validate_identity(&self.profile_id)?;
        if self.revision == 0 {
            return Err(CharacterPurposeRefusal::InvalidRevision);
        }
        Ok(())
    }
}

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
    pub fn validate(&self) -> Result<(), CharacterPurposeRefusal> {
        validate_identity(&self.purpose_id)?;
        validate_summary(&self.summary)?;
        if self.revision == 0 {
            return Err(CharacterPurposeRefusal::InvalidRevision);
        }
        if self.obligations.is_empty() || self.obligations.len() > MAX_PURPOSE_OBLIGATIONS {
            return Err(CharacterPurposeRefusal::ObligationBound);
        }
        for (index, obligation) in self.obligations.iter().enumerate() {
            validate_identity(&obligation.obligation_id)?;
            validate_summary(&obligation.summary)?;
            if self.obligations[..index]
                .iter()
                .any(|prior| prior.obligation_id == obligation.obligation_id)
            {
                return Err(CharacterPurposeRefusal::DuplicateObligation);
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
) -> Result<FulfillmentReadiness, CharacterPurposeRefusal> {
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
pub enum CharacterOrientationCue {
    ContinueUsefulWork,
    RepairBeforeCompletion,
    InvestigateCompletion,
    PreserveDisagreement,
    WelcomeAppropriateFulfillment,
    ContinueWithoutFulfillmentClaim,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CharacterContext {
    pub character_profile_id: String,
    pub character_revision: u64,
    pub purpose_id: String,
    pub purpose_revision: u64,
    pub readiness: FulfillmentReadiness,
    pub cues: Vec<CharacterOrientationCue>,
}

pub fn derive_character_context(
    character: &CharacterProfile,
    purpose: &PurposeState,
) -> Result<CharacterContext, CharacterPurposeRefusal> {
    character.validate()?;
    let readiness = derive_fulfillment_readiness(purpose)?;
    let mut cues = Vec::new();
    match &readiness {
        FulfillmentReadiness::Ready { .. }
            if character.fulfillment == FulfillmentOrientation::WelcomeWhenExactlyReady =>
        {
            cues.push(CharacterOrientationCue::WelcomeAppropriateFulfillment);
        }
        FulfillmentReadiness::Ready { .. } | FulfillmentReadiness::Unavailable { .. } => {
            cues.push(CharacterOrientationCue::ContinueWithoutFulfillmentClaim);
        }
        FulfillmentReadiness::NotReady { reasons, .. } => {
            for reason in reasons {
                let cue = match reason.kind {
                    FulfillmentReadinessReasonKind::Unfinished => {
                        CharacterOrientationCue::ContinueUsefulWork
                    }
                    FulfillmentReadinessReasonKind::RepairRequired => {
                        CharacterOrientationCue::RepairBeforeCompletion
                    }
                    FulfillmentReadinessReasonKind::CompletionEvidenceMissing
                    | FulfillmentReadinessReasonKind::CompletionUncertain => {
                        CharacterOrientationCue::InvestigateCompletion
                    }
                    FulfillmentReadinessReasonKind::CompletionDisputed => {
                        CharacterOrientationCue::PreserveDisagreement
                    }
                };
                if !cues.contains(&cue) {
                    cues.push(cue);
                }
            }
        }
    }
    Ok(CharacterContext {
        character_profile_id: character.profile_id.clone(),
        character_revision: character.revision,
        purpose_id: purpose.purpose_id.clone(),
        purpose_revision: purpose.revision,
        readiness,
        cues,
    })
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CharacterPurposeRefusal {
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

fn validate_identity(value: &str) -> Result<(), CharacterPurposeRefusal> {
    if value.is_empty() {
        return Err(CharacterPurposeRefusal::EmptyIdentity);
    }
    if value.len() > MAX_CHARACTER_PURPOSE_ID_BYTES {
        return Err(CharacterPurposeRefusal::IdentityBound);
    }
    Ok(())
}

fn validate_summary(value: &str) -> Result<(), CharacterPurposeRefusal> {
    if value.is_empty() {
        return Err(CharacterPurposeRefusal::EmptySummary);
    }
    if value.len() > MAX_CHARACTER_PURPOSE_SUMMARY_BYTES {
        return Err(CharacterPurposeRefusal::SummaryBound);
    }
    Ok(())
}

fn validate_obligation_state(
    state: &PurposeObligationState,
) -> Result<(), CharacterPurposeRefusal> {
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
        return Err(CharacterPurposeRefusal::MissingEvidence);
    }
    if evidence.len() > MAX_OBLIGATION_EVIDENCE_SIGNS {
        return Err(CharacterPurposeRefusal::EvidenceBound);
    }
    for (index, sign) in evidence.iter().enumerate() {
        validate_identity(sign.as_str())?;
        if evidence[..index].contains(sign) {
            return Err(CharacterPurposeRefusal::DuplicateEvidence);
        }
    }
    if matches!(state, PurposeObligationState::Disputed { .. }) && evidence.len() < 2 {
        return Err(CharacterPurposeRefusal::MissingEvidence);
    }
    Ok(())
}
