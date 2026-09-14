//! Exact bridge from selected autobiography records to generic grounded answers.
//! Selection grants neither present-time truth nor authority beyond the checked read.

use crate::{BoundedAutobiography, MemoryRefusal};
use conduit_ai::{
    GroundedAnswer, GroundedAnswerPolicy, GroundedAnswerRefusal, GroundedAnswerRequest,
    GroundingInputAssessment, ModelDerivedResult, ProposedGroundedClaim,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeteRecollectionTrace {
    pub selected_experience_identities: Vec<String>,
    pub grounded_answer: GroundedAnswer,
    /// Retained observations remain historical evidence and cannot satisfy current-situation input.
    pub historical_not_current: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PeteRecollectionRefusal {
    Memory(MemoryRefusal),
    EmptySelection,
    DuplicateSelection,
    SelectedRecordUnavailable,
    ContextWithoutExperienceIdentity,
    ContextRecordNotSelected,
    SelectedRecordAbsentFromContext,
    Grounding(GroundedAnswerRefusal),
}

pub struct PeteRecollectionInputs<'a> {
    pub memory: &'a BoundedAutobiography,
    pub selected_experience_identities: &'a [String],
    pub read_authorized: bool,
    pub policy: &'a GroundedAnswerPolicy,
    pub request: &'a GroundedAnswerRequest,
    pub assessment: &'a GroundingInputAssessment,
    pub model_result: &'a ModelDerivedResult,
    pub proposed_claims: &'a [ProposedGroundedClaim],
}

pub fn assemble_pete_recollection(
    inputs: PeteRecollectionInputs<'_>,
) -> Result<PeteRecollectionTrace, PeteRecollectionRefusal> {
    let PeteRecollectionInputs {
        memory,
        selected_experience_identities,
        read_authorized,
        policy,
        request,
        assessment,
        model_result,
        proposed_claims,
    } = inputs;
    if selected_experience_identities.is_empty() {
        return Err(PeteRecollectionRefusal::EmptySelection);
    }
    if selected_experience_identities
        .iter()
        .enumerate()
        .any(|(index, identity)| {
            identity.is_empty() || selected_experience_identities[index + 1..].contains(identity)
        })
    {
        return Err(PeteRecollectionRefusal::DuplicateSelection);
    }
    let retained = memory
        .records(read_authorized)
        .map_err(PeteRecollectionRefusal::Memory)?;
    if selected_experience_identities.iter().any(|identity| {
        !retained
            .iter()
            .any(|record| record.candidate.identity == *identity)
    }) {
        return Err(PeteRecollectionRefusal::SelectedRecordUnavailable);
    }

    let mut context_identities = Vec::with_capacity(request.context.items.len());
    for item in &request.context.items {
        let identity = item
            .temporal
            .as_ref()
            .map(|temporal| temporal.evidence_identity.as_str())
            .ok_or(PeteRecollectionRefusal::ContextWithoutExperienceIdentity)?;
        if !selected_experience_identities
            .iter()
            .any(|selected| selected == identity)
        {
            return Err(PeteRecollectionRefusal::ContextRecordNotSelected);
        }
        context_identities.push(identity);
    }
    if selected_experience_identities
        .iter()
        .any(|selected| !context_identities.contains(&selected.as_str()))
    {
        return Err(PeteRecollectionRefusal::SelectedRecordAbsentFromContext);
    }

    let grounded_answer = policy
        .assemble(request, assessment, model_result, proposed_claims)
        .map_err(PeteRecollectionRefusal::Grounding)?;
    Ok(PeteRecollectionTrace {
        selected_experience_identities: selected_experience_identities.to_vec(),
        grounded_answer,
        historical_not_current: true,
    })
}

#[cfg(test)]
#[path = "pete_recollection_tests.rs"]
mod tests;
