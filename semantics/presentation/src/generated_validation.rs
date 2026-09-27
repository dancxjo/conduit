//! Trusted retention boundary for untrusted semantic-validator assessments.

use alloc::{boxed::Box, string::String, vec::Vec};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    generated_correlation::{validate_correlation, validate_generated_candidate},
    GeneratedManifestation, GeneratedManifestationCandidate, GeneratedSemanticCorrelation,
    GeneratedValidationDisposition, GeneratedValidationError, GeneratedValidationReceipt,
    GenerativePresenterRequest,
};

pub const MAX_ACCEPTED_GENERATED_CORRELATIONS: usize = 128;
pub const MAX_VALIDATION_REFUSAL_CODE_BYTES: usize = 128;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GeneratedValidationOutcome {
    Accepted(Box<GeneratedManifestation>),
    Terminal(Box<GeneratedValidationReceipt>),
}

/// Untrusted output decoded from a validator implementation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedValidatorAssessment {
    pub assessment_identity: String,
    pub candidate_digest: String,
    pub source_presentation_identity: String,
    pub source_presentation_revision: u64,
    pub mask_identity: String,
    pub mask_contract_revision: String,
    pub disposition: GeneratedValidationDisposition,
    pub refusal_code: Option<String>,
    pub accepted_correlations: Vec<GeneratedSemanticCorrelation>,
}

/// One planner-selected validator realization. Public wire data cannot create
/// this token; the future planned validator Back will receive it internally.
#[allow(dead_code)]
pub struct GeneratedValidationSession {
    validator_implementation_identity: String,
    validator_back_identity: String,
    mask_identity: String,
    mask_contract_revision: String,
}

#[allow(dead_code)]
impl GeneratedValidationSession {
    pub(crate) fn selected(
        validator_implementation_identity: String,
        validator_back_identity: String,
        mask_identity: String,
        mask_contract_revision: String,
    ) -> Result<Self, GeneratedValidationError> {
        for value in [
            &validator_implementation_identity,
            &validator_back_identity,
            &mask_identity,
            &mask_contract_revision,
        ] {
            crate::generative_presenter::validate_identity(value)
                .map_err(|_| GeneratedValidationError::InvalidValidatorIdentity)?;
        }
        Ok(Self {
            validator_implementation_identity,
            validator_back_identity,
            mask_identity,
            mask_contract_revision,
        })
    }

