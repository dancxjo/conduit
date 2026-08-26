//! Deterministic documentary specimen for the shared LLM Patchbay projection.

use conduit_ai::{
    llm_contract, CandidateFormProvenance, CandidateLifecycle, ConfidencePermille, EffectReceipt,
    LlmDeterminismProfile, LlmWorkBounds, LocalModelCachePolicy, LocalModelIdentity,
    LocalModelKindProfile, LocalModelLifecycleState, LocalModelLimits, LocalModelOffer,
    ModelDerivedResult, ModelEffectProposal, ModelResultDisposition, ModelResultProvenance,
    ModelWorkAccounting, ProposalDecision, ProposalDecisionOutcome, ProposalRefusal,
    LLM_INTERPRET_KIND,
};
use conduit_core::{
    ArtifactId, BootId, CapabilityId, CapabilityLimits, ExecutionProfileId, GearId, HostId,
    ImplementationId, KindId, OfferGeneration, PlacementId, PlanId, PlannedGear, SignId,
};
use conduit_presentation::Presentation;

use crate::{project_llm_patchbay, CandidateFormInspection, LlmGearActivity, LlmPatchbayTruth};

pub fn llm_documentary_presentation_with_adapter(
    adapter: &dyn crate::PatchbayHostAdapter,
) -> Result<Presentation, String> {
    let base = crate::portable_demonstration_with_adapter(adapter)?;
    let contract =
        llm_contract(LLM_INTERPRET_KIND).ok_or("missing reviewed llm/interpret contract")?;
    let placement = documentary_placement(&contract);
    let offer = documentary_offer();
    let payload = b"bird observed".to_vec();
    let result = ModelDerivedResult {
        provenance: ModelResultProvenance::ModelDerived,
        payload_kind: contract.result_payload_kind.as_str().into(),
        payload: payload.clone(),
        implementation_identity: "ollama/gpt-oss:20b/q4".into(),
        request_identity: "request/observe/3".into(),
        run_identity: "run/observe/3".into(),
        confidence: Some(ConfidencePermille(900)),
        disposition: ModelResultDisposition::Produced,
        determinism: LlmDeterminismProfile::ProviderNondeterministic,
        accounting: ModelWorkAccounting {
            input_bytes: 512,
            context_items: 2,
            output_bytes: payload.len() as u64,
            work_units: 900,
            history_items: 0,
        },
    };
    let candidate = CandidateFormInspection {
        candidate_identity: "candidate-form/bird-dashboard".into(),
        provenance: CandidateFormProvenance {
            implementation_identity: "ollama/gpt-oss:20b/q4".into(),
            request_identity: "request/compose/birds".into(),
            run_identity: "run/compose/birds".into(),
            catalog_basis_identity: "catalog/vision/1".into(),
        },
        lifecycle: CandidateLifecycle::AwaitingExplicitValidationPlanAndPlay,
        source_document_identity: "source/candidate-bird-dashboard".into(),
    };
    let proposal_plan = base
        .basis
        .plan_id
        .clone()
        .ok_or("documentary Presentation lacks its exact Plan")?;
    let proposals = vec![
        documentary_proposal(
            "proposal/request-light",
            "effect/light@1",
            proposal_plan.clone(),
        ),
        documentary_proposal("proposal/request-door", "effect/door@1", proposal_plan),
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
    project_llm_patchbay(
        base.revision,
        base.basis,
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
    .map_err(|error| error.to_string())
}

#[cfg(test)]
pub fn llm_documentary_presentation() -> Result<Presentation, String> {
    llm_documentary_presentation_with_adapter(crate::host_adapter::test_host_adapter())
}

fn documentary_proposal(
    proposal_id: &str,
    operation_kind: &str,
    plan_id: PlanId,
) -> ModelEffectProposal {
    ModelEffectProposal {
        proposal_id: proposal_id.into(),
        plan_id,
        operation_kind: KindId::from(operation_kind),
        canonical_arguments: br#"{"enabled":true}"#.to_vec(),
        rationale: "Model-derived suggestion awaiting ordinary authority".into(),
        evidence: vec![SignId::from("sign/observed-bird")],
    }
}

fn documentary_offer() -> LocalModelOffer {
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
            compute: conduit_ai::LocalModelComputeNeed {
                minimum_lanes: 2,
                preferred_lanes: 4,
                maximum_lanes: 8,
                minimum_service_guarantee: conduit_core::ComputeServiceGuarantee::Shared,
            },
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

fn documentary_placement(contract: &conduit_ai::LlmSemanticContract) -> PlannedGear {
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
