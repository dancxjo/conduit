use conduit_ai::{
    CandidateEvaluation, EvaluationDisposition, EvaluationMetric, HumanAssessment,
    HumanAssessmentDisposition, LearnedRealizationIdentity, ModelInvocationEvidence,
    ModelInvocationTerminal, ModelOperation, PromotionDecision, PromotionGrant, PromotionReceipt,
    PromotionTerminal, RollbackGrant, RollbackReceipt, RollbackTerminal, ShadowContract,
    ShadowResourceEnvelope, ShadowRun, ShadowTerminal,
};

fn realization(checkpoint: Option<u8>, provider: u8) -> LearnedRealizationIdentity {
    LearnedRealizationIdentity {
        artifact: [61; 32],
        checkpoint: checkpoint.map(|value| [value; 32]),
        signature: [62; 32],
        provider: [provider; 32],
    }
}

fn evidence(value: LearnedRealizationIdentity) -> ModelInvocationEvidence {
    ModelInvocationEvidence {
        artifact_identity: value.artifact,
        checkpoint_identity: value.checkpoint,
        signature_identity: value.signature,
        runtime_implementation_identity: "std/pete-interpretation@1".into(),
        runtime_build_identity: "build/pete-interpretation-1".into(),
        precision_profile: "integer/fixed-point".into(),
        operation: ModelOperation::Infer,
        input_identities: vec![[66; 32]],
        stochastic_seed: None,
        admitted_work_units: 8,
        terminal: ModelInvocationTerminal::Produced,
    }
}

#[test]
fn pete_interpretation_candidate_is_body_scoped_non_actuating_and_rollback_capable() {
    let baseline = realization(None, 63);
    let candidate = realization(Some(64), 65);
    let contract = ShadowContract {
        identity: [67; 32],
        subject_identity: [68; 32],
        baseline,
        candidate,
        shared_input_set_identity: [69; 32],
        resources: ShadowResourceEnvelope {
            maximum_runs: 1,
            maximum_input_bytes: 256,
            maximum_output_bytes: 128,
            maximum_work_units: 32,
        },
        candidate_output_route: "observation/pete/interpretation-shadow".into(),
        protected_effect_routes: vec!["effect/pete/create-wheels".into()],
    };
    contract.validate().unwrap();
    let shadow = ShadowRun {
        identity: [70; 32],
        contract_identity: contract.identity,
        shared_input_set_identity: contract.shared_input_set_identity,
        shared_input_identity: [66; 32],
        baseline_evidence: evidence(baseline),
        candidate_evidence: evidence(candidate),
        baseline_provider_identity: baseline.provider,
        candidate_provider_identity: candidate.provider,
        baseline_output_identity: [71; 32],
        candidate_output_identity: [72; 32],
        consumed_input_bytes: 64,
        consumed_output_bytes: 32,
        consumed_work_units: 16,
        terminal: ShadowTerminal::Compared,
    };
    contract.validate_run(&shadow).unwrap();

    let evaluation = CandidateEvaluation {
        identity: [73; 32],
        subject_identity: contract.subject_identity,
        suite_identity: [74; 32],
        baseline,
        candidate,
        shared_input_set_identity: contract.shared_input_set_identity,
        shadow_run_identities: vec![shadow.identity],
        metrics: vec![EvaluationMetric {
            identity: "pete/interpretation-agreement@1".into(),
            baseline_value_millionths: 750_000,
            candidate_value_millionths: 900_000,
        }],
        human_assessments: vec![HumanAssessment {
            evaluator_identity: [75; 32],
            evidence_identity: [76; 32],
            disposition: HumanAssessmentDisposition::SupportsCandidate,
        }],
        disposition: EvaluationDisposition::Sufficient,
    };
    evaluation.validate(&contract).unwrap();
    let grant = PromotionGrant {
        identity: [77; 32],
        authority_identity: [78; 32],
        subject_identity: contract.subject_identity,
        evaluation_identity: evaluation.identity,
        candidate,
        rollback_target: baseline,
        valid_from_tick: 10,
        valid_until_tick: Some(20),
        decision: PromotionDecision::Approved,
    };
    grant.admit(&evaluation, 15).unwrap();
    let promotion = PromotionReceipt {
        identity: [79; 32],
        grant_identity: grant.identity,
        subject_identity: contract.subject_identity,
        before_plan_identity: [80; 32],
        before_play_identity: Some([81; 32]),
        after_plan_identity: Some([82; 32]),
        after_play_identity: Some([83; 32]),
        selected: candidate,
        rollback_target: baseline,
        terminal: PromotionTerminal::Promoted,
    };
    promotion.validate(&grant).unwrap();
    let rollback = RollbackGrant {
        identity: [84; 32],
        authority_identity: [85; 32],
        subject_identity: contract.subject_identity,
        promotion_receipt_identity: promotion.identity,
        current: candidate,
        target: baseline,
        maximum_attempts: 1,
        valid_until_tick: Some(30),
    };
    rollback.admit(&promotion, 25).unwrap();
    RollbackReceipt {
        identity: [86; 32],
        grant_identity: rollback.identity,
        subject_identity: contract.subject_identity,
        before_plan_identity: [82; 32],
        after_plan_identity: Some([87; 32]),
        attempt: 1,
        selected: Some(baseline),
        terminal: RollbackTerminal::RolledBack,
    }
    .validate(&rollback)
    .unwrap();
}
