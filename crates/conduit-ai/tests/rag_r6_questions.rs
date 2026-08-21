use conduit_ai::{
    AnswerSpan, Chunk, Citation, ClockBasis, ContextCandidate, ContextOrderingPolicy,
    ContextRedundancyPolicy, ContextSelectionDisposition, ContextSelectionPolicy,
    ContextTemporalEvidence, EntityBoundary, ExtractedSourceValue, ExtractionLineage,
    FusionStrategy, GroundedAnswer, GroundedAnswerDisposition, GroundedAnswerPolicy,
    GroundedAnswerRequest, GroundingInputAssessment, HybridCandidate, HybridFusionPolicy,
    HybridRetrievalOutcome, LlmDeterminismProfile, MechanismScore, ModelDerivedResult,
    ModelResultDisposition, ModelResultProvenance, ModelWorkAccounting, ProposedClaimSupport,
    ProposedGroundedClaim, RerankingPolicy, RerankingReceipt, RerankingStrategy,
    RetrievalMechanism, RetrievalMode, RetrievalStage, RetrieverIdentity, SourceRef, SourceSpan,
    SourceSpanUnit, StageCandidate, StructuredContext, TemporalContext, TemporalEvidenceBatch,
    TemporalEvidenceCandidate, TemporalProvenance, TemporalReference, TemporalRetrievalIntent,
    TemporalSource, TemporalValidity,
};
use conduit_core::{
    BoundedResourceRef, KindId, ResourceClassId, ResourceExtent, ResourceLifetime,
    ResourceSemanticIdentity, ResourceVersionIdentity,
};

const DECISION_AT: u64 = 1_000;

#[derive(Clone)]
struct Evidence {
    label: &'static str,
    family: &'static str,
    chunk: Chunk<ExtractedSourceValue>,
    provenance: TemporalProvenance,
    validity: TemporalValidity,
    boundary: Option<EntityBoundary>,
}

struct QueryCase {
    identity: &'static str,
    modes: Vec<RetrievalMode>,
    hard_filter: Option<TemporalRetrievalIntent>,
    evidence: Vec<Evidence>,
    mechanisms: Vec<RetrievalMechanism>,
    maximum_items: u16,
    answer: &'static str,
    sufficient: bool,
}

struct RetrievalExplanation {
    intent: conduit_ai::RetrievalIntent,
    stages: Vec<RetrievalStage<ExtractedSourceValue>>,
    temporal_filter: Option<TemporalRetrievalIntent>,
    temporal_evidence: TemporalEvidenceBatch,
    metadata_filters: Vec<String>,
    fused: Vec<HybridCandidate<ExtractedSourceValue>>,
    reranking: RerankingReceipt,
    context: StructuredContext,
    answer: GroundedAnswer,
}

fn evidence(
    id: u8,
    version: u8,
    label: &'static str,
    family: &'static str,
    text: &'static str,
    temporal: (u64, TemporalValidity, Option<u64>, Option<EntityBoundary>),
) -> Evidence {
    let (event_at, validity, valid_until, boundary) = temporal;
    let value = if family == "catalog" {
        ExtractedSourceValue::StructuredItems(vec![text.as_bytes().to_vec()])
    } else {
        ExtractedSourceValue::Text(text.as_bytes().to_vec())
    };
    let span_unit = if family == "catalog" {
        SourceSpanUnit::Items
    } else {
        SourceSpanUnit::Bytes
    };
    let span_end = if family == "catalog" {
        1
    } else {
        text.len() as u64
    };
    let chunk = Chunk::new(
        ExtractionLineage {
            source: SourceRef {
                resource: BoundedResourceRef {
                    identity: ResourceSemanticIdentity::from_digest([id; 32]),
                    content_profile: KindId::from(format!("source/{family}@1")),
                    access_class: ResourceClassId::from("resource/read-authorized@1"),
                    extent: ResourceExtent {
                        bytes: text.len() as u64,
                        items: (family == "catalog").then_some(1),
                    },
                    lifetime: ResourceLifetime {
                        version: ResourceVersionIdentity::from_digest([version; 32]),
                        expires_at: None,
                    },
                },
            },
            span: SourceSpan {
                unit: span_unit,
                start: 0,
                end: span_end,
            },
            extraction_profile: format!("extract/{family}@1"),
            transform_profiles: vec![],
            parent_chunk: None,
        },
        value,
    )
    .unwrap();
    Evidence {
        label,
        family,
        chunk,
        provenance: TemporalProvenance {
            event_at: Some(event_at),
            valid_from: (validity != TemporalValidity::UnknownWhetherCurrent).then_some(event_at),
            valid_until,
            observed_at: Some(event_at + 1),
            recorded_at: Some(event_at + 2),
            ingested_at: Some(event_at + 3),
            retrieved_at: 990,
            reference_at: DECISION_AT,
            clock_basis: ClockBasis::UnixEpochMilliseconds,
            uncertainty_millis: None,
        },
        validity,
        boundary,
    }
}

