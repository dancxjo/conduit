use conduit_ai::{
    Chunk, ClockBasis, EntityBoundary, ExtractionLineage, FusionStrategy, HybridFusionPolicy,
    HybridRetrievalOutcome, HybridRetrievalRefusal, MechanismScore, RetrievalMechanism,
    RetrievalStage, RetrieverIdentity, SourceRef, SourceSpan, SourceSpanUnit, StageCandidate,
    TemporalEvidenceBatch, TemporalEvidenceCandidate, TemporalProvenance, TemporalReference,
    TemporalRetrievalIntent, TemporalSource, TemporalValidity,
};
use conduit_core::{
    BoundedResourceRef, KindId, ResourceClassId, ResourceExtent, ResourceLifetime,
    ResourceSemanticIdentity, ResourceVersionIdentity,
};

fn chunk(source_identity: u8, version: u8, start: u64, value: &str) -> Chunk<String> {
    Chunk::new(
        ExtractionLineage {
            source: SourceRef {
                resource: BoundedResourceRef {
                    identity: ResourceSemanticIdentity::from_digest([source_identity; 32]),
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
            },
            span: SourceSpan {
                unit: SourceSpanUnit::Bytes,
                start,
                end: start + 20,
            },
            extraction_profile: "extract/text-utf8@1".into(),
            transform_profiles: vec![],
            parent_chunk: None,
        },
        value.into(),
    )
    .unwrap()
}

fn candidate(
    chunk: &Chunk<String>,
    rank: u16,
    score: Option<MechanismScore>,
    temporal: Option<&str>,
) -> StageCandidate<String> {
    StageCandidate {
        chunk: chunk.clone(),
        rank,
        score,
        temporal_evidence_identity: temporal.map(Into::into),
    }
}

fn stage(
    identity: &str,
    mechanism: RetrievalMechanism,
    candidates: Vec<StageCandidate<String>>,
) -> RetrievalStage<String> {
    RetrievalStage {
        retriever: RetrieverIdentity {
            identity: identity.into(),
            mechanism,
        },
        work_units: candidates.len() as u32,
        candidates,
    }
}

fn policy(temporal: bool) -> HybridFusionPolicy {
    HybridFusionPolicy {
        identity: if temporal {
            "fusion/rrf-with-hard-origin@1"
        } else {
            "fusion/rrf@1"
        }
        .into(),
        strategy: FusionStrategy::ReciprocalRank { rank_constant: 60 },
        required_mechanisms: vec![
            RetrievalMechanism::VectorSimilarity,
            RetrievalMechanism::Lexical,
            RetrievalMechanism::Metadata,
            RetrievalMechanism::Temporal,
        ],
        temporal_hard_filter: temporal.then_some(TemporalRetrievalIntent::DurationSince {
            boundary: EntityBoundary::Created,
        }),
        maximum_candidates_per_stage: 8,
        maximum_output_candidates: 8,
        maximum_total_work_units: 32,
    }
}

fn provenance(event_at: u64) -> TemporalProvenance {
    TemporalProvenance {
        event_at: Some(event_at),
        valid_from: Some(event_at),
        valid_until: None,
        observed_at: Some(event_at + 1),
        recorded_at: Some(event_at + 2),
        ingested_at: Some(event_at + 3),
        retrieved_at: 900,
        reference_at: 1_000,
        clock_basis: ClockBasis::UnixEpochMilliseconds,
        uncertainty_millis: None,
    }
}

fn evidence(complete: bool) -> TemporalEvidenceBatch {
    TemporalEvidenceBatch {
        reference: TemporalReference {
            reference_at: 1_000,
            clock_basis: ClockBasis::UnixEpochMilliseconds,
        },
        candidates: vec![
            TemporalEvidenceCandidate {
                identity: "summary/recent".into(),
                provenance: provenance(900),
                source: TemporalSource::Event,
                boundary: None,
                transition: None,
                validity: TemporalValidity::Current,
            },
            TemporalEvidenceCandidate {
                identity: "project/created".into(),
                provenance: provenance(100),
                source: TemporalSource::Event,
                boundary: Some(EntityBoundary::Created),
                transition: None,
                validity: TemporalValidity::Historical,
            },
        ],
        earliest_history_complete: complete,
    }
}

fn combined_stages() -> Vec<RetrievalStage<String>> {
    let recent = chunk(7, 3, 40, "recent summary");
    let origin = chunk(7, 1, 0, "project origin");
    vec![
        stage(
            "retriever/vector-exact@1",
            RetrievalMechanism::VectorSimilarity,
            vec![
                candidate(
                    &recent,
                    1,
                    Some(MechanismScore::SimilarityMicros(990_000)),
                    None,
                ),
                candidate(
                    &origin,
                    2,
                    Some(MechanismScore::SimilarityMicros(210_000)),
                    None,
                ),
            ],
        ),
        stage(
            "retriever/lexical-project@1",
            RetrievalMechanism::Lexical,
            vec![
                candidate(&recent, 1, Some(MechanismScore::LexicalScore(800)), None),
                candidate(&origin, 2, Some(MechanismScore::LexicalScore(200)), None),
            ],
        ),
        stage(
            "retriever/metadata-project@1",
            RetrievalMechanism::Metadata,
            vec![candidate(
                &origin,
                1,
                Some(MechanismScore::MetadataMatch),
                None,
            )],
        ),
        stage(
            "retriever/temporal-boundary@1",
            RetrievalMechanism::Temporal,
            vec![
                candidate(
                    &origin,
                    1,
                    Some(MechanismScore::TemporalBoundary),
                    Some("project/created"),
                ),
                candidate(
                    &recent,
                    2,
                    Some(MechanismScore::TemporalBoundary),
                    Some("summary/recent"),
                ),
            ],
        ),
    ]
}

#[test]
fn vector_lexical_metadata_and_temporal_paths_fuse_with_exact_provenance() {
    let result = policy(true)
        .fuse(&combined_stages(), Some(&evidence(true)))
        .unwrap();
    let HybridRetrievalOutcome::Candidates(candidates) = result else {
        panic!("complete boundary history must produce candidates");
    };
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].chunk.value, "project origin");
    assert_eq!(candidates[0].rank, 1);
    assert_eq!(candidates[0].contributions.len(), 4);
    assert!(candidates[0]
        .contributions
        .iter()
        .any(|path| path.retriever.mechanism == RetrievalMechanism::VectorSimilarity));
    assert!(candidates[0]
        .contributions
        .iter()
        .any(|path| path.temporal_evidence_identity.as_deref() == Some("project/created")));
}

