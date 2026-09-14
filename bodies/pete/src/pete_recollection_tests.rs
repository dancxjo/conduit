use super::*;
use crate::{ExperienceCandidate, ExperienceKind, ExperienceProvenance, Sensitivity};
use conduit_ai::{
    AnswerSpan, Chunk, Citation, ContextOrderingPolicy, ContextRedundancyPolicy,
    ContextSelectionDisposition, ContextTemporalEvidence, ExtractedSourceValue, ExtractionLineage,
    GroundedAnswerDisposition, LlmDeterminismProfile, MechanismScore, ModelResultDisposition,
    ModelResultProvenance, ModelWorkAccounting, ProposedClaimSupport, RerankScore,
    RerankedCandidate, RerankingProofClass, RetrievalContribution, RetrievalIntent,
    RetrievalMechanism, RetrievalMode, RetrieverIdentity, SelectedContextCost, SelectedContextItem,
    SelectedContextRationale, SourceRef, SourceSpan, SourceSpanUnit, StructuredContext,
    TemporalContext, TemporalProvenance, TemporalSource, TemporalValidity,
};
use conduit_core::{
    BoundedResourceRef, KindId, ResourceClassId, ResourceExtent, ResourceLifetime,
    ResourceSemanticIdentity, ResourceVersionIdentity, TemporalRelation,
};

fn retained_memory() -> BoundedAutobiography {
    let mut memory = BoundedAutobiography::new(8).unwrap();
    for (identity, source, event, kind, content) in [
        (
            "experience/battery/1",
            "sign/create/battery/1",
            100,
            ExperienceKind::ObservedSign,
            "Battery was 41 percent",
        ),
        (
            "experience/human/1",
            "sign/human/utterance/1",
            110,
            ExperienceKind::HumanStatement,
            "What happened?",
        ),
        (
            "experience/model/1",
            "model/run/response/1",
            120,
            ExperienceKind::ModelDerived,
            "I will check.",
        ),
        (
            "experience/action/1",
            "request/drive/1",
            130,
            ExperienceKind::ActionRequest,
            "Drive forward",
        ),
        (
            "experience/effect/1",
            "sign/drive/refused/1",
            140,
            ExperienceKind::EffectResult,
            "Drive refused by safety",
        ),
    ] {
        memory
            .retain(
                ExperienceCandidate {
                    identity: identity.into(),
                    kind,
                    content: content.as_bytes().to_vec(),
                    provenance: vec![ExperienceProvenance {
                        source_identity: source.into(),
                        event_at_millis: event,
                        recorded_at_millis: event + 1,
                        body_identity: "body/pete".into(),
                        host_identity: Some("host/brainstem".into()),
                        boot_identity: Some("boot/1".into()),
                        plan_identity: Some("plan/1".into()),
                        play_identity: Some("play/1".into()),
                    }],
                    sensitivity: Sensitivity::LocalPrivate,
                    supersedes: None,
                    explicit_remember: true,
                },
                true,
            )
            .unwrap();
    }
    memory
}

fn source(version: u8) -> SourceRef {
    SourceRef {
        resource: BoundedResourceRef {
            identity: ResourceSemanticIdentity::from_digest([7; 32]),
            content_profile: KindId::from("pete/retained-experience@1"),
            access_class: ResourceClassId::from("resource/read-authorized@1"),
            extent: ResourceExtent {
                bytes: 1_024,
                items: Some(1),
            },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest([version; 32]),
                expires_at: None,
            },
        },
    }
}

