//! Trusted retention boundary for untrusted semantic-validator assessments.

use alloc::{boxed::Box, string::String, vec::Vec};
use conduit_core::{
    verify_plan, BootId, CapabilityId, HostId, ImplementationId, PlacementId, Plan, PlanId,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    generated_correlation::{validate_correlation, validate_generated_candidate},
    GeneratedManifestation, GeneratedManifestationCandidate, GeneratedSemanticCorrelation,
    GeneratedValidationDisposition, GeneratedValidationError, GeneratedValidationReceipt,
    GenerativePresenterRequest,
};

pub const MAX_ACCEPTED_GENERATED_CORRELATIONS: usize = 128;
pub const MAX_GENERATED_VALIDATION_ENVELOPE_BYTES: usize =
    crate::MAX_GENERATIVE_PRESENTER_INPUT_BYTES
        + crate::MAX_GENERATIVE_PRESENTER_OUTPUT_BYTES
        + 1_024;
pub const MAX_VALIDATION_REFUSAL_CODE_BYTES: usize = 128;
pub const GENERATED_VALIDATOR_KIND: &str = "presentation/generated-semantic-validator";
pub const GENERATED_VALIDATOR_CONTRACT_REVISION: &str =
    "conduit.presentation/generated-semantic-validator@2";
pub const GENERATED_VALIDATION_ENVELOPE_VALUE_KIND: &str =
    "conduit.presentation/generated-validation-envelope@2";
pub const GENERATED_VALIDATOR_ASSESSMENT_VALUE_KIND: &str =
    "conduit.presentation/generated-validator-assessment@1";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedValidationEnvelope {
    pub request: GenerativePresenterRequest,
    pub candidate: GeneratedManifestationCandidate,
}

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
    plan_id: PlanId,
    placement_id: PlacementId,
    validator_back_identity: CapabilityId,
    validator_implementation_identity: ImplementationId,
    host_id: HostId,
    boot_id: BootId,
    mask_identity: String,
    mask_contract_revision: String,
}

#[allow(dead_code)]
impl GeneratedValidationSession {
    /// Assess one envelope under the identity of this exact planned validator.
    /// The returned assessment is still untrusted until [`Self::retain`]
    /// correlates it back to this same session and Plan.
    pub fn assess(
        &self,
        envelope: &GeneratedValidationEnvelope,
        assessment_identity: String,
    ) -> GeneratedValidatorAssessment {
        assess_generated_output_exactly(envelope, assessment_identity, self.mask_identity.clone())
    }

    pub fn from_plan(
        plan: &Plan,
        placement_id: &PlacementId,
        mask_identity: String,
        mask_contract_revision: String,
    ) -> Result<Self, GeneratedValidationError> {
        if !verify_plan(plan) {
            return Err(GeneratedValidationError::InvalidPlan);
        }
        let placement = plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .find(|placement| &placement.placement_id == placement_id)
            .ok_or(GeneratedValidationError::MissingValidatorPlacement)?;
        if placement.kind_id.as_str() != GENERATED_VALIDATOR_KIND
            || placement.kind_contract_revision.as_str() != GENERATED_VALIDATOR_CONTRACT_REVISION
            || placement.inputs.len() != 1
            || placement.inputs[0].value_kind.as_str() != GENERATED_VALIDATION_ENVELOPE_VALUE_KIND
            || placement.outputs.len() != 1
            || placement.outputs[0].value_kind.as_str() != GENERATED_VALIDATOR_ASSESSMENT_VALUE_KIND
        {
            return Err(GeneratedValidationError::WrongValidatorKind);
        }
        for value in [&mask_identity, &mask_contract_revision] {
            crate::generative_presenter::validate_identity(value)
                .map_err(|_| GeneratedValidationError::InvalidValidatorIdentity)?;
        }
        Ok(Self {
            plan_id: plan.plan_id.clone(),
            placement_id: placement.placement_id.clone(),
            validator_back_identity: placement.capability_id.clone(),
            validator_implementation_identity: placement.implementation_id.clone(),
            host_id: placement.host_id.clone(),
            boot_id: placement.boot_id.clone(),
            mask_identity,
            mask_contract_revision,
        })
    }

