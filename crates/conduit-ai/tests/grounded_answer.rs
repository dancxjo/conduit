use conduit_ai::{
    AnswerSpan, Chunk, Citation, ContextOrderingPolicy, ContextRedundancyPolicy,
    ContextSelectionDisposition, ExtractedSourceValue, ExtractionLineage,
    GroundedAnswerDisposition, GroundedAnswerPolicy, GroundedAnswerRefusal, GroundedAnswerRequest,
    GroundedClaimSupport, GroundingInputAssessment, HybridCandidate, LlmDeterminismProfile,
    MechanismScore, ModelDerivedResult, ModelRefusal, ModelResultDisposition,
    ModelResultProvenance, ModelWorkAccounting, ProposedClaimSupport, ProposedGroundedClaim,
    RerankScore, RerankedCandidate, RerankingProofClass, RetrievalContribution, RetrievalIntent,
    RetrievalMechanism, RetrievalMode, RetrieverIdentity, SelectedContextCost, SelectedContextItem,
    SelectedContextRationale, SourceRef, SourceSpan, SourceSpanUnit, StructuredContext,
};
use conduit_core::{
    BoundedResourceRef, KindId, ResourceClassId, ResourceExtent, ResourceLifetime,
    ResourceSemanticIdentity, ResourceVersionIdentity,
};

fn source(version: u8) -> SourceRef {
    SourceRef {
        resource: BoundedResourceRef {
            identity: ResourceSemanticIdentity::from_digest([7; 32]),
            content_profile: KindId::from("document/text-utf8@1"),
            access_class: ResourceClassId::from("resource/read-authorized@1"),
            extent: ResourceExtent {
                bytes: 1_024,
                items: None,
            },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest([version; 32]),
                expires_at: None,
            },
        },
    }
}

fn selected_item(version: u8, rank: u16, start: u64, text: &str) -> SelectedContextItem {
    let chunk = Chunk::new(
        ExtractionLineage {
            source: source(version),
            span: SourceSpan {
                unit: SourceSpanUnit::Bytes,
                start,
                end: start + text.len() as u64,
            },
            extraction_profile: "extract/text-utf8@1".into(),
            transform_profiles: vec![],
            parent_chunk: None,
        },
        ExtractedSourceValue::Text(text.as_bytes().to_vec()),
    )
    .unwrap();
    SelectedContextItem {
        reranked: RerankedCandidate {
            candidate: HybridCandidate {
                chunk,
                rank,
                fusion_score_micros: 100_000 - u64::from(rank),
                contributions: vec![RetrievalContribution {
                    retriever: RetrieverIdentity {
                        identity: "retriever/exact@1".into(),
                        mechanism: RetrievalMechanism::DomainExact,
                    },
                    stage_rank: rank,
                    score: Some(MechanismScore::ExactMatch),
                    temporal_evidence_identity: None,
                }],
            },
            original_rank: rank,
            reranked_rank: rank,
            score: RerankScore::HybridFusion(100_000 - u64::from(rank)),
        },
        reranking_policy_identity: "rerank/preserve-hybrid@1".into(),
        reranking_proof_class: RerankingProofClass::DeterministicConformance,
        temporal: None,
        redundancy_group: None,
        rationale: SelectedContextRationale::Reranked,
        budget: SelectedContextCost {
            bytes: text.len() as u32,
            tokens: 4,
            work_units: 1,
        },
    }
}

fn fixture_request() -> GroundedAnswerRequest {
    let items = vec![
        selected_item(1, 1, 0, "The project began in April."),
        selected_item(2, 2, 64, "The current design is bounded."),
    ];
    GroundedAnswerRequest {
        identity: "request/project-history/7".into(),
        retrieval_intent: RetrievalIntent {
            identity: "intent/project-history".into(),
            modes: vec![RetrievalMode::Exact],
            maximum_candidates: 8,
        },
        context: StructuredContext {
            policy_identity: "context/reranked-diverse@1".into(),
            token_accounting_profile: "tokens/exact-fixture@1".into(),
            redundancy: ContextRedundancyPolicy::KeepAll,
            ordering: ContextOrderingPolicy::Reranked,
            used: SelectedContextCost {
                bytes: items.iter().map(|item| item.budget.bytes).sum(),
                tokens: items.iter().map(|item| item.budget.tokens).sum(),
                work_units: items.iter().map(|item| item.budget.work_units).sum(),
            },
            items,
            disposition: ContextSelectionDisposition::Complete,
        },
    }
}