fn stages(case: &QueryCase, vector_identity: &str) -> Vec<RetrievalStage<ExtractedSourceValue>> {
    case.mechanisms
        .iter()
        .map(|mechanism| {
            let identity = match mechanism {
                RetrievalMechanism::VectorSimilarity => vector_identity.to_string(),
                RetrievalMechanism::Lexical => "retriever/lexical@1".into(),
                RetrievalMechanism::Metadata => "retriever/metadata@1".into(),
                RetrievalMechanism::Temporal => "retriever/temporal@1".into(),
                RetrievalMechanism::DomainExact => "retriever/domain-exact@1".into(),
            };
            let candidates = case
                .evidence
                .iter()
                .enumerate()
                .map(|(index, item)| StageCandidate {
                    chunk: item.chunk.clone(),
                    rank: u16::try_from(index + 1).unwrap(),
                    score: Some(match mechanism {
                        RetrievalMechanism::VectorSimilarity => {
                            MechanismScore::SimilarityMicros(990_000 - index as i64)
                        }
                        RetrievalMechanism::Lexical => {
                            MechanismScore::LexicalScore(900 - index as u32)
                        }
                        RetrievalMechanism::Metadata => MechanismScore::MetadataMatch,
                        RetrievalMechanism::Temporal => MechanismScore::TemporalBoundary,
                        RetrievalMechanism::DomainExact => MechanismScore::ExactMatch,
                    }),
                    temporal_evidence_identity: (*mechanism == RetrievalMechanism::Temporal)
                        .then(|| item.label.to_string()),
                })
                .collect::<Vec<_>>();
            RetrievalStage {
                retriever: RetrieverIdentity {
                    identity,
                    mechanism: *mechanism,
                },
                work_units: candidates.len() as u32,
                candidates,
            }
        })
        .collect()
}

fn temporal_evidence(case: &QueryCase) -> TemporalEvidenceBatch {
    TemporalEvidenceBatch {
        reference: TemporalReference {
            reference_at: DECISION_AT,
            clock_basis: ClockBasis::UnixEpochMilliseconds,
        },
        candidates: case
            .evidence
            .iter()
            .map(|item| TemporalEvidenceCandidate {
                identity: item.label.into(),
                provenance: item.provenance.clone(),
                source: TemporalSource::Event,
                boundary: item.boundary,
                transition: None,
                validity: item.validity,
            })
            .collect(),
        earliest_history_complete: true,
    }
}