    #[cfg(test)]
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
            plan_id: PlanId::from("plan/test-generated-validator"),
            placement_id: PlacementId::from("placement/test-generated-validator"),
            validator_implementation_identity: ImplementationId::from(
                validator_implementation_identity,
            ),
            validator_back_identity: CapabilityId::from(validator_back_identity),
            host_id: HostId::from("host/test-generated-validator"),
            boot_id: BootId::from("boot/test-generated-validator"),
            mask_identity,
            mask_contract_revision,
        })
    }

    pub fn retain(
        self,
        plan: &Plan,
        request: &GenerativePresenterRequest,
        candidate: GeneratedManifestationCandidate,
        assessment: GeneratedValidatorAssessment,
    ) -> Result<GeneratedValidationOutcome, GeneratedValidationError> {
        let placement = exact_session_placement(plan, &self)?;
        if placement.kind_id.as_str() != GENERATED_VALIDATOR_KIND {
            return Err(GeneratedValidationError::WrongValidatorKind);
        }
        self.retain_assessment(request, candidate, assessment)
    }

    #[cfg(test)]
    pub(crate) fn retain_unplanned(
        self,
        request: &GenerativePresenterRequest,
        candidate: GeneratedManifestationCandidate,
        assessment: GeneratedValidatorAssessment,
    ) -> Result<GeneratedValidationOutcome, GeneratedValidationError> {
        self.retain_assessment(request, candidate, assessment)
    }

    fn retain_assessment(
        self,
        request: &GenerativePresenterRequest,
        candidate: GeneratedManifestationCandidate,
        assessment: GeneratedValidatorAssessment,
    ) -> Result<GeneratedValidationOutcome, GeneratedValidationError> {
        validate_generated_candidate(request, &candidate)
            .map_err(GeneratedValidationError::InvalidCandidate)?;
        if !candidate.wording_envelope_is_bounded() {
            return Err(GeneratedValidationError::InvalidAssessmentContract);
        }
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
        let finite_wording = request.policy.template_contract_revision
            == crate::FINITE_FACE_WORDING_TEMPLATE_REVISION;
        // A selected validator is still an untrusted implementation boundary.
        // Its acceptance cannot turn a stale or invented model wording proposal
        // into speech merely by naming valid Face correlations.
        if accepted
            && (candidate.disposition != crate::GeneratedManifestationDisposition::Produced
                || (finite_wording != candidate.wording_proposal.is_some())
                || (finite_wording
                    && (!exact_model_wording(request, &candidate)
                        || assessment.accepted_correlations != candidate.correlations)))
        {
            return Err(GeneratedValidationError::InvalidAssessmentContract);
        }
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
                if !assessment.accepted_correlations.iter().any(|item| matches!(item, GeneratedSemanticCorrelation::Action { identity, .. } if identity == affordance.action_identity())) {
                    return Err(GeneratedValidationError::InvalidAcceptedCorrelations);
                }
            }
        }
        let receipt_identity = receipt_identity(&self, &assessment);
        let receipt = GeneratedValidationReceipt {
            receipt_identity,
            plan_id: self.plan_id,
            placement_id: self.placement_id,
            validator_implementation_identity: self.validator_implementation_identity,
            validator_back_identity: self.validator_back_identity,
            host_id: self.host_id,
            boot_id: self.boot_id,
            assessment_identity: assessment.assessment_identity,
            candidate_digest: assessment.candidate_digest,
            source_presentation_identity: assessment.source_presentation_identity,
            source_presentation_revision: assessment.source_presentation_revision,
            mask_identity: assessment.mask_identity,
            mask_contract_revision: assessment.mask_contract_revision,
            accepted_correlations: assessment.accepted_correlations,
            disposition: assessment.disposition,
            terminal_code: assessment.refusal_code,
            raw_provider_output: candidate.raw_provider_output.clone(),
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

fn exact_session_placement<'a>(
    plan: &'a Plan,
    session: &GeneratedValidationSession,
) -> Result<&'a conduit_core::PlannedGear, GeneratedValidationError> {
    if !verify_plan(plan) || plan.plan_id != session.plan_id {
        return Err(GeneratedValidationError::StaleValidatorBinding);
    }
    let placement = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .find(|placement| placement.placement_id == session.placement_id)
        .ok_or(GeneratedValidationError::StaleValidatorBinding)?;
    if placement.capability_id != session.validator_back_identity
        || placement.implementation_id != session.validator_implementation_identity
        || placement.host_id != session.host_id
        || placement.boot_id != session.boot_id
        || placement.kind_contract_revision.as_str() != GENERATED_VALIDATOR_CONTRACT_REVISION
    {
        return Err(GeneratedValidationError::StaleValidatorBinding);
    }
    Ok(placement)
}