fn policy() -> GroundedAnswerPolicy {
    GroundedAnswerPolicy {
        identity: "grounding/exact-context-citations@1".into(),
        answer_kind: "value/text-utf8@1".into(),
        maximum_output_bytes: 1_024,
        maximum_claims: 8,
        maximum_citations: 8,
        maximum_work_units: 64,
    }
}

fn model_result(
    implementation: &str,
    run: &str,
    disposition: ModelResultDisposition,
) -> ModelDerivedResult {
    let payload = if matches!(
        disposition,
        ModelResultDisposition::Produced | ModelResultDisposition::Truncated
    ) {
        b"The project began in April; the current design is bounded.".to_vec()
    } else {
        vec![]
    };
    ModelDerivedResult {
        provenance: ModelResultProvenance::ModelDerived,
        payload_kind: "llm/generated-result@1".into(),
        implementation_identity: implementation.into(),
        request_identity: "request/project-history/7".into(),
        run_identity: run.into(),
        confidence: None,
        disposition,
        determinism: LlmDeterminismProfile::ProviderNondeterministic,
        accounting: ModelWorkAccounting {
            input_bytes: 128,
            context_items: 2,
            output_bytes: payload.len() as u64,
            work_units: 32,
            history_items: 0,
        },
        payload,
    }
}

fn citation(request: &GroundedAnswerRequest, index: usize) -> Citation {
    let chunk = &request.context.items[index].reranked.candidate.chunk;
    Citation {
        source: chunk.lineage.source.clone(),
        span: chunk.lineage.span,
        chunk_identity: chunk.identity,
    }
}

fn supported_claims(request: &GroundedAnswerRequest) -> Vec<ProposedGroundedClaim> {
    vec![
        ProposedGroundedClaim {
            answer_span: AnswerSpan { start: 0, end: 27 },
            support: ProposedClaimSupport::Supported {
                citations: vec![citation(request, 0)],
            },
        },
        ProposedGroundedClaim {
            answer_span: AnswerSpan { start: 29, end: 58 },
            support: ProposedClaimSupport::Supported {
                citations: vec![citation(request, 1)],
            },
        },
    ]
}

#[test]
fn ordinary_model_replacement_preserves_exact_portable_grounding() {
    let request = fixture_request();
    let claims = supported_claims(&request);
    let first = policy()
        .assemble(
            &request,
            &GroundingInputAssessment::Sufficient,
            &model_result("model/local-a", "run/1", ModelResultDisposition::Produced),
            &claims,
        )
        .unwrap();
    let replacement = policy()
        .assemble(
            &request,
            &GroundingInputAssessment::Sufficient,
            &model_result("model/cloud-b", "run/2", ModelResultDisposition::Produced),
            &claims,
        )
        .unwrap();
    assert_eq!(first.disposition, GroundedAnswerDisposition::Supported);
    assert_eq!(first.provenance, ModelResultProvenance::ModelDerived);
    assert_eq!(first.citations, replacement.citations);
    assert_eq!(first.claims, replacement.claims);
    assert_ne!(
        first.model_implementation_identity,
        replacement.model_implementation_identity
    );
    assert_eq!(
        first.context_policy_identity,
        request.context.policy_identity
    );
}

#[test]
fn hallucinated_or_mutated_non_context_citations_refuse() {
    let request = fixture_request();
    let mut claims = supported_claims(&request);
    let ProposedClaimSupport::Supported { citations } = &mut claims[0].support else {
        unreachable!();
    };
    citations[0].source = source(9);
    assert_eq!(
        policy().assemble(
            &request,
            &GroundingInputAssessment::Sufficient,
            &model_result("model/a", "run/1", ModelResultDisposition::Produced),
            &claims,
        ),
        Err(GroundedAnswerRefusal::CitationNotInContext)
    );
}