    pub(crate) fn retain(
        self,
        request: &GenerativePresenterRequest,
        candidate: GeneratedManifestationCandidate,
        assessment: GeneratedValidatorAssessment,
    ) -> Result<GeneratedValidationOutcome, GeneratedValidationError> {
        validate_generated_candidate(request, &candidate)
            .map_err(GeneratedValidationError::InvalidCandidate)?;
        crate::generative_presenter::validate_identity(&assessment.assessment_identity)
            .map_err(|_| GeneratedValidationError::InvalidAssessmentIdentity)?;
        if assessment
            .refusal_code
            .as_ref()
            .is_some_and(|code| code.is_empty() || code.len() > MAX_VALIDATION_REFUSAL_CODE_BYTES)
        {
            return Err(GeneratedValidationError::InvalidAssessmentContract);
        }
        if assessment.candidate_digest != candidate.digest() {
            return Err(GeneratedValidationError::CandidateDigestMismatch);
        }
        if assessment.source_presentation_identity != candidate.source_presentation_identity
            || assessment.source_presentation_revision != candidate.source_presentation_revision
        {
            return Err(GeneratedValidationError::PresentationMismatch);
        }
        if assessment.mask_identity != self.mask_identity
            || assessment.mask_contract_revision != self.mask_contract_revision
            || candidate.mask_contract_revision != self.mask_contract_revision
        {
            return Err(GeneratedValidationError::MaskMismatch);
        }
        let accepted = assessment.disposition == GeneratedValidationDisposition::Accepted;
        if !accepted {
            if assessment.refusal_code.is_none() || !assessment.accepted_correlations.is_empty() {
                return Err(GeneratedValidationError::InvalidAssessmentContract);
            }
        } else if assessment.refusal_code.is_some() {
            return Err(GeneratedValidationError::RefusalContract);
        }
        if accepted
            && (assessment.accepted_correlations.is_empty()
                || assessment.accepted_correlations.len() > MAX_ACCEPTED_GENERATED_CORRELATIONS)
        {
            return Err(GeneratedValidationError::InvalidAcceptedCorrelations);
        }
        for (index, correlation) in assessment.accepted_correlations.iter().enumerate() {
            if assessment.accepted_correlations[index + 1..].contains(correlation) {
                return Err(GeneratedValidationError::InvalidAcceptedCorrelations);
            }
            validate_correlation(request, correlation)
                .map_err(|_| GeneratedValidationError::InvalidAcceptedCorrelations)?;
        }
        if accepted {
            for affordance in &candidate.affordances {
                if !assessment.accepted_correlations.iter().any(|item| matches!(item, GeneratedSemanticCorrelation::Action { identity, .. } if identity == &affordance.action_identity)) {
                    return Err(GeneratedValidationError::InvalidAcceptedCorrelations);
                }
            }
        }
        let receipt_identity = receipt_identity(&self, &assessment);
        let receipt = GeneratedValidationReceipt {
            receipt_identity,
            validator_implementation_identity: self.validator_implementation_identity,
            validator_back_identity: self.validator_back_identity,
            assessment_identity: assessment.assessment_identity,
            candidate_digest: assessment.candidate_digest,
            source_presentation_identity: assessment.source_presentation_identity,
            source_presentation_revision: assessment.source_presentation_revision,
            mask_identity: assessment.mask_identity,
            mask_contract_revision: assessment.mask_contract_revision,
            accepted_correlations: assessment.accepted_correlations,
            disposition: assessment.disposition,
            terminal_code: assessment.refusal_code,
        };
        if accepted {
            Ok(GeneratedValidationOutcome::Accepted(Box::new(
                GeneratedManifestation::validated(candidate, receipt),
            )))
        } else {
            Ok(GeneratedValidationOutcome::Terminal(Box::new(receipt)))
        }
    }
}

#[allow(dead_code)]
fn receipt_identity(
    session: &GeneratedValidationSession,
    assessment: &GeneratedValidatorAssessment,
) -> String {
    let mut state = Sha256::new();
    state.update(b"conduit.presentation/generated-validation-receipt@1\0");
    for value in [
        &session.validator_implementation_identity,
        &session.validator_back_identity,
        &assessment.assessment_identity,
        &assessment.candidate_digest,
        &assessment.source_presentation_identity,
        &assessment.mask_identity,
        &assessment.mask_contract_revision,
    ] {
        state.update((value.len() as u64).to_be_bytes());
        state.update(value.as_bytes());
    }
    state.update(assessment.source_presentation_revision.to_be_bytes());
    state.update([match assessment.disposition {
        GeneratedValidationDisposition::Accepted => 0,
        GeneratedValidationDisposition::Refused => 1,
        GeneratedValidationDisposition::Failed => 2,
        GeneratedValidationDisposition::Cancelled => 3,
        GeneratedValidationDisposition::ValidatorLost => 4,
    }]);
    if let Some(code) = &assessment.refusal_code {
        state.update([1]);
        state.update((code.len() as u64).to_be_bytes());
        state.update(code.as_bytes());
    } else {
        state.update([0]);
    }
    for correlation in &assessment.accepted_correlations {
        crate::generated_correlation::hash_correlation(&mut state, correlation);
    }
    let digest = state.finalize();
    let mut output = String::from("sha256:");
    for byte in digest {
        use core::fmt::Write;
        write!(&mut output, "{byte:02x}").expect("string write");
    }
    output
}
