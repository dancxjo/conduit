extern crate std;
use super::*;
use crate::{
    Chunk, ChunkIdentity, ClockBasis, ExtractionLineage, HybridCandidate, RagIdentity, RerankScore,
    RetrievalMechanism, RetrieverIdentity, SourceRef, SourceSpan, SourceSpanUnit,
    TransformProfiles,
};
use alloc::{format, vec, vec::Vec};
use conduit_core::{
    BoundedResourceRef, KindId, ResourceClassId, ResourceExtent, ResourceLifetime,
    ResourceSemanticIdentity, ResourceVersionIdentity,
};
fn candidate(rank: u16, seed: u8, text: &str) -> ContextCandidate {
    let lineage = ExtractionLineage {
        source: SourceRef {
            resource: BoundedResourceRef {
                identity: ResourceSemanticIdentity::from_digest([1; 32]),
                content_profile: KindId::from("text/utf8"),
                access_class: ResourceClassId::from("read/fixture"),
                extent: ResourceExtent {
                    bytes: 4096,
                    items: None,
                },
                lifetime: ResourceLifetime {
                    version: ResourceVersionIdentity::from_digest([seed; 32]),
                    expires_at: None,
                },
            },
        },
        span: SourceSpan::new(SourceSpanUnit::Bytes, 0, text.len() as u64).unwrap(),
        extraction_profile: RagIdentity::new("extract/text@1".into()).unwrap(),
        transform_profiles: TransformProfiles::new(BoundedSequence::new()).unwrap(),
        parent_chunk: None,
    };
    let chunk = Chunk::new(
        lineage,
        ExtractedSourceValue::Text(text.as_bytes().to_vec()),
    )
    .unwrap();
    ContextCandidate {
        reranked: RerankedCandidate {
            candidate: HybridCandidate {
                chunk,
                rank,
                fusion_score_micros: 1,
                contributions: vec![RetrievalContribution {
                    retriever: RetrieverIdentity::new(
                        "metadata/exact@1".into(),
                        RetrievalMechanism::Metadata,
                    )
                    .unwrap(),
                    stage_rank: rank,
                    score: None,
                    temporal_evidence_identity: None,
                }],
            },
            original_rank: rank,
            reranked_rank: rank,
            score: RerankScore::hybrid_fusion(1).unwrap(),
        },
        reranking_policy_identity: "rerank/preserve@1".into(),
        reranking_proof_class: RerankingProofClass::DeterministicConformance,
        temporal: None,
        redundancy_group: None,
        token_count: 2,
    }
}
fn temporal(value: &mut ContextCandidate, ticks: u64, clock: ClockBasis) {
    let id = format!("time/{ticks}");
    value.reranked.candidate.contributions[0].temporal_evidence_identity = Some(id.clone());
    let provenance = TemporalProvenance {
        event_at: Some(ticks),
        observed_at: None,
        recorded_at: None,
        ingested_at: None,
        valid_from: None,
        valid_until: None,
        retrieved_at: 100,
        reference_at: 100,
        clock_basis: clock,
        uncertainty_millis: None,
    };
    value.temporal = Some(ContextTemporalEvidence {
        evidence_identity: id,
        source: TemporalSource::Event,
        boundary: None,
        context: TemporalContext {
            source: TemporalSource::Event,
            relation: provenance.relation(TemporalSource::Event).unwrap(),
            validity: TemporalValidity::Historical,
            relation_to_query_window: None,
        },
        provenance,
    });
}
fn policy(
    order: ContextOrderingPolicy,
    redundancy: ContextRedundancyPolicy,
    items: u16,
    bytes: u32,
    tokens: u32,
    work: u32,
) -> ContextSelectionPolicy {
    ContextSelectionPolicy::new(
        "selection/exact@1".into(),
        "tokens/explicit@1".into(),
        redundancy,
        order,
        items,
        bytes,
        tokens,
        work,
    )
    .unwrap()
}
fn run(
    policy: &ContextSelectionPolicy,
    values: &[ContextCandidate],
) -> Result<
    (
        BorrowedContextSelection,
        Vec<usize>,
        Vec<BorrowedContextOmission>,
    ),
    ContextSelectionRefusal,
