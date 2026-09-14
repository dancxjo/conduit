//! Exact bridge from selected autobiography records to generic grounded answers.
//! Selection grants neither present-time truth nor authority beyond the checked read.

use crate::{
    BoundedAutobiography, ExperienceKind, MemoryRefusal, Sensitivity, MAXIMUM_RETAINED_EXPERIENCES,
};
use conduit_ai::{
    GroundedAnswer, GroundedAnswerPolicy, GroundedAnswerRefusal, GroundedAnswerRequest,
    GroundingInputAssessment, ModelDerivedResult, ProposedGroundedClaim,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeteRecollectionTrace {
    pub retrieval_candidates: Vec<PeteRetrievalCandidate>,
    pub selected_experiences: Vec<PeteSelectedExperienceTrace>,
    pub selected_experience_identities: Vec<String>,
    pub grounded_answer: GroundedAnswer,
    /// Retained observations remain historical evidence and cannot satisfy current-situation input.
    pub historical_not_current: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PeteRetrievalBasis {
    Lexical,
    Metadata,
    Temporal,
    VectorSimilarity,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeteRetrievalCandidate {
    pub experience_identity: String,
    pub bases: Vec<PeteRetrievalBasis>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeteSelectedExperienceTrace {
    pub experience_identity: String,
    pub experience_kind: ExperienceKind,
    pub sensitivity: Sensitivity,
    pub original_source_identities: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PeteRecollectionRefusal {
    Memory(MemoryRefusal),
    EmptySelection,
    EmptyCandidates,
    TooManyCandidates,
    InvalidCandidate,
    DuplicateCandidate,
    CandidateRecordUnavailable,
    SelectedRecordNotCandidate,
    DuplicateSelection,
    SelectedRecordUnavailable,
    ContextWithoutExperienceIdentity,
    ContextRecordNotSelected,
    SelectedRecordAbsentFromContext,
    Grounding(GroundedAnswerRefusal),
}

pub struct PeteRecollectionInputs<'a> {
    pub memory: &'a BoundedAutobiography,
    pub retrieval_candidates: &'a [PeteRetrievalCandidate],
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
        retrieval_candidates,
        selected_experience_identities,
        read_authorized,
        policy,
        request,
        assessment,
        model_result,
        proposed_claims,
    } = inputs;
    if retrieval_candidates.is_empty() {
        return Err(PeteRecollectionRefusal::EmptyCandidates);
    }
    if retrieval_candidates.len() > MAXIMUM_RETAINED_EXPERIENCES {
        return Err(PeteRecollectionRefusal::TooManyCandidates);
    }
    for (index, candidate) in retrieval_candidates.iter().enumerate() {
        if candidate.experience_identity.is_empty() || candidate.bases.is_empty() {
            return Err(PeteRecollectionRefusal::InvalidCandidate);
        }
        if retrieval_candidates[index + 1..]
            .iter()
            .any(|other| other.experience_identity == candidate.experience_identity)
        {
            return Err(PeteRecollectionRefusal::DuplicateCandidate);
        }
    }
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
    if selected_experience_identities.iter().any(|selected| {
        !retrieval_candidates
            .iter()
            .any(|candidate| candidate.experience_identity == *selected)
    }) {
        return Err(PeteRecollectionRefusal::SelectedRecordNotCandidate);
    }
    let retained = memory
        .records(read_authorized)
        .map_err(PeteRecollectionRefusal::Memory)?;
    if retrieval_candidates.iter().any(|candidate| {
        !retained
            .iter()
            .any(|record| record.candidate.identity == candidate.experience_identity)
    }) {
        return Err(PeteRecollectionRefusal::CandidateRecordUnavailable);
    }
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
    let selected_experiences = selected_experience_identities
        .iter()
        .map(|identity| {
            let record = retained
                .iter()
                .find(|record| record.candidate.identity == *identity)
                .expect("selected identities were checked against retained records");
            PeteSelectedExperienceTrace {
                experience_identity: identity.clone(),
                experience_kind: record.candidate.kind,
                sensitivity: record.candidate.sensitivity,
                original_source_identities: record
                    .candidate
                    .provenance
                    .iter()
                    .map(|source| source.source_identity.clone())
                    .collect(),
            }
        })
        .collect();
    Ok(PeteRecollectionTrace {
        retrieval_candidates: retrieval_candidates.to_vec(),
        selected_experiences,
        selected_experience_identities: selected_experience_identities.to_vec(),
        grounded_answer,
        historical_not_current: true,
    })
}

#[cfg(test)]
#[path = "pete_recollection_tests.rs"]
mod tests;
