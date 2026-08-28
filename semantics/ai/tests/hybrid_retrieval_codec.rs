use conduit_ai::{
    Chunk, ExtractedSourceValue, ExtractionLineage, FusionStrategy, HybridFusionPolicy,
    HybridRetrievalCodecRefusal, HybridRetrievalOutcome, HybridRetrievalReceipt, MechanismScore,
    RetrievalMechanism, RetrievalStage, RetrieverIdentity, SourceRef, SourceSpan, SourceSpanUnit,
    StageCandidate, MAXIMUM_HYBRID_BATCH_BYTES,
};
use conduit_core::{
    BoundedResourceRef, KindId, ResourceClassId, ResourceExtent, ResourceLifetime,
    ResourceSemanticIdentity, ResourceVersionIdentity,
};

fn chunk(version: u8, start: u64, text: &str) -> Chunk<ExtractedSourceValue> {
    Chunk::new(
        ExtractionLineage {
            source: SourceRef {
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
            },
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
    .unwrap()
}

fn stage(
    identity: &str,
    mechanism: RetrievalMechanism,
    chunk: Chunk<ExtractedSourceValue>,
    score: MechanismScore,
    temporal: Option<&str>,
) -> RetrievalStage<ExtractedSourceValue> {
    RetrievalStage {
        retriever: RetrieverIdentity {
            identity: identity.into(),
            mechanism,
        },
        candidates: vec![StageCandidate {
            chunk,
            rank: 1,
            score: Some(score),
            temporal_evidence_identity: temporal.map(Into::into),
        }],
        work_units: 1,
    }
}

fn stages() -> Vec<RetrievalStage<ExtractedSourceValue>> {
    let shared = chunk(1, 0, "project origin");
    vec![
        stage(
            "retriever/vector@1",
            RetrievalMechanism::VectorSimilarity,
            shared.clone(),
            MechanismScore::SimilarityMicros(900_000),
            None,
        ),
        stage(
            "retriever/lexical@1",
            RetrievalMechanism::Lexical,
            shared.clone(),
            MechanismScore::LexicalScore(42),
            None,
        ),
        stage(
            "retriever/metadata@1",
            RetrievalMechanism::Metadata,
            shared.clone(),
            MechanismScore::MetadataMatch,
            None,
        ),
        stage(
            "retriever/temporal@1",
            RetrievalMechanism::Temporal,
            shared,
            MechanismScore::TemporalBoundary,
            Some("project/created"),
        ),
    ]
}

fn policy() -> HybridFusionPolicy {
    HybridFusionPolicy {
        identity: "fusion/reciprocal-rank@1".into(),
        strategy: FusionStrategy::ReciprocalRank { rank_constant: 60 },
        required_mechanisms: vec![
            RetrievalMechanism::VectorSimilarity,
            RetrievalMechanism::Lexical,
            RetrievalMechanism::Metadata,
            RetrievalMechanism::Temporal,
        ],
        temporal_hard_filter: None,
        maximum_candidates_per_stage: 8,
        maximum_output_candidates: 8,
        maximum_total_work_units: 32,
    }
}

#[test]
fn every_stage_and_fused_provenance_round_trip_canonically() {
    let stages = stages();
    for stage in &stages {
        let encoded = stage.encode().unwrap();
        let decoded = RetrievalStage::decode(&encoded).unwrap();
        assert_eq!(&decoded, stage);
        assert_eq!(decoded.encode().unwrap(), encoded);
    }

    let outcome = policy().fuse(&stages, None).unwrap();
    let receipt = HybridRetrievalReceipt {
        policy_identity: policy().identity,
        outcome,
    };
    let encoded = receipt.encode().unwrap();
    let decoded = HybridRetrievalReceipt::decode(&encoded).unwrap();
    assert_eq!(decoded, receipt);
    assert_eq!(decoded.encode().unwrap(), encoded);
    let HybridRetrievalOutcome::Candidates(candidates) = decoded.outcome else {
        panic!("four complete stages must produce candidates");
    };
    assert_eq!(candidates[0].contributions.len(), 4);
}

#[test]
fn malformed_trailing_oversized_and_zero_rank_values_fail_closed() {
    let stage = stages().remove(0);
    let mut trailing = stage.encode().unwrap();
    trailing.push(0);
    assert_eq!(
        RetrievalStage::decode(&trailing),
        Err(HybridRetrievalCodecRefusal::Malformed)
    );

    let oversized = vec![0; MAXIMUM_HYBRID_BATCH_BYTES as usize + 1];
    assert_eq!(
        RetrievalStage::decode(&oversized),
        Err(HybridRetrievalCodecRefusal::OutputTooLarge)
    );

    let mut zero_rank = stage;
    zero_rank.candidates[0].rank = 0;
    assert_eq!(
        zero_rank.encode(),
        Err(HybridRetrievalCodecRefusal::InvalidRank)
    );

    let mut crossed_score = stages().remove(0);
    crossed_score.candidates[0].score = Some(MechanismScore::LexicalScore(42));
    assert_eq!(
        crossed_score.encode(),
        Err(HybridRetrievalCodecRefusal::InvalidMechanismScore)
    );
}

#[test]
fn terminal_boundary_outcomes_retain_exact_policy_identity() {
    for outcome in [
        HybridRetrievalOutcome::NeedEarlierHistory,
        HybridRetrievalOutcome::BoundaryUnavailable,
    ] {
        let receipt = HybridRetrievalReceipt {
            policy_identity: "fusion/reciprocal-rank-origin@1".into(),
            outcome,
        };
        assert_eq!(
            HybridRetrievalReceipt::decode(&receipt.encode().unwrap()).unwrap(),
            receipt
        );
    }
}