> {
    let views: Vec<_> = values
        .iter()
        .map(BorrowedContextCandidate::from_owned)
        .collect();
    let mut selector = PreparedContextSelector::prepare(values.len().max(1)).unwrap();
    let mut selected = vec![usize::MAX; values.len()];
    let mut omitted = vec![None; values.len()];
    let result = crate::temporal_evidence_selection::assert_without_allocations(|| {
        selector.select(policy, &views, &mut selected, &mut omitted)
    })?;
    selected.truncate(result.selected_len);
    let omitted = omitted.into_iter().flatten().collect();
    Ok((result, selected, omitted))
}
#[test]
fn reranked_and_chronological_order_share_owned_results() {
    let mut values = vec![
        candidate(2, 1, "first"),
        candidate(1, 2, "second"),
        candidate(3, 3, "third"),
    ];
    for (value, time) in values.iter_mut().zip([30, 10, 20]) {
        temporal(value, time, ClockBasis::UnixEpochMilliseconds);
    }
    for (order, expected) in [
        (ContextOrderingPolicy::Reranked, vec![1, 0, 2]),
        (
            ContextOrderingPolicy::ChronologicalOldestFirst,
            vec![1, 2, 0],
        ),
    ] {
        let p = policy(order, ContextRedundancyPolicy::KeepAll, 3, 100, 100, 100);
        let (result, indices, omitted) = run(&p, &values).unwrap();
        assert_eq!(indices, expected);
        assert!(omitted.is_empty());
        assert_eq!(
            result.used,
            SelectedContextCost {
                bytes: 16,
                tokens: 6,
                work_units: 3
            }
        );
        let owned = p.select(&values).unwrap();
        assert_eq!(owned.used, result.used);
        assert_eq!(
            owned
                .items
                .iter()
                .map(|v| v.reranked.candidate.chunk.identity)
                .collect::<Vec<_>>(),
            indices
                .iter()
                .map(|&i| values[i].reranked.candidate.chunk.identity)
                .collect::<Vec<_>>()
        );
    }
}
#[test]
fn exact_budget_precedence_and_group_consumption_are_preserved() {
    let mut values = vec![
        candidate(1, 1, "oversized"),
        candidate(2, 2, "ok"),
        candidate(3, 3, "x"),
    ];
    values[0].redundancy_group = Some("group/a".into());
    values[1].redundancy_group = Some("group/a".into());
    values[2].redundancy_group = Some("group/b".into());
    let p = policy(
        ContextOrderingPolicy::Reranked,
        ContextRedundancyPolicy::OnePerReviewedGroup,
        3,
        3,
        100,
        100,
    );
    let (result, selected, omitted) = run(&p, &values).unwrap();
    assert_eq!(selected, vec![2]);
    assert_eq!(
        omitted.iter().map(|v| v.reason).collect::<Vec<_>>(),
        vec![
            ContextOmissionReason::ByteBudget,
            ContextOmissionReason::ReviewedRedundancy
        ]
    );
    assert_eq!(result.used.work_units, 1);
    for (items, bytes, tokens, work, reason) in [
        (1, 100, 100, 100, ContextOmissionReason::ItemBudget),
        (3, 9, 100, 100, ContextOmissionReason::ByteBudget),
        (3, 100, 2, 100, ContextOmissionReason::TokenBudget),
        (3, 100, 100, 1, ContextOmissionReason::WorkBudget),
    ] {
        let p = policy(
            ContextOrderingPolicy::Reranked,
            ContextRedundancyPolicy::KeepAll,
            items,
            bytes,
            tokens,
            work,
        );
        let (_, _, omitted) = run(&p, &values).unwrap();
        assert_eq!(omitted[0].reason, reason);
    }
}
#[test]
fn owned_validation_refusals_match_borrowed_and_clear_slots() {
    let p = policy(
        ContextOrderingPolicy::Reranked,
        ContextRedundancyPolicy::KeepAll,
        2,
        100,
        100,
        100,
    );
    let original = vec![candidate(1, 1, "a"), candidate(2, 2, "b")];
    for change in 0..6 {
        let mut values = original.clone();
        let expected = match change {
            0 => {
                values[1].reranked.reranked_rank = 1;
                ContextSelectionRefusal::DuplicateCandidate
            }
            1 => {
                values[1].reranked.candidate.chunk = values[0].reranked.candidate.chunk.clone();
                ContextSelectionRefusal::DuplicateCandidate
            }
            2 => {
                values[0].token_count = 0;
                ContextSelectionRefusal::EmptyTokenCost
            }
            3 => {
                values[0].reranked.original_rank = 0;
                ContextSelectionRefusal::InvalidCandidate
            }
            4 => {
                values[0].reranked.candidate.contributions.clear();
                ContextSelectionRefusal::InvalidCandidate
            }
            _ => {
                values[0].reranked.candidate.chunk.identity = ChunkIdentity::from_digest([99; 32]);
                ContextSelectionRefusal::InvalidCandidate
            }
        };
        assert_eq!(run(&p, &values).unwrap_err(), expected);
        assert_eq!(p.select(&values).unwrap_err(), expected);
    }
    let mut selector = PreparedContextSelector::prepare(2).unwrap();
    let views: Vec<_> = original
        .iter()
        .map(BorrowedContextCandidate::from_owned)
        .collect();
    let mut selected = [7; 2];
    let mut omissions = [Some(BorrowedContextOmission {
        candidate_index: 7,
        reason: ContextOmissionReason::ByteBudget,
    }); 2];
    assert_eq!(
        selector.select(&p, &[], &mut selected, &mut omissions),
        Err(ContextSelectionRefusal::EmptyCandidates)
    );
    assert_eq!(selected, [usize::MAX; 2]);
    assert_eq!(omissions, [None; 2]);
    assert!(selector
        .select(&p, &views, &mut selected, &mut omissions)
        .is_ok());
}
#[test]
fn temporal_reference_contribution_and_clock_cases_keep_owner_semantics() {
    let mut values = vec![candidate(1, 1, "a"), candidate(2, 2, "b")];
    temporal(&mut values[0], 10, ClockBasis::UnixEpochMilliseconds);
    temporal(
        &mut values[1],
        20,
        ClockBasis::monotonic_milliseconds(
            crate::ClockIdentity::new("other/clock".into()).unwrap(),
        )
        .unwrap(),
    );
    let p = policy(
        ContextOrderingPolicy::ChronologicalOldestFirst,
        ContextRedundancyPolicy::KeepAll,
        2,
        100,
        100,
        100,
    );
    assert_eq!(run(&p, &values).unwrap().1, vec![0, 1]); // Owner orders explicit ticks; it does not invent cross-clock conversion.
    for change in 0..5 {
        let mut values = values.clone();
        let expected = match change {
            0 => {
                values[0].temporal = None;
                ContextSelectionRefusal::MissingTemporalEvidence
            }
            1 => {
                values[0].temporal.as_mut().unwrap().provenance.retrieved_at = 101;
                ContextSelectionRefusal::InvalidTemporalEvidence
            }
            2 => {
                values[0].temporal.as_mut().unwrap().context.relation = TemporalRelation::Present;
                ContextSelectionRefusal::TemporalEvidenceNotContributed
            }
            3 => {
                values[0].reranked.candidate.contributions[0].temporal_evidence_identity = None;
                ContextSelectionRefusal::TemporalEvidenceNotContributed
            }
            _ => {
                values[0].temporal.as_mut().unwrap().provenance.valid_from = Some(30);
                values[0].temporal.as_mut().unwrap().provenance.valid_until = Some(20);
                ContextSelectionRefusal::InvalidTemporalEvidence
            }
        };
        assert_eq!(run(&p, &values).unwrap_err(), expected);
        assert_eq!(p.select(&values).unwrap_err(), expected);
    }
}
#[test]
fn canonical_lineage_primitives_are_validated_without_allocation() {
    let values = [candidate(1, 1, "hello")];
    let original = &values[0].reranked.candidate.chunk;
    let encoded = original.lineage.source.resource.encode().unwrap();
    let lineage = BorrowedContextLineage {
        source: BorrowedContextSource::Encoded(&encoded),
        span_unit: SourceSpanUnit::Bytes,
        span_start: 0,
        span_end: 5,
        extraction_profile: original.lineage.extraction_profile.get(),
        transform_profiles: &[],
        parent_chunk: None,
    };
    let mut view = BorrowedContextCandidate::from_owned(&values[0]);
    view.chunk = BorrowedContextChunk::Explicit {
        identity: original.identity,
        lineage,
        value: BorrowedContextValue::Text(b"hello"),
    };
    let p = policy(
        ContextOrderingPolicy::Reranked,
        ContextRedundancyPolicy::KeepAll,
        1,
        100,
        100,
        100,
    );
    let mut selector = PreparedContextSelector::prepare(1).unwrap();
    let mut selected = [usize::MAX];
    let mut omitted = [None];
    let result = crate::temporal_evidence_selection::assert_without_allocations(|| {
        selector.select(&p, &[view], &mut selected, &mut omitted)
    });
    assert_eq!(result.unwrap().used.bytes, 5);
    assert_eq!(selected, [0]);
    view.chunk = BorrowedContextChunk::Explicit {
        identity: ChunkIdentity::from_digest([99; 32]),
        lineage,
        value: BorrowedContextValue::Text(b"hello"),
    };
    assert_eq!(
        selector.select(&p, &[view], &mut selected, &mut omitted),
        Err(ContextSelectionRefusal::InvalidCandidate)
    );
}

