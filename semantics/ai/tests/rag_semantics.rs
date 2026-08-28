use conduit_ai::{
    AnswerSpan, Candidate, Chunk, ChunkIdentity, ContextBudgetCost, ContextItem, ContextSelection,
    ContextSelectionOutcome, ContextSelectionRationale, ContextTruncationReason, ExtractionLineage,
    GroundedClaim, GroundedResult, GroundingDisposition, ModelResultProvenance, RagSemanticRefusal,
    RetrievalIntent, RetrievalMode, SourceRef, SourceSpan, SourceSpanUnit, TemporalRetrievalIntent,
};
use conduit_core::{
    BoundedResourceRef, KindId, ResourceClassId, ResourceExtent, ResourceLifetime,
    ResourceSemanticIdentity, ResourceVersionIdentity,
};

fn source(version: u8) -> SourceRef {
    SourceRef {
        resource: BoundedResourceRef {
            identity: ResourceSemanticIdentity::from_digest([7; 32]),
            content_profile: KindId::from("document/markdown@1"),
            access_class: ResourceClassId::from("resource/read-authorized@1"),
            extent: ResourceExtent {
                bytes: 1_024,
                items: Some(100),
            },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest([version; 32]),
                expires_at: None,
            },
        },
    }
}

fn intent() -> RetrievalIntent {
    RetrievalIntent {
        identity: "retrieval/project-history".into(),
        modes: vec![
            RetrievalMode::Semantic,
            RetrievalMode::Boundary(TemporalRetrievalIntent::EarliestEvidence),
        ],
        maximum_candidates: 8,
    }
}

fn lineage(version: u8, start: u64, end: u64) -> ExtractionLineage {
    ExtractionLineage {
        source: source(version),
        span: SourceSpan {
            unit: SourceSpanUnit::Bytes,
            start,
            end,
        },
        extraction_profile: "extract/markdown-blocks@1".into(),
        transform_profiles: vec!["transform/normalize-newlines@1".into()],
        parent_chunk: None,
    }
}

fn context() -> ContextSelection<&'static str> {
    ContextSelection {
        items: vec![ContextItem {
            candidate: Candidate {
                chunk: Chunk::new(lineage(3, 10, 40), "source text").unwrap(),
                rank: 1,
                score: None,
                retrieval_basis: "exact temporal boundary candidate".into(),
            },
            rationale: ContextSelectionRationale::BoundaryEvidence,
            budget: ContextBudgetCost {
                bytes: 30,
                tokens: 8,
            },
        }],
        outcome: ContextSelectionOutcome::Complete,
    }
}

#[test]
fn exact_source_version_span_and_transform_lineage_derive_chunk_identity() {
    let base = lineage(3, 10, 40);
    let identity = base.identity().unwrap();
    assert_eq!(Chunk::new(base.clone(), "a").unwrap().identity, identity);

    let mut changed_version = base.clone();
    changed_version.source = source(4);
    assert_ne!(changed_version.identity().unwrap(), identity);
    let mut changed_span = base.clone();
    changed_span.span.end = 41;
    assert_ne!(changed_span.identity().unwrap(), identity);
    let mut changed_transform = base;
    changed_transform
        .transform_profiles
        .push("transform/remove-front-matter@1".into());
    assert_ne!(changed_transform.identity().unwrap(), identity);
}

#[test]
fn temporal_boundary_intent_is_typed_and_bounded() {
    assert_eq!(intent().validate(), Ok(()));
    let mut invalid = intent();
    invalid.modes = vec![RetrievalMode::Temporal(
        TemporalRetrievalIntent::EvidenceWithin { start: 9, end: 2 },
    )];
    assert_eq!(
        invalid.validate(),
        Err(RagSemanticRefusal::InvalidTemporalIntent)
    );
    let mut duplicate = intent();
    duplicate.modes = vec![RetrievalMode::Exact, RetrievalMode::Exact];
    assert_eq!(
        duplicate.validate(),
        Err(RagSemanticRefusal::DuplicateRetrievalMode)
    );
}