fn execute(case: &QueryCase, model_identity: &str, vector_identity: &str) -> RetrievalExplanation {
    let intent = conduit_ai::RetrievalIntent {
        identity: case.identity.into(),
        modes: case.modes.clone(),
        maximum_candidates: 16,
    };
    intent.validate().unwrap();
    let stages = stages(case, vector_identity);
    let temporal_evidence = temporal_evidence(case);
    let outcome = HybridFusionPolicy {
        identity: "fusion/r6-explicit-rrf@1".into(),
        strategy: FusionStrategy::ReciprocalRank { rank_constant: 60 },
        required_mechanisms: case.mechanisms.clone(),
        temporal_hard_filter: case.hard_filter.clone(),
        maximum_candidates_per_stage: 16,
        maximum_output_candidates: 16,
        maximum_total_work_units: 128,
    }
    .fuse(&stages, Some(&temporal_evidence))
    .unwrap();
    let HybridRetrievalOutcome::Candidates(fused) = outcome else {
        panic!("complete fixture history must resolve the boundary")
    };
    let reranking = RerankingPolicy {
        identity: "rerank/preserve-hybrid-deterministic@1".into(),
        strategy: RerankingStrategy::PreserveHybridFusion,
        maximum_candidates: 16,
        maximum_work_units: 128,
    }
    .rerank(&fused, &[])
    .unwrap();
    let candidates = reranking
        .candidates
        .iter()
        .map(|candidate| {
            let source = case
                .evidence
                .iter()
                .find(|item| item.chunk.identity == candidate.candidate.chunk.identity)
                .unwrap();
            ContextCandidate {
                reranked: candidate.clone(),
                reranking_policy_identity: reranking.policy_identity.clone(),
                reranking_proof_class: reranking.proof_class,
                temporal: Some(ContextTemporalEvidence {
                    evidence_identity: source.label.into(),
                    provenance: source.provenance.clone(),
                    source: TemporalSource::Event,
                    boundary: source.boundary,
                    context: TemporalContext {
                        source: TemporalSource::Event,
                        relation: source.provenance.relation(TemporalSource::Event).unwrap(),
                        validity: source.validity,
                        relation_to_query_window: None,
                    },
                }),
                redundancy_group: Some(format!("source/{}", source.label)),
                token_count: 8,
            }
        })
        .collect::<Vec<_>>();
    let context = ContextSelectionPolicy {
        identity: "context/r6-bounded@1".into(),
        token_accounting_profile: "tokens/exact-fixture@1".into(),
        redundancy: ContextRedundancyPolicy::KeepAll,
        ordering: ContextOrderingPolicy::ChronologicalOldestFirst,
        maximum_items: case.maximum_items,
        maximum_bytes: 16_384,
        maximum_tokens: 4_096,
        maximum_work_units: 64,
    }
    .select(&candidates)
    .unwrap();
    let request = GroundedAnswerRequest {
        identity: format!("request/{}", case.identity),
        retrieval_intent: intent.clone(),
        context: context.clone(),
    };
    let payload = case.answer.as_bytes().to_vec();
    let model = ModelDerivedResult {
        provenance: ModelResultProvenance::ModelDerived,
        payload_kind: "llm/generated-result@1".into(),
        payload: payload.clone(),
        implementation_identity: model_identity.into(),
        request_identity: request.identity.clone(),
        run_identity: format!("run/{model_identity}/{}", case.identity),
        confidence: None,
        disposition: ModelResultDisposition::Produced,
        determinism: LlmDeterminismProfile::ProviderNondeterministic,
        accounting: ModelWorkAccounting {
            input_bytes: 256,
            context_items: context.items.len() as u64,
            output_bytes: payload.len() as u64,
            work_units: 32,
            history_items: 0,
        },
    };
    let claims = if case.sufficient {
        let citations = context
            .items
            .iter()
            .map(|item| Citation {
                source: item.reranked.candidate.chunk.lineage.source.clone(),
                span: item.reranked.candidate.chunk.lineage.span,
                chunk_identity: item.reranked.candidate.chunk.identity,
            })
            .collect();
        vec![ProposedGroundedClaim {
            answer_span: AnswerSpan {
                start: 0,
                end: payload.len() as u32,
            },
            support: ProposedClaimSupport::Supported { citations },
        }]
    } else {
        vec![ProposedGroundedClaim {
            answer_span: AnswerSpan {
                start: 0,
                end: payload.len() as u32,
            },
            support: ProposedClaimSupport::Unsupported {
                rationale: "The omitted source is required to answer exactly.".into(),
            },
        }]
    };
    let assessment = if case.sufficient {
        GroundingInputAssessment::Sufficient
    } else {
        GroundingInputAssessment::InsufficientEvidence {
            limitation: "Finite context omitted required current evidence.".into(),
        }
    };
    let answer = GroundedAnswerPolicy {
        identity: "grounding/exact-context-citations@1".into(),
        answer_kind: "value/text-utf8@1".into(),
        maximum_output_bytes: 16_384,
        maximum_claims: 16,
        maximum_citations: 32,
        maximum_work_units: 4_096,
    }
    .assemble(&request, &assessment, &model, &claims)
    .unwrap();
    RetrievalExplanation {
        intent,
        stages,
        temporal_filter: case.hard_filter.clone(),
        temporal_evidence,
        metadata_filters: vec!["authority=admitted".into(), "corpus=project-history".into()],
        fused,
        reranking,
        context,
        answer,
    }
}