#[test]
fn recent_semantic_page_cannot_masquerade_as_historical_origin() {
    let mut recent_only = evidence(false);
    recent_only
        .candidates
        .retain(|candidate| candidate.identity == "summary/recent");
    assert_eq!(
        policy(true).fuse(&combined_stages(), Some(&recent_only)),
        Ok(HybridRetrievalOutcome::NeedEarlierHistory)
    );
}

#[test]
fn provider_scores_remain_local_and_do_not_define_fusion_order() {
    let mut policy = policy(false);
    policy.required_mechanisms = vec![
        RetrievalMechanism::VectorSimilarity,
        RetrievalMechanism::Lexical,
    ];
    let stages = &combined_stages()[..2];
    let first = policy.fuse(stages, None).unwrap();
    let second = policy.fuse(stages, None).unwrap();
    assert_eq!(first, second);
    let HybridRetrievalOutcome::Candidates(candidates) = first else {
        panic!("non-temporal fusion must produce candidates");
    };
    assert_eq!(candidates[0].chunk.value, "recent summary");
    assert_eq!(candidates[0].contributions.len(), 2);
    assert_ne!(
        candidates[0].contributions[0].score,
        candidates[0].contributions[1].score
    );
}

#[test]
fn deduplication_preserves_changed_source_versions_as_distinct_truth() {
    let old = chunk(7, 1, 0, "same text");
    let changed = chunk(7, 2, 0, "same text");
    let stages = vec![
        stage(
            "retriever/vector@1",
            RetrievalMechanism::VectorSimilarity,
            vec![
                candidate(&old, 1, None, None),
                candidate(&changed, 2, None, None),
            ],
        ),
        stage(
            "retriever/lexical@1",
            RetrievalMechanism::Lexical,
            vec![
                candidate(&old, 1, None, None),
                candidate(&changed, 2, None, None),
            ],
        ),
    ];
    let mut policy = policy(false);
    policy.required_mechanisms = vec![
        RetrievalMechanism::VectorSimilarity,
        RetrievalMechanism::Lexical,
    ];
    let HybridRetrievalOutcome::Candidates(candidates) = policy.fuse(&stages, None).unwrap() else {
        panic!("fusion must produce candidates");
    };
    assert_eq!(candidates.len(), 2);
    assert_ne!(candidates[0].chunk.identity, candidates[1].chunk.identity);
    assert!(candidates
        .iter()
        .all(|candidate| candidate.contributions.len() == 2));
}

#[test]
fn required_mechanisms_stage_bounds_and_work_pressure_fail_closed() {
    let stages = combined_stages();
    let mut missing = policy(false);
    missing
        .required_mechanisms
        .push(RetrievalMechanism::DomainExact);
    assert_eq!(
        missing.fuse(&stages, None),
        Err(HybridRetrievalRefusal::MissingRequiredMechanism)
    );

    let mut candidate_pressure = policy(false);
    candidate_pressure.maximum_candidates_per_stage = 1;
    assert_eq!(
        candidate_pressure.fuse(&stages, None),
        Err(HybridRetrievalRefusal::StageCandidateLimitExceeded)
    );

    let mut work_pressure = policy(false);
    work_pressure.maximum_total_work_units = 3;
    assert_eq!(
        work_pressure.fuse(&stages, None),
        Err(HybridRetrievalRefusal::WorkBoundExceeded)
    );
}

#[test]
fn malformed_ranks_duplicate_stage_chunks_and_temporal_identity_leaks_refuse() {
    let origin = chunk(7, 1, 0, "origin");
    let mut zero_rank = combined_stages();
    zero_rank[0].candidates[0].rank = 0;
    assert_eq!(
        policy(false).fuse(&zero_rank, None),
        Err(HybridRetrievalRefusal::RankZero)
    );

    let duplicate = vec![stage(
        "retriever/vector@1",
        RetrievalMechanism::VectorSimilarity,
        vec![
            candidate(&origin, 1, None, None),
            candidate(&origin, 2, None, None),
        ],
    )];
    let mut one = policy(false);
    one.required_mechanisms = vec![RetrievalMechanism::VectorSimilarity];
    assert_eq!(
        one.fuse(&duplicate, None),
        Err(HybridRetrievalRefusal::DuplicateChunkInStage)
    );

    let leaked = vec![stage(
        "retriever/vector@1",
        RetrievalMechanism::VectorSimilarity,
        vec![candidate(&origin, 1, None, Some("provider/document-7"))],
    )];
    assert_eq!(
        one.fuse(&leaked, None),
        Err(HybridRetrievalRefusal::UnexpectedTemporalEvidenceIdentity)
    );
}