#[test]
fn unsupported_claims_and_evidence_dispositions_remain_explicit() {
    let request = fixture_request();
    let mut claims = supported_claims(&request);
    claims[1].support = ProposedClaimSupport::Unsupported {
        rationale: "selected context does not support this clause".into(),
    };
    let partial = policy()
        .assemble(
            &request,
            &GroundingInputAssessment::Sufficient,
            &model_result("model/a", "run/1", ModelResultDisposition::Produced),
            &claims,
        )
        .unwrap();
    assert_eq!(
        partial.disposition,
        GroundedAnswerDisposition::PartiallySupported
    );
    assert!(matches!(
        partial.claims[1].support,
        GroundedClaimSupport::Unsupported { .. }
    ));

    for (assessment, expected) in [
        (
            GroundingInputAssessment::InsufficientEvidence {
                limitation: "origin evidence is absent".into(),
            },
            GroundedAnswerDisposition::InsufficientEvidence,
        ),
        (
            GroundingInputAssessment::ConflictingEvidence {
                limitation: "selected sources disagree".into(),
            },
            GroundedAnswerDisposition::ConflictingEvidence,
        ),
    ] {
        let answer = policy()
            .assemble(
                &request,
                &assessment,
                &model_result("model/a", "run/2", ModelResultDisposition::Produced),
                &claims,
            )
            .unwrap();
        assert_eq!(answer.disposition, expected);
        assert_eq!(answer.limitations.len(), 1);
    }
}

#[test]
fn terminal_model_outcomes_and_request_mismatch_stay_distinct() {
    let request = fixture_request();
    let claims = supported_claims(&request);
    let refused = model_result(
        "model/a",
        "run/refused",
        ModelResultDisposition::Refused(ModelRefusal::ContextUnavailable),
    );
    assert_eq!(
        policy().assemble(
            &request,
            &GroundingInputAssessment::Sufficient,
            &refused,
            &claims
        ),
        Err(GroundedAnswerRefusal::ModelDidNotProduce(
            ModelResultDisposition::Refused(ModelRefusal::ContextUnavailable)
        ))
    );
    let mut mismatched = model_result("model/a", "run/2", ModelResultDisposition::Produced);
    mismatched.request_identity = "request/another".into();
    assert_eq!(
        policy().assemble(
            &request,
            &GroundingInputAssessment::Sufficient,
            &mismatched,
            &claims
        ),
        Err(GroundedAnswerRefusal::ModelRequestMismatch)
    );
}

#[test]
fn forged_context_accounting_and_every_policy_bound_fail_closed() {
    let mut request = fixture_request();
    request.context.used.tokens += 1;
    assert_eq!(
        policy().assemble(
            &request,
            &GroundingInputAssessment::Sufficient,
            &model_result("model/a", "run/1", ModelResultDisposition::Produced),
            &supported_claims(&request),
        ),
        Err(GroundedAnswerRefusal::ContextAccountingMismatch)
    );
    for mutate in [
        |policy: &mut GroundedAnswerPolicy| policy.maximum_output_bytes = 0,
        |policy: &mut GroundedAnswerPolicy| policy.maximum_claims = 0,
        |policy: &mut GroundedAnswerPolicy| policy.maximum_citations = 0,
        |policy: &mut GroundedAnswerPolicy| policy.maximum_work_units = 0,
    ] {
        let request = fixture_request();
        let mut policy = policy();
        mutate(&mut policy);
        assert_eq!(
            policy.assemble(
                &request,
                &GroundingInputAssessment::Sufficient,
                &model_result("model/a", "run/1", ModelResultDisposition::Produced),
                &supported_claims(&request),
            ),
            Err(GroundedAnswerRefusal::InvalidBound)
        );
    }
}