#[rustfmt::skip]
fn cases() -> Vec<QueryCase> {
    vec![
        QueryCase {
            identity: "query/remaining",
            modes: vec![RetrievalMode::Exact, RetrievalMode::Metadata],
            hard_filter: None,
            evidence: vec![evidence(1, 3, "issue/1424", "repository", "R6 remains on the RAG epic.", (900, TemporalValidity::Current, None, None))],
            mechanisms: vec![RetrievalMechanism::Lexical, RetrievalMechanism::Metadata, RetrievalMechanism::Temporal],
            maximum_items: 4,
            answer: "R6 remains to be accepted.",
            sufficient: true,
        },
        QueryCase {
            identity: "query/latest",
            modes: vec![RetrievalMode::Temporal(TemporalRetrievalIntent::LatestEvidence)],
            hard_filter: Some(TemporalRetrievalIntent::LatestEvidence),
            evidence: vec![
                evidence(2, 1, "commit/old", "repository", "R4 merged.", (700, TemporalValidity::Historical, Some(799), None)),
                evidence(3, 1, "commit/latest", "repository", "R5 merged.", (800, TemporalValidity::Current, None, Some(EntityBoundary::LastChanged))),
            ],
            mechanisms: vec![RetrievalMechanism::VectorSimilarity, RetrievalMechanism::Lexical, RetrievalMechanism::Temporal],
            maximum_items: 4,
            answer: "R5 conformance merged most recently.",
            sufficient: true,
        },
        QueryCase {
            identity: "query/duration",
            modes: vec![RetrievalMode::Boundary(TemporalRetrievalIntent::DurationSince { boundary: EntityBoundary::Created })],
            hard_filter: Some(TemporalRetrievalIntent::DurationSince {
                boundary: EntityBoundary::Created,
            }),
            evidence: vec![
                evidence(4, 9, "summary/recent", "summary", "Recent summary of the RAG work.", (950, TemporalValidity::Current, None, None)),
                evidence(5, 1, "docs/origin", "documentation", "The project began here.", (100, TemporalValidity::Historical, None, Some(EntityBoundary::Created))),
            ],
            mechanisms: vec![RetrievalMechanism::VectorSimilarity, RetrievalMechanism::Lexical, RetrievalMechanism::Metadata, RetrievalMechanism::Temporal],
            maximum_items: 4,
            answer: "The project has been active for 900 milliseconds.",
            sufficient: true,
        },
        QueryCase {
            identity: "query/historical",
            modes: vec![RetrievalMode::Temporal(TemporalRetrievalIntent::StateValidAt { instant: 250 })],
            hard_filter: Some(TemporalRetrievalIntent::StateValidAt { instant: 250 }),
            evidence: vec![
                evidence(6, 1, "sign/state-a", "sign", "R2 was active.", (200, TemporalValidity::Historical, Some(299), None)),
                evidence(7, 1, "sign/state-b", "sign", "R3 is active.", (300, TemporalValidity::Current, None, None)),
            ],
            mechanisms: vec![RetrievalMechanism::Metadata, RetrievalMechanism::Temporal],
            maximum_items: 4,
            answer: "At instant 250, R2 was active.",
            sufficient: true,
        },
        QueryCase {
            identity: "query/stale",
            modes: vec![RetrievalMode::Metadata, RetrievalMode::Temporal(TemporalRetrievalIntent::LatestEvidence)],
            hard_filter: None,
            evidence: vec![
                evidence(8, 1, "status/current", "sign", "R5 is accepted.", (800, TemporalValidity::Current, None, None)),
                evidence(9, 1, "summary/stale", "summary", "R4 is the latest accepted stage.", (650, TemporalValidity::Superseded, Some(799), None)),
                evidence(10, 1, "catalog/unknown", "catalog", "safety_class=unknown", (750, TemporalValidity::UnknownWhetherCurrent, None, None)),
            ],
            mechanisms: vec![RetrievalMechanism::Lexical, RetrievalMechanism::Metadata, RetrievalMechanism::Temporal],
            maximum_items: 2,
            answer: "The old summary is stale; catalog currency is unknown.",
            sufficient: false,
        },
        QueryCase {
            identity: "query/cross-domain",
            modes: vec![RetrievalMode::Exact, RetrievalMode::Metadata, RetrievalMode::Temporal(TemporalRetrievalIntent::EvidenceWithin { start: 850, end: 950 })],
            hard_filter: None,
            evidence: vec![
                evidence(11, 1, "calendar/demo", "calendar", "Demo at 900 requires reduced-safe motion.", (900, TemporalValidity::Current, None, None)),
                evidence(12, 1, "catalog/create", "catalog", "Create motion offers reduced-safe.", (850, TemporalValidity::Current, None, None)),
            ],
            mechanisms: vec![RetrievalMechanism::Metadata, RetrievalMechanism::Temporal, RetrievalMechanism::DomainExact],
            maximum_items: 4,
            answer: "The demo at 900 can use the Create reduced-safe motion offer.",
            sufficient: true,
        },
    ]
}

