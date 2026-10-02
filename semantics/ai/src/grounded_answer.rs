//! Grounded answer assembly from one admitted context and ordinary LLM result.

use crate::{
    llm_contract, AnswerClaimSupport, AnswerSpan, Citation, CitationIndices,
    GroundedAnswerDisposition, GroundedAnswerPolicy, GroundedAnswerRefusal, GroundedClaimSupport,
    GroundingInputAssessment, GroundingLimitation, ModelDerivedResult, ModelResultDisposition,
    ModelResultProvenance, RetrievalIntent, StructuredContext, LLM_GENERATE_KIND,
    MAXIMUM_RAG_IDENTITY_BYTES, MAXIMUM_RAG_TEXT_BYTES,
};
use alloc::{string::String, vec::Vec};
use conduit_plot::rust_binding::BoundedSequence;

pub const MAXIMUM_GROUNDED_ANSWER_WORK_UNITS: u64 = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq)]
/// Rust's specialization of the authored `GroundedAnswerRequest<T, TContext>` Type family.
pub struct GroundedAnswerRequest {
    pub identity: String,
    pub retrieval_intent: RetrievalIntent,
    pub context: StructuredContext,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Established carrier for the authored `ProposedClaimSupport<Citation>` Type.
pub enum ProposedClaimSupport {
    Supported { citations: Vec<Citation> },
    Unsupported { rationale: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Established carrier for the authored `ProposedGroundedClaim<Citation>` Type.
pub struct ProposedGroundedClaim {
    pub answer_span: AnswerSpan,
    pub support: ProposedClaimSupport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Flat compatibility carrier for the authored
/// `GroundedAnswer<GroundedAnswerBytePages, Citation>` Type.
pub struct GroundedAnswer {
    pub provenance: ModelResultProvenance,
    pub policy_identity: String,
    pub request_identity: String,
    pub context_policy_identity: String,
    pub model_implementation_identity: String,
    pub model_run_identity: String,
    pub answer_kind: String,
    pub answer: Vec<u8>,
    pub disposition: GroundedAnswerDisposition,
    pub claims: Vec<AnswerClaimSupport>,
    pub citations: Vec<Citation>,
    pub limitations: Vec<String>,
}

impl GroundedAnswerPolicy {
    pub fn assemble(
        &self,
        request: &GroundedAnswerRequest,
        assessment: &GroundingInputAssessment,
        model_result: &ModelDerivedResult,
        proposed_claims: &[ProposedGroundedClaim],
    ) -> Result<GroundedAnswer, GroundedAnswerRefusal> {
        validate_request(request)?;
        let contract = llm_contract(LLM_GENERATE_KIND).expect("generate contract is catalogued");
        model_result
            .validate(&contract)
            .map_err(GroundedAnswerRefusal::InvalidModelResult)?;
        if model_result.request_identity != request.identity {
            return Err(GroundedAnswerRefusal::ModelRequestMismatch);
        }
        if !matches!(
            model_result.disposition,
            ModelResultDisposition::Produced | ModelResultDisposition::Truncated
        ) {
            return Err(GroundedAnswerRefusal::ModelDidNotProduce(
                model_result.disposition,
            ));
        }
        if model_result.payload.len() > *self.maximum_output_bytes() as usize {
            return Err(GroundedAnswerRefusal::OutputBoundExceeded);
        }
        if model_result.accounting.work_units > *self.maximum_work_units() {
            return Err(GroundedAnswerRefusal::WorkBoundExceeded);
        }
        if model_result.accounting.context_items != request.context.items.len() as u64 {
            return Err(GroundedAnswerRefusal::ContextAccountingMismatch);
        }
        if proposed_claims.is_empty() {
            return Err(GroundedAnswerRefusal::EmptyClaims);
        }
        if proposed_claims.len() > usize::from(*self.maximum_claims()) {
            return Err(GroundedAnswerRefusal::ClaimLimitExceeded);
        }
        let mut citations = Vec::new();
        let mut claims = Vec::with_capacity(proposed_claims.len());
        let mut unsupported = false;
        for proposed in proposed_claims {
            validate_span(proposed.answer_span, model_result.payload.len())?;
            let support = match &proposed.support {
                ProposedClaimSupport::Supported {
                    citations: proposed_citations,
                } => {
                    if proposed_citations.is_empty() {
                        return Err(GroundedAnswerRefusal::EmptyCitationSet);
                    }
                    let mut indices = Vec::with_capacity(proposed_citations.len());
                    for citation in proposed_citations {
                        validate_citation(citation, &request.context)?;
                        let index = match citations.iter().position(|item| item == citation) {
                            Some(index) => index,
                            None => {
                                if citations.len() >= usize::from(*self.maximum_citations()) {
                                    return Err(GroundedAnswerRefusal::CitationLimitExceeded);
                                }
                                citations.push(citation.clone());
                                citations.len() - 1
                            }
                        };
                        let index = u16::try_from(index)
                            .map_err(|_| GroundedAnswerRefusal::ArithmeticOverflow)?;
                        if indices.contains(&index) {
                            return Err(GroundedAnswerRefusal::DuplicateCitation);
                        }
                        indices.push(index);
                    }
                    GroundedClaimSupport::supported(
                        CitationIndices::new(
                            BoundedSequence::try_from_iter(indices)
                                .expect("grounding policy admits at most 128 citations"),
                        )
                        .expect("supported claims always contain citations"),
                    )
                    .expect("checked citation indices make valid support")
                }
                ProposedClaimSupport::Unsupported { rationale } => {
                    validate_limitation(rationale)?;
                    unsupported = true;
                    GroundedClaimSupport::unsupported(
                        GroundingLimitation::new(rationale.clone())
                            .expect("validated rationale is a grounding limitation"),
                    )
                    .expect("checked limitation makes valid unsupported support")
                }
            };
            claims.push(
                AnswerClaimSupport::new(proposed.answer_span, support)
                    .expect("checked span and support make valid claim support"),
            );
        }
        let (disposition, limitations) = match assessment {
            GroundingInputAssessment::Sufficient => (
                if unsupported {
                    GroundedAnswerDisposition::PartiallySupported
                } else {
                    GroundedAnswerDisposition::Supported
                },
                Vec::new(),
            ),
            GroundingInputAssessment::InsufficientEvidence(payload) => (
                GroundedAnswerDisposition::InsufficientEvidence,
                alloc::vec![payload.limitation().get().clone()],
            ),
            GroundingInputAssessment::ConflictingEvidence(payload) => (
                GroundedAnswerDisposition::ConflictingEvidence,
                alloc::vec![payload.limitation().get().clone()],
            ),
        };
        Ok(GroundedAnswer {
            provenance: ModelResultProvenance::ModelDerived,
            policy_identity: self.identity().clone(),
            request_identity: request.identity.clone(),
            context_policy_identity: request.context.policy_identity.clone(),
            model_implementation_identity: model_result.implementation_identity.clone(),
            model_run_identity: model_result.run_identity.clone(),
            answer_kind: self.answer_kind().clone(),
            answer: model_result.payload.clone(),
            disposition,
            claims,
            citations,
            limitations,
        })
    }
}

fn validate_request(request: &GroundedAnswerRequest) -> Result<(), GroundedAnswerRefusal> {
    validate_identity(&request.identity)?;
    request
        .retrieval_intent
        .validate()
        .map_err(|_| GroundedAnswerRefusal::InvalidRetrievalIntent)?;
    validate_identity(&request.context.policy_identity)?;
    validate_identity(&request.context.token_accounting_profile)?;
    if request.context.items.is_empty()
        || request.context.items.len() > crate::MAXIMUM_CONTEXT_ITEMS
    {
        return Err(GroundedAnswerRefusal::InvalidContext);
    }
    let (mut bytes, mut tokens, mut work_units) = (0_u32, 0_u32, 0_u32);
    for (index, item) in request.context.items.iter().enumerate() {
        item.reranked
            .candidate
            .chunk
            .validate()
            .map_err(|_| GroundedAnswerRefusal::InvalidContext)?;
        validate_identity(&item.reranking_policy_identity)?;
        if request.context.items[index + 1..].iter().any(|other| {
            other.reranked.candidate.chunk.identity == item.reranked.candidate.chunk.identity
        }) {
            return Err(GroundedAnswerRefusal::DuplicateContextItem);
        }
        bytes = bytes
            .checked_add(item.budget.bytes())
            .ok_or(GroundedAnswerRefusal::ArithmeticOverflow)?;
        tokens = tokens
            .checked_add(item.budget.tokens())
            .ok_or(GroundedAnswerRefusal::ArithmeticOverflow)?;
        work_units = work_units
            .checked_add(item.budget.work_units)
            .ok_or(GroundedAnswerRefusal::ArithmeticOverflow)?;
    }
    if request.context.used.bytes != bytes
        || request.context.used.tokens != tokens
        || request.context.used.work_units != work_units
    {
        return Err(GroundedAnswerRefusal::ContextAccountingMismatch);
    }
    Ok(())
}

fn validate_citation(
    citation: &Citation,
    context: &StructuredContext,
) -> Result<(), GroundedAnswerRefusal> {
    if context.items.iter().any(|item| {
        let chunk = &item.reranked.candidate.chunk;
        chunk.identity == citation.chunk_identity
            && chunk.lineage.source == citation.source
            && chunk.lineage.span == citation.span
    }) {
        Ok(())
    } else {
        Err(GroundedAnswerRefusal::CitationNotInContext)
    }
}

fn validate_span(span: AnswerSpan, answer_bytes: usize) -> Result<(), GroundedAnswerRefusal> {
    if span.end() as usize > answer_bytes {
        Err(GroundedAnswerRefusal::InvalidAnswerSpan)
    } else {
        Ok(())
    }
}

fn validate_identity(value: &str) -> Result<(), GroundedAnswerRefusal> {
    if value.is_empty() {
        return Err(GroundedAnswerRefusal::EmptyIdentity);
    }
    if value.len() > MAXIMUM_RAG_IDENTITY_BYTES {
        return Err(GroundedAnswerRefusal::IdentityTooLarge);
    }
    Ok(())
}

fn validate_limitation(value: &str) -> Result<(), GroundedAnswerRefusal> {
    if value.is_empty() {
        return Err(GroundedAnswerRefusal::EmptyLimitation);
    }
    if value.len() > MAXIMUM_RAG_TEXT_BYTES {
        return Err(GroundedAnswerRefusal::LimitationTooLarge);
    }
    Ok(())
}
