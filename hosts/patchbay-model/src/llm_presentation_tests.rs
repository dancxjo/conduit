use super::*;
use conduit_ai::{
    llm_contract, ConfidencePermille, LlmDeterminismProfile, LlmWorkBounds, LocalModelCachePolicy,
    LocalModelIdentity, LocalModelKindProfile, LocalModelLifecycleState, LocalModelLimits,
    ModelEffectProposal, ModelResultDisposition, ModelWorkAccounting, ProposalRefusal,
    LLM_INTERPRET_KIND,
};
use conduit_body::Body;
use conduit_core::{
    ActivePlayId, ArtifactId, BootId, CapabilityId, CapabilityLimits, CheckedFormId,
    ExecutionProfileId, ExpandedFormId, GearId, HostId, ImplementationId, KindContractRevision,
    KindId, OfferGeneration, PlacementId, PlanId, SignId, SourceDocumentId,
};
use conduit_presentation::PresentationPropertyValue;

fn basis() -> PresentationBasis {
    let source = SourceDocumentId::from("source/llm");
    let checked = CheckedFormId::from("checked/llm");
    let body = Body::born(
        source.clone(),
        checked.clone(),
        1,
        SignId::from("sign/born"),
    )
    .unwrap();
    let (body, wake) = body.wake(2, SignId::from("sign/woke")).unwrap();
    PresentationBasis {
        seed_id: Some(body.seed_id.clone()),
        body_id: Some(body.body_id),
        wake_id: Some(wake.wake_id),
        source_document_id: Some(source),
        checked_form_id: Some(checked),
        expanded_form_id: Some(ExpandedFormId::from("expanded/llm")),
        plan_id: Some(PlanId::from("plan/llm")),
        active_play_id: Some(ActivePlayId::from("play/llm")),
        sign_ids: vec![SignId::from("sign/system-effect")],
    }
}

fn offer() -> LocalModelOffer {
    LocalModelOffer {
        identity: LocalModelIdentity {
            runtime_name: "ollama".into(),
            runtime_version: "0.23.0".into(),
            runtime_build_identity: "ollama/0.23.0/linux-amd64".into(),
            model_name: "gpt-oss:20b".into(),
            model_content_identity: "gpt-oss/20b/q4".into(),
            architecture: "gpt-oss".into(),
            parameter_profile: "20B".into(),
            quantization: "Q4_K_M".into(),
        },
        limits: LocalModelLimits {
            work: LlmWorkBounds {
                maximum_input_bytes: 4096,
                maximum_context_items: 8,
                maximum_output_bytes: 1024,
                maximum_work_units: 8192,
                maximum_history_items: 4,
            },
            model_bytes: 8_000_000_000,
            admitted_memory_mib: 16_384,
            maximum_in_flight: 1,
            maximum_queue_items: 2,
            maximum_queue_bytes: 8192,
            cancellation_supported: true,
            cache_policy: LocalModelCachePolicy::OneLoadedModelUntilShutdown,
        },
        supported_profiles: vec![LocalModelKindProfile::InterpretSignEvidence],
        initialized: true,
        lifecycle: LocalModelLifecycleState::Ready,
        determinism: LlmDeterminismProfile::ProviderNondeterministic,
    }
}

fn placement(contract: &LlmSemanticContract) -> PlannedGear {
    PlannedGear {
        placement_id: PlacementId::from("placement/interpreter"),
        gear_id: GearId::from("observer/interpreter"),
        kind_id: contract.kind_id.clone(),
        kind_contract_revision: contract.kind_contract_revision.clone(),
        execution_profile_id: ExecutionProfileId::from("conduit.llm/local-model-hosted@1"),
        configuration: vec![],
        host_id: HostId::from("host/forebrain"),
        boot_id: BootId::from("boot/forebrain/7"),
        offer_generation: OfferGeneration(11),
        capability_id: CapabilityId::from("local-model/llm/interpret"),
        implementation_id: ImplementationId::from("std/local-open-weight-model@1"),
        artifact_id: ArtifactId::from("ollama/gpt-oss/20b/q4"),
        realization_characteristics: vec![],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 2,
            max_queue_bytes: 8192,
        },
        inputs: contract.inputs.clone(),
        outputs: contract.outputs.clone(),
        host_operations: vec![],
        resources: vec![],
        authority: vec![],
        pool_references: vec![],
    }
}

