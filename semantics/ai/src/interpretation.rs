use alloc::{string::String, vec::Vec};
use conduit_core::SignId;
use serde::{Deserialize, Serialize};

use crate::{TemporalReference, TemporalRetrievalIntent};

pub const MAXIMUM_INTERPRETATION_EVIDENCE: usize = 16;
pub const MAXIMUM_INTERPRETATION_TEXT_BYTES: usize = 2_048;
pub const MAXIMUM_INTERPRETATION_IMPLICATIONS: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterpretationEvidence {
    pub sign_id: SignId,
    pub observation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterpretationRequest {
    pub evidence: Vec<InterpretationEvidence>,
    pub context: String,
    pub temporal_reference: TemporalReference,
    pub temporal_intent: Option<TemporalRetrievalIntent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InterpretationProvenance {
    ModelDerived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InterpretationDisposition {
    Interpreted,
    InsufficientEvidence,
    ContradictoryEvidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileReportedConfidence {
    /// A bounded model/profile score, deliberately not named or treated as probability.
    pub score_permille: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelInterpretation {
    pub provenance: InterpretationProvenance,
    pub hypothesis: String,
    pub referenced_evidence: Vec<SignId>,
    pub unresolved_evidence: Vec<SignId>,
    pub confidence: Option<ProfileReportedConfidence>,
    pub implications: Vec<String>,
    pub disposition: InterpretationDisposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InterpretationInvalidity {
    EmptyEvidence,
    TooMuchEvidence,
    EmptySignIdentity,
    DuplicateSignIdentity,
    TextBoundExceeded,
    MissingHypothesis,
    TooManyImplications,
    InvalidConfidence,
    FabricatedEvidenceReference,
    ResolvedEvidenceMarkedUnresolved,
    InvalidTemporalContext,
}

impl InterpretationRequest {
    pub fn validate(&self) -> Result<(), InterpretationInvalidity> {
        if self.evidence.is_empty() {
            return Err(InterpretationInvalidity::EmptyEvidence);
        }
        if self.evidence.len() > MAXIMUM_INTERPRETATION_EVIDENCE {
            return Err(InterpretationInvalidity::TooMuchEvidence);
        }
        if self
            .evidence
            .iter()
            .any(|evidence| evidence.sign_id.as_str().is_empty())
        {
            return Err(InterpretationInvalidity::EmptySignIdentity);
        }
        if self.evidence.iter().enumerate().any(|(index, evidence)| {
            self.evidence[index + 1..]
                .iter()
                .any(|candidate| candidate.sign_id == evidence.sign_id)
        }) {
            return Err(InterpretationInvalidity::DuplicateSignIdentity);
        }
        if self.context.len() > MAXIMUM_INTERPRETATION_TEXT_BYTES
            || self.evidence.iter().any(|evidence| {
                evidence.observation.is_empty()
                    || evidence.observation.len() > MAXIMUM_INTERPRETATION_TEXT_BYTES
            })
        {
            return Err(InterpretationInvalidity::TextBoundExceeded);
        }
        if self.temporal_reference.validate().is_err()
            || self
                .temporal_intent
                .as_ref()
                .is_some_and(|intent| intent.validate().is_err())
        {
            return Err(InterpretationInvalidity::InvalidTemporalContext);
        }
        Ok(())
    }
}

impl ModelInterpretation {
    pub fn validate_against(
        &self,
        request: &InterpretationRequest,
    ) -> Result<(), InterpretationInvalidity> {
        request.validate()?;
        if self.hypothesis.is_empty() {
            return Err(InterpretationInvalidity::MissingHypothesis);
        }
        if self.hypothesis.len() > MAXIMUM_INTERPRETATION_TEXT_BYTES
            || self
                .implications
                .iter()
                .any(|value| value.is_empty() || value.len() > MAXIMUM_INTERPRETATION_TEXT_BYTES)
        {
            return Err(InterpretationInvalidity::TextBoundExceeded);
        }
        if self.implications.len() > MAXIMUM_INTERPRETATION_IMPLICATIONS {
            return Err(InterpretationInvalidity::TooManyImplications);
        }
        if self
            .confidence
            .is_some_and(|value| value.score_permille > 1_000)
        {
            return Err(InterpretationInvalidity::InvalidConfidence);
        }
        if self.referenced_evidence.iter().any(|sign_id| {
            !request
                .evidence
                .iter()
                .any(|evidence| &evidence.sign_id == sign_id)
        }) {
            return Err(InterpretationInvalidity::FabricatedEvidenceReference);
        }
        if self.unresolved_evidence.iter().any(|sign_id| {
            request
                .evidence
                .iter()
                .any(|evidence| &evidence.sign_id == sign_id)
        }) {
            return Err(InterpretationInvalidity::ResolvedEvidenceMarkedUnresolved);
        }
        Ok(())
    }
}