fn request() -> GroundedAnswerRequest {
    let text = b"Battery was 41 percent".to_vec();
    let chunk = Chunk::new(
        ExtractionLineage {
            source: source(1),
            span: SourceSpan {
                unit: SourceSpanUnit::Bytes,
                start: 0,
                end: text.len() as u64,
            },
            extraction_profile: "extract/retained-experience@1".into(),
            transform_profiles: vec![],
            parent_chunk: None,
        },
        ExtractedSourceValue::Text(text.clone()),
    )
    .unwrap();
    let item = SelectedContextItem {
        reranked: RerankedCandidate {
            candidate: conduit_ai::HybridCandidate {
                chunk,
                rank: 1,
                fusion_score_micros: 100_000,
                contributions: vec![RetrievalContribution {
                    retriever: RetrieverIdentity {
                        identity: "retriever/temporal-exact@1".into(),
                        mechanism: RetrievalMechanism::Temporal,
                    },
                    stage_rank: 1,
                    score: Some(MechanismScore::TemporalBoundary),
                    temporal_evidence_identity: Some("experience/battery/1".into()),
                }],
            },
            original_rank: 1,
            reranked_rank: 1,
            score: RerankScore::HybridFusion(100_000),
        },
        reranking_policy_identity: "rerank/exact@1".into(),
        reranking_proof_class: RerankingProofClass::DeterministicConformance,
        temporal: Some(ContextTemporalEvidence {
            evidence_identity: "experience/battery/1".into(),
            provenance: TemporalProvenance {
                event_at: Some(100),
                valid_from: Some(100),
                valid_until: None,
                observed_at: Some(100),
                recorded_at: Some(101),
                ingested_at: Some(101),
                retrieved_at: 200,
                reference_at: 200,
                clock_basis: conduit_ai::ClockBasis::UnixEpochMilliseconds,
                uncertainty_millis: None,
            },
            source: TemporalSource::Event,
            boundary: None,
            context: TemporalContext {
                source: TemporalSource::Event,
                relation: TemporalRelation::Past {
                    minimum_ticks: 100,
                    maximum_ticks: 100,
                },
                validity: TemporalValidity::Historical,
                relation_to_query_window: None,
            },
        }),
        redundancy_group: None,
        rationale: SelectedContextRationale::TemporalChronology,
        budget: SelectedContextCost {
            bytes: text.len() as u32,
            tokens: 5,
            work_units: 1,
        },
    };
    GroundedAnswerRequest {
        identity: "request/recollection/1".into(),
        retrieval_intent: RetrievalIntent {
            identity: "intent/recent-battery".into(),
            modes: vec![RetrievalMode::Temporal(
                conduit_ai::TemporalRetrievalIntent::LatestEvidence,
            )],
            maximum_candidates: 4,
        },
        context: StructuredContext {
            policy_identity: "context/pete-memory@1".into(),
            token_accounting_profile: "tokens/exact-fixture@1".into(),
            redundancy: ContextRedundancyPolicy::KeepAll,
            ordering: ContextOrderingPolicy::ChronologicalOldestFirst,
            items: vec![item],
            disposition: ContextSelectionDisposition::Complete,
            used: SelectedContextCost {
                bytes: text.len() as u32,
                tokens: 5,
                work_units: 1,
            },
        },
    }
}

fn model_result() -> ModelDerivedResult {
    ModelDerivedResult {
        provenance: ModelResultProvenance::ModelDerived,
        payload_kind: "llm/generated-result@1".into(),
        implementation_identity: "model/fixture".into(),
        request_identity: "request/recollection/1".into(),
        run_identity: "run/recollection/1".into(),
        confidence: None,
        disposition: ModelResultDisposition::Produced,
        determinism: LlmDeterminismProfile::ProviderNondeterministic,
        accounting: ModelWorkAccounting {
            input_bytes: 64,
            context_items: 1,
            output_bytes: 27,
            work_units: 4,
            history_items: 0,
        },
        payload: b"My battery was low earlier.".to_vec(),
    }
}

fn policy() -> GroundedAnswerPolicy {
    GroundedAnswerPolicy {
        identity: "grounding/pete-memory@1".into(),
        answer_kind: "value/text-utf8@1".into(),
        maximum_output_bytes: 128,
        maximum_claims: 2,
        maximum_citations: 2,
        maximum_work_units: 16,
    }
}

fn candidates() -> Vec<PeteRetrievalCandidate> {
    [
        "experience/battery/1",
        "experience/human/1",
        "experience/model/1",
        "experience/action/1",
        "experience/effect/1",
    ]
    .into_iter()
    .map(|identity| PeteRetrievalCandidate {
        experience_identity: identity.into(),
        bases: vec![PeteRetrievalBasis::Lexical, PeteRetrievalBasis::Temporal],
    })
    .collect()
}

fn claim(request: &GroundedAnswerRequest) -> ProposedGroundedClaim {
    let chunk = &request.context.items[0].reranked.candidate.chunk;
    ProposedGroundedClaim {
        answer_span: AnswerSpan { start: 0, end: 27 },
        support: ProposedClaimSupport::Supported {
            citations: vec![Citation {
                source: chunk.lineage.source.clone(),
                span: chunk.lineage.span,
                chunk_identity: chunk.identity,
            }],
        },
    }
}