fn result(contract: &LlmSemanticContract) -> ModelDerivedResult {
    ModelDerivedResult {
        provenance: ModelResultProvenance::ModelDerived,
        payload_kind: contract.result_payload_kind.as_str().into(),
        payload: b"bird observed".to_vec(),
        implementation_identity: "ollama/gpt-oss:20b/q4".into(),
        request_identity: "request/observe/3".into(),
        run_identity: "run/observe/3".into(),
        confidence: Some(ConfidencePermille(900)),
        disposition: ModelResultDisposition::Produced,
        determinism: LlmDeterminismProfile::ProviderNondeterministic,
        accounting: ModelWorkAccounting {
            input_bytes: 512,
            context_items: 2,
            output_bytes: 13,
            work_units: 900,
            history_items: 0,
        },
    }
}

fn proposal(proposal_id: &str, operation_kind: &str) -> ModelEffectProposal {
    ModelEffectProposal {
        proposal_id: proposal_id.into(),
        plan_id: PlanId::from("plan/llm"),
        operation_kind: KindId::from(operation_kind),
        canonical_arguments: br#"{"enabled":true}"#.to_vec(),
        rationale: "Await an ordinary authority decision".into(),
        evidence: vec![SignId::from("sign/observed-bird")],
    }
}

#[test]
fn shared_presentation_keeps_semantics_realization_provenance_and_effect_stages_distinct() {
    let contract = llm_contract(LLM_INTERPRET_KIND).unwrap();
    let placement = placement(&contract);
    let offer = offer();
    let result = result(&contract);
    let candidate = CandidateFormInspection {
        candidate_identity: "candidate-form/birds".into(),
        provenance: CandidateFormProvenance {
            implementation_identity: "ollama/gpt-oss:20b/q4".into(),
            request_identity: "request/compose/birds".into(),
            run_identity: "run/compose/birds".into(),
            catalog_basis_identity: "catalog/vision/1".into(),
        },
        lifecycle: CandidateLifecycle::AwaitingExplicitValidationPlanAndPlay,
        source_document_identity: "source/candidate-birds".into(),
    };
    let proposals = vec![
        proposal("proposal/request-light", "effect/light@1"),
        proposal("proposal/request-door", "effect/door@1"),
    ];
    let decisions = vec![
        ProposalDecision {
            decision_id: "decision/request-light/1".into(),
            proposal_id: "proposal/request-light".into(),
            authority_id: Some("authority/light".into()),
            outcome: ProposalDecisionOutcome::Authorized {
                request_id: "request/light/1".into(),
            },
        },
        ProposalDecision {
            decision_id: "decision/request-door/2".into(),
            proposal_id: "proposal/request-door".into(),
            authority_id: None,
            outcome: ProposalDecisionOutcome::Refused(ProposalRefusal::MissingAuthority),
        },
    ];
    let effects = vec![EffectReceipt {
        effect_id: "effect/light/1".into(),
        request_id: "request/light/1".into(),
        resulting_signs: vec![SignId::from("sign/system-effect")],
    }];
    let presentation = project_llm_patchbay(
        9,
        basis(),
        &LlmPatchbayTruth {
            gear_identity: "gear/interpreter".into(),
            contract: &contract,
            placement: Some(&placement),
            model_offer: Some(&offer),
            activity: LlmGearActivity::Completed,
            result: Some(&result),
            candidate_form: Some(&candidate),
            proposals: &proposals,
            decisions: &decisions,
            effects: &effects,
        },
    )
    .unwrap();

    presentation.validate().unwrap();
    let port_labels = presentation
        .subjects
        .iter()
        .filter(|subject| subject.role == PresentationRole::Port)
        .map(|subject| subject.label.as_str())
        .collect::<Vec<_>>();
    assert_eq!(port_labels, ["request", "result"]);
    assert!(!port_labels.contains(&"prompt"));
    assert!(!port_labels.contains(&"completion"));
    assert!(has_text_property(
        &presentation,
        "gear/interpreter",
        "model-name",
        "gpt-oss:20b"
    ));
    assert!(has_count_property(
        &presentation,
        "gear/interpreter",
        "maximum-output-bytes",
        1024
    ));
    assert!(has_any_text_property(
        &presentation,
        "evidence-class",
        "MODEL-DERIVED INFO"
    ));
    assert!(has_any_text_property(
        &presentation,
        "evidence-class",
        "SYSTEM SIGN EVIDENCE"
    ));
    assert!(has_any_text_property(
        &presentation,
        "stage",
        "AWAITING AUTHORITY"
    ));
    assert!(has_any_text_property(
        &presentation,
        "authority-state",
        "ADMITTED"
    ));
    assert!(has_any_text_property(
        &presentation,
        "authority-state",
        "REFUSED"
    ));
    assert!(has_text_property(
        &presentation,
        "candidate-form/birds",
        "auto-run",
        "false"
    ));
    assert_eq!(
        presentation.actions.len(),
        0,
        "projection cannot invent execution actions"
    );
}