#[test]
#[rustfmt::skip]
fn all_six_query_classes_retain_bounded_machine_readable_explanations() {
    let receipts = cases().iter().map(|case| execute(case, "model/local-a@1", "vector/index-a@7")).collect::<Vec<_>>();
    assert_eq!(receipts.len(), 6);
    for receipt in &receipts {
        assert!(!receipt.intent.modes.is_empty());
        assert!(!receipt.stages.is_empty());
        assert!(!receipt.metadata_filters.is_empty());
        assert!(!receipt.fused.is_empty());
        assert!(!receipt.reranking.candidates.is_empty());
        assert!(!receipt.context.items.is_empty());
        assert_eq!(receipt.answer.request_identity, format!("request/{}", receipt.intent.identity));
        for citation in &receipt.answer.citations {
            assert!(receipt.context.items.iter().any(|item| {
                let chunk = &item.reranked.candidate.chunk;
                citation.chunk_identity == chunk.identity
                    && citation.source == chunk.lineage.source
                    && citation.span == chunk.lineage.span
            }));
        }
    }
    assert_eq!(receipts[2].temporal_filter, Some(TemporalRetrievalIntent::DurationSince { boundary: EntityBoundary::Created }));
    assert_eq!(receipts[2].context.items[0].temporal.as_ref().unwrap().evidence_identity, "docs/origin");
    assert_eq!(receipts[2].context.items[0].temporal.as_ref().unwrap().provenance.age(TemporalSource::Event), Ok(900));
    assert_eq!(receipts[3].context.items[0].temporal.as_ref().unwrap().context.validity, TemporalValidity::Historical);
    assert!(receipts[4].temporal_evidence.candidates.iter().any(|item| item.validity == TemporalValidity::Current));
    assert!(receipts[4].temporal_evidence.candidates.iter().any(|item| item.validity == TemporalValidity::Superseded));
    assert!(receipts[4].context.items.iter().any(|item| item.temporal.as_ref().unwrap().context.validity == TemporalValidity::UnknownWhetherCurrent));
    assert!(matches!(receipts[4].context.disposition, ContextSelectionDisposition::Omitted { .. }));
    assert_eq!(receipts[4].answer.disposition, GroundedAnswerDisposition::InsufficientEvidence);
    assert!(!receipts[5].stages.iter().any(|stage| stage.retriever.mechanism == RetrievalMechanism::VectorSimilarity));

    let families = cases().into_iter().flat_map(|case| case.evidence).map(|item| item.family).collect::<Vec<_>>();
    for family in ["repository", "documentation", "sign", "summary", "calendar", "catalog"] {
        assert!(families.contains(&family));
    }
}

#[test]
#[rustfmt::skip]
fn model_and_vector_realization_swaps_preserve_grounded_semantics() {
    let case = &cases()[1];
    let first = execute(case, "model/local-a@1", "vector/index-a@7");
    let second = execute(case, "model/hosted-b@3", "vector/index-b@11");
    assert_eq!(first.answer.answer, second.answer.answer);
    assert_eq!(first.answer.disposition, second.answer.disposition);
    assert_eq!(first.answer.citations, second.answer.citations);
    assert_ne!(first.answer.model_implementation_identity, second.answer.model_implementation_identity);
    assert_ne!(first.stages[0].retriever.identity, second.stages[0].retriever.identity);
    assert_eq!(first.context.items.iter().map(|item| item.reranked.candidate.chunk.identity).collect::<Vec<_>>(), second.context.items.iter().map(|item| item.reranked.candidate.chunk.identity).collect::<Vec<_>>());
}