#[test]
fn selected_historical_experience_yields_one_grounded_model_recollection() {
    let memory = retained_memory();
    assert_eq!(memory.records(true).unwrap().len(), 5);
    assert_eq!(
        memory.select_temporal(
            200,
            &conduit_ai::TemporalRetrievalIntent::EvidenceWithin {
                start: 100,
                end: 140,
            },
            true,
        ),
        Ok(conduit_ai::TemporalEvidenceSelection::Selected {
            identities: vec![
                "experience/battery/1".into(),
                "experience/human/1".into(),
                "experience/model/1".into(),
                "experience/action/1".into(),
                "experience/effect/1".into(),
            ]
        })
    );
    let request = request();
    let candidates = candidates();
    let result = assemble_pete_recollection(PeteRecollectionInputs {
        memory: &memory,
        retrieval_candidates: &candidates,
        selected_experience_identities: &["experience/battery/1".into()],
        read_authorized: true,
        policy: &policy(),
        request: &request,
        assessment: &GroundingInputAssessment::Sufficient,
        model_result: &model_result(),
        proposed_claims: &[claim(&request)],
    })
    .unwrap();
    assert_eq!(
        result.grounded_answer.disposition,
        GroundedAnswerDisposition::Supported
    );
    assert_eq!(
        result.grounded_answer.provenance,
        ModelResultProvenance::ModelDerived
    );
    assert!(result.historical_not_current);
    assert_eq!(result.retrieval_candidates, candidates);
    assert_eq!(
        result.selected_experiences[0].experience_kind,
        ExperienceKind::ObservedSign
    );
    assert_eq!(
        result.selected_experiences[0].original_source_identities,
        ["sign/create/battery/1"]
    );
    assert_eq!(
        result.grounded_answer.model_run_identity,
        "run/recollection/1"
    );
    let lineage = result.evidence_lineage().unwrap();
    let patchbay = patchbay_model::PatchbayEvidenceLineage::project(&lineage).unwrap();
    assert_eq!(
        patchbay
            .row("model-inference/run/recollection/1")
            .unwrap()
            .causal_inputs,
        ["selected-experience/experience/battery/1"]
    );
    assert_eq!(
        patchbay
            .row("retrieval-candidate/experience/battery/1")
            .unwrap()
            .causal_inputs,
        ["original-fact/sign/create/battery/1"]
    );
}

#[test]
fn hallucinated_citation_and_unselected_context_both_refuse() {
    let memory = retained_memory();
    let request = request();
    let candidates = candidates();
    let mut false_claim = claim(&request);
    let ProposedClaimSupport::Supported { citations } = &mut false_claim.support else {
        unreachable!()
    };
    citations[0].source = source(9);
    assert!(matches!(
        assemble_pete_recollection(PeteRecollectionInputs {
            memory: &memory,
            retrieval_candidates: &candidates,
            selected_experience_identities: &["experience/battery/1".into()],
            read_authorized: true,
            policy: &policy(),
            request: &request,
            assessment: &GroundingInputAssessment::Sufficient,
            model_result: &model_result(),
            proposed_claims: &[false_claim],
        }),
        Err(PeteRecollectionRefusal::Grounding(
            GroundedAnswerRefusal::CitationNotInContext
        ))
    ));

    assert_eq!(
        assemble_pete_recollection(PeteRecollectionInputs {
            memory: &memory,
            retrieval_candidates: &candidates,
            selected_experience_identities: &["experience/not-selected".into()],
            read_authorized: true,
            policy: &policy(),
            request: &request,
            assessment: &GroundingInputAssessment::Sufficient,
            model_result: &model_result(),
            proposed_claims: &[claim(&request)],
        }),
        Err(PeteRecollectionRefusal::SelectedRecordNotCandidate)
    );
}

#[test]
fn unavailable_memory_and_candidate_without_retained_truth_refuse_distinctly() {
    let mut memory = retained_memory();
    let request = request();
    let candidates = candidates();
    memory.set_provider_available(false);
    assert_eq!(
        assemble_pete_recollection(PeteRecollectionInputs {
            memory: &memory,
            retrieval_candidates: &candidates,
            selected_experience_identities: &["experience/battery/1".into()],
            read_authorized: true,
            policy: &policy(),
            request: &request,
            assessment: &GroundingInputAssessment::Sufficient,
            model_result: &model_result(),
            proposed_claims: &[claim(&request)],
        }),
        Err(PeteRecollectionRefusal::Memory(
            MemoryRefusal::ProviderUnavailable
        ))
    );

    memory.set_provider_available(true);
    let unknown = [PeteRetrievalCandidate {
        experience_identity: "experience/not-retained".into(),
        bases: vec![PeteRetrievalBasis::Metadata],
    }];
    assert_eq!(
        assemble_pete_recollection(PeteRecollectionInputs {
            memory: &memory,
            retrieval_candidates: &unknown,
            selected_experience_identities: &["experience/not-retained".into()],
            read_authorized: true,
            policy: &policy(),
            request: &request,
            assessment: &GroundingInputAssessment::Sufficient,
            model_result: &model_result(),
            proposed_claims: &[claim(&request)],
        }),
        Err(PeteRecollectionRefusal::CandidateRecordUnavailable)
    );
}