#[test]
fn candidates_context_and_citations_remain_distinct_and_exact() {
    let mut context = context();
    context.validate_against(&intent()).unwrap();
    context.outcome = ContextSelectionOutcome::Truncated {
        omitted_candidates: 2,
        reason: ContextTruncationReason::TokenBudget,
    };
    context.validate_against(&intent()).unwrap();
    let citation = conduit_ai::Citation {
        source: context.items[0].candidate.chunk.lineage.source.clone(),
        span: context.items[0].candidate.chunk.lineage.span,
        chunk_identity: context.items[0].candidate.chunk.identity,
    };
    citation.validate_against(&context).unwrap();

    let mut anonymous = citation.clone();
    anonymous.chunk_identity = ChunkIdentity::from_digest([9; 32]);
    assert_eq!(
        anonymous.validate_against(&context),
        Err(RagSemanticRefusal::CitationNotInContext)
    );
    let mut wrong_version = citation;
    wrong_version.source = source(9);
    assert_eq!(
        wrong_version.validate_against(&context),
        Err(RagSemanticRefusal::CitationNotInContext)
    );
}

#[test]
fn grounded_results_are_model_derived_and_citation_fenced() {
    let context = context();
    let citation = conduit_ai::Citation {
        source: context.items[0].candidate.chunk.lineage.source.clone(),
        span: context.items[0].candidate.chunk.lineage.span,
        chunk_identity: context.items[0].candidate.chunk.identity,
    };
    let result = GroundedResult {
        provenance: ModelResultProvenance::ModelDerived,
        answer_kind: "value/text@1".into(),
        answer: b"The project began here.".to_vec(),
        disposition: GroundingDisposition::Supported,
        claims: vec![GroundedClaim {
            answer_span: AnswerSpan { start: 0, end: 23 },
            citation_indices: vec![0],
        }],
        citations: vec![citation],
        limitations: vec![],
    };
    assert_eq!(result.validate_against(&intent(), &context), Ok(()));

    let mut fabricated = result.clone();
    fabricated.citations[0].span.end += 1;
    assert_eq!(
        fabricated.validate_against(&intent(), &context),
        Err(RagSemanticRefusal::CitationNotInContext)
    );
}

#[test]
fn insufficient_and_conflicting_evidence_are_first_class() {
    for disposition in [
        GroundingDisposition::InsufficientEvidence,
        GroundingDisposition::ConflictingEvidence,
    ] {
        let result = GroundedResult {
            provenance: ModelResultProvenance::ModelDerived,
            answer_kind: "value/text@1".into(),
            answer: b"No supported conclusion.".to_vec(),
            disposition,
            claims: vec![],
            citations: vec![],
            limitations: vec!["required boundary evidence is missing or conflicts".into()],
        };
        assert_eq!(result.validate_against(&intent(), &context()), Ok(()));
    }
}

#[test]
fn malformed_spans_ranks_budgets_and_claims_fail_closed() {
    let mut invalid_lineage = lineage(3, 40, 40);
    assert_eq!(
        invalid_lineage.identity(),
        Err(RagSemanticRefusal::EmptySpan)
    );
    invalid_lineage.span = SourceSpan {
        unit: SourceSpanUnit::Items,
        start: 99,
        end: 101,
    };
    assert_eq!(
        invalid_lineage.identity(),
        Err(RagSemanticRefusal::SpanOutsideSource)
    );

    let mut context = context();
    context.items[0].candidate.rank = 0;
    assert_eq!(
        context.validate_against(&intent()),
        Err(RagSemanticRefusal::RankZero)
    );
    context.items[0].candidate.rank = 1;
    context.items[0].budget = ContextBudgetCost {
        bytes: 0,
        tokens: 0,
    };
    assert_eq!(
        context.validate_against(&intent()),
        Err(RagSemanticRefusal::EmptyBudget)
    );

    context.items[0].budget = ContextBudgetCost {
        bytes: 30,
        tokens: 8,
    };
    context.outcome = ContextSelectionOutcome::Truncated {
        omitted_candidates: 0,
        reason: ContextTruncationReason::TokenBudget,
    };
    assert_eq!(
        context.validate_against(&intent()),
        Err(RagSemanticRefusal::EmptyTruncation)
    );
}

#[test]
fn contract_contains_no_prompt_provider_or_backend_identity() {
    let source = include_str!("../src/rag_semantics.rs");
    for forbidden in [
        "OpenAI",
        "Anthropic",
        "Pinecone",
        "Postgres",
        "prompt_template",
    ] {
        assert!(!source.contains(forbidden));
    }
}