pub fn assess_generated_output_exactly(
    envelope: &GeneratedValidationEnvelope,
    assessment_identity: String,
    mask_identity: String,
) -> GeneratedValidatorAssessment {
    let candidate = &envelope.candidate;
    let finite_wording = envelope.request.policy.template_contract_revision
        == crate::FINITE_FACE_WORDING_TEMPLATE_REVISION;
    let accepted = candidate.disposition == crate::GeneratedManifestationDisposition::Produced
        && validate_generated_candidate(&envelope.request, candidate).is_ok()
        && candidate.wording_envelope_is_bounded()
        && finite_wording == candidate.wording_proposal.is_some()
        && if finite_wording {
            exact_model_wording(&envelope.request, candidate)
        } else {
            candidate.raw_provider_output.is_none() && exact_text_segments(envelope)
        }
        && exact_action_correlations(envelope);
    GeneratedValidatorAssessment {
        assessment_identity,
        candidate_digest: candidate.digest(),
        source_presentation_identity: candidate.source_presentation_identity.clone(),
        source_presentation_revision: candidate.source_presentation_revision,
        mask_identity,
        mask_contract_revision: candidate.mask_contract_revision.clone(),
        disposition: if accepted {
            GeneratedValidationDisposition::Accepted
        } else {
            GeneratedValidationDisposition::Refused
        },
        refusal_code: (!accepted).then(|| String::from("not-exactly-reconstructible")),
        accepted_correlations: if accepted {
            candidate.correlations.clone()
        } else {
            Vec::new()
        },
    }
}

fn exact_model_wording(
    request: &GenerativePresenterRequest,
    candidate: &GeneratedManifestationCandidate,
) -> bool {
    let Some(proposal) = &candidate.wording_proposal else {
        return false;
    };
    let Some(raw) = &candidate.raw_provider_output else {
        return false;
    };
    if raw.is_empty() || raw.len() > crate::MAX_RAW_PRESENTER_OUTPUT_BYTES {
        return false;
    }
    let Ok(rendered) = proposal.render_exact(&request.semantic_data.presentation) else {
        return false;
    };
    if !matches!(candidate.content.as_slice(), [segment]
        if segment.role == crate::GeneratedContentRole::Speech
            && segment.bytes == rendered.as_bytes())
    {
        return false;
    }
    proposal.clauses.iter().all(|clause| {
        candidate
            .correlations
            .iter()
            .any(|correlation| match (clause, correlation) {
                (
                    crate::GeneratedWordingClause::Text { index, subject, .. },
                    GeneratedSemanticCorrelation::Text {
                        index: actual,
                        subject: actual_subject,
                    },
                ) => index == actual && subject == actual_subject,
                (
                    crate::GeneratedWordingClause::Property {
                        index,
                        subject,
                        name,
                        ..
                    },
                    GeneratedSemanticCorrelation::Property {
                        index: actual,
                        subject: actual_subject,
                        name: actual_name,
                    },
                ) => index == actual && subject == actual_subject && name == actual_name,
                (
                    crate::GeneratedWordingClause::Action {
                        index, identity, ..
                    },
                    GeneratedSemanticCorrelation::Action {
                        index: actual,
                        identity: actual_identity,
                        ..
                    },
                ) => index == actual && identity == actual_identity,
                _ => false,
            })
    })
}

fn exact_text_segments(envelope: &GeneratedValidationEnvelope) -> bool {
    envelope.candidate.content.iter().all(|segment| {
        let Some(text) = envelope
            .request
            .semantic_data
            .presentation
            .text
            .get(segment.source_text_index as usize)
        else {
            return false;
        };
        segment.bytes == text.text.as_bytes()
            && envelope.candidate.correlations.iter().any(|correlation| {
                matches!(correlation, GeneratedSemanticCorrelation::Text { index, subject }
                    if *index == segment.source_text_index && subject == &text.subject)
            })
    })
}

fn exact_action_correlations(envelope: &GeneratedValidationEnvelope) -> bool {
    let actions = envelope
        .candidate
        .correlations
        .iter()
        .filter(|correlation| matches!(correlation, GeneratedSemanticCorrelation::Action { .. }))
        .collect::<Vec<_>>();
    actions.len() == envelope.candidate.affordances.len()
        && actions.iter().all(|correlation| match correlation {
            GeneratedSemanticCorrelation::Action { identity, .. } => envelope
                .candidate
                .affordances
                .iter()
                .any(|affordance| affordance.action_identity() == identity),
            _ => false,
        })
}

#[allow(dead_code)]
fn receipt_identity(
    session: &GeneratedValidationSession,
    assessment: &GeneratedValidatorAssessment,
) -> String {
    let mut state = Sha256::new();
    state.update(b"conduit.presentation/generated-validation-receipt@1\0");
    for value in [
        session.plan_id.as_str(),
        session.placement_id.as_str(),
        session.validator_implementation_identity.as_str(),
        session.validator_back_identity.as_str(),
        session.host_id.as_str(),
        session.boot_id.as_str(),
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