#[test]
fn primitive_span_profile_group_and_buffer_bounds_fail_closed() {
    let values = [candidate(1, 1, "hello")];
    let original = &values[0].reranked.candidate.chunk;
    let resource = &original.lineage.source.resource;
    let lineage = BorrowedContextLineage {
        source: BorrowedContextSource::Owned(resource),
        span_unit: SourceSpanUnit::Bytes,
        span_start: 0,
        span_end: 0,
        extraction_profile: "extract/text@1",
        transform_profiles: &[],
        parent_chunk: None,
    };
    let mut view = BorrowedContextCandidate::from_owned(&values[0]);
    view.chunk = BorrowedContextChunk::Explicit {
        identity: original.identity,
        lineage,
        value: BorrowedContextValue::Text(b"hello"),
    };
    let p = policy(
        ContextOrderingPolicy::Reranked,
        ContextRedundancyPolicy::KeepAll,
        1,
        100,
        100,
        100,
    );
    let mut selector = PreparedContextSelector::prepare(1).unwrap();
    let mut selected = [0];
    let mut omitted = [None];
    assert_eq!(
        selector.select(&p, &[view], &mut selected, &mut omitted),
        Err(ContextSelectionRefusal::InvalidCandidate)
    );
    view = BorrowedContextCandidate::from_owned(&values[0]);
    let group = policy(
        ContextOrderingPolicy::Reranked,
        ContextRedundancyPolicy::OnePerReviewedGroup,
        1,
        100,
        100,
        100,
    );
    assert_eq!(
        selector.select(&group, &[view], &mut selected, &mut omitted),
        Err(ContextSelectionRefusal::MissingRedundancyGroup)
    );
    assert_eq!(
        selector.select(&p, &[view], &mut [], &mut omitted),
        Err(ContextSelectionRefusal::InvalidBound)
    );
    let transforms = ["same", "same"];
    view.chunk = BorrowedContextChunk::Explicit {
        identity: original.identity,
        lineage: BorrowedContextLineage {
            span_end: 5,
            transform_profiles: &transforms,
            ..lineage
        },
        value: BorrowedContextValue::Text(b"hello"),
    };
    assert_eq!(
        selector.select(&p, &[view], &mut selected, &mut omitted),
        Err(ContextSelectionRefusal::InvalidCandidate)
    );
}

#[test]
fn owned_admission_refuses_empty_and_oversized_before_allocating_views() {
    let p = policy(
        ContextOrderingPolicy::Reranked,
        ContextRedundancyPolicy::KeepAll,
        1,
        100,
        100,
        100,
    );
    let values = vec![candidate(1, 1, "a"); MAXIMUM_HYBRID_OUTPUT_CANDIDATES as usize + 1];
    let empty = crate::temporal_evidence_selection::assert_without_allocations(|| p.select(&[]));
    assert_eq!(empty.unwrap_err(), ContextSelectionRefusal::EmptyCandidates);
    let oversized =
        crate::temporal_evidence_selection::assert_without_allocations(|| p.select(&values));
    assert_eq!(
        oversized.unwrap_err(),
        ContextSelectionRefusal::CandidateLimitExceeded
    );
}