#[test]
fn stale_contract_invalid_offer_and_unbounded_stage_history_refuse() {
    let contract = llm_contract(LLM_INTERPRET_KIND).unwrap();
    let mut stale_placement = placement(&contract);
    stale_placement.kind_contract_revision = KindContractRevision::from("stale");
    let truth = LlmPatchbayTruth {
        gear_identity: "gear/interpreter".into(),
        contract: &contract,
        placement: Some(&stale_placement),
        model_offer: None,
        activity: LlmGearActivity::Waiting,
        result: None,
        candidate_form: None,
        proposals: &[],
        decisions: &[],
        effects: &[],
    };
    assert_eq!(
        project_llm_patchbay(1, basis(), &truth),
        Err(LlmPresentationError::ContractMismatch)
    );

    let placement = placement(&contract);
    let mut invalid_offer = offer();
    invalid_offer.initialized = false;
    let truth = LlmPatchbayTruth {
        placement: Some(&placement),
        model_offer: Some(&invalid_offer),
        ..truth
    };
    assert_eq!(
        project_llm_patchbay(1, basis(), &truth),
        Err(LlmPresentationError::InvalidModelOffer)
    );

    let decisions = (0..=MAXIMUM_LLM_PRESENTATION_STAGES)
        .map(|index| ProposalDecision {
            decision_id: format!("decision/{index}"),
            proposal_id: format!("proposal/{index}"),
            authority_id: None,
            outcome: ProposalDecisionOutcome::Refused(ProposalRefusal::MissingAuthority),
        })
        .collect::<Vec<_>>();
    let truth = LlmPatchbayTruth {
        model_offer: None,
        decisions: &decisions,
        ..truth
    };
    assert_eq!(
        project_llm_patchbay(1, basis(), &truth),
        Err(LlmPresentationError::TooManyStages)
    );
}

fn has_text_property(presentation: &Presentation, subject: &str, name: &str, value: &str) -> bool {
    presentation.properties.iter().any(|property| {
        property.subject == subject
            && property.name == name
            && property.value == PresentationPropertyValue::Text(value.into())
    })
}

fn has_any_text_property(presentation: &Presentation, name: &str, value: &str) -> bool {
    presentation.properties.iter().any(|property| {
        property.name == name && property.value == PresentationPropertyValue::Text(value.into())
    })
}

fn has_count_property(presentation: &Presentation, subject: &str, name: &str, value: u64) -> bool {
    presentation.properties.iter().any(|property| {
        property.subject == subject
            && property.name == name
            && property.value == PresentationPropertyValue::Count(value)
    })
}
