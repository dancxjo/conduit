use conduit_ai::*;

fn realization(checkpoint: Option<u8>, provider: u8) -> LearnedRealizationIdentity {
    LearnedRealizationIdentity {
        artifact: [1; 32],
        checkpoint: checkpoint.map(|value| [value; 32]),
        signature: [2; 32],
        provider: [provider; 32],
    }
}

fn evidence(value: LearnedRealizationIdentity, input: [u8; 32]) -> ModelInvocationEvidence {
    ModelInvocationEvidence {
        artifact_identity: value.artifact,
        checkpoint_identity: value.checkpoint,
        signature_identity: value.signature,
        runtime_implementation_identity: "std/learned-interpretation@1".into(),
        runtime_build_identity: "build/interpretation-17".into(),
        precision_profile: "integer/fixed-point".into(),
        operation: ModelOperation::Infer,
        input_identities: vec![input],
        stochastic_seed: None,
        admitted_work_units: 10,
        terminal: ModelInvocationTerminal::Produced,
    }
}

fn contract() -> ShadowContract {
    ShadowContract {
        identity: [10; 32],
        subject_identity: [11; 32],
        baseline: realization(None, 3),
        candidate: realization(Some(4), 5),
        shared_input_set_identity: [12; 32],
        resources: ShadowResourceEnvelope {
            maximum_runs: 4,
            maximum_input_bytes: 1024,
            maximum_output_bytes: 1024,
            maximum_work_units: 100,
        },
        candidate_output_route: "observation/interpretation-shadow".into(),
        protected_effect_routes: vec!["effect/protected-actuator".into()],
    }
}

fn run(contract: &ShadowContract) -> ShadowRun {
    ShadowRun {
        identity: [13; 32],
        contract_identity: contract.identity,
        shared_input_set_identity: contract.shared_input_set_identity,
        shared_input_identity: [14; 32],
        baseline_evidence: evidence(contract.baseline, [14; 32]),
        candidate_evidence: evidence(contract.candidate, [14; 32]),
        baseline_provider_identity: contract.baseline.provider,
        candidate_provider_identity: contract.candidate.provider,
        baseline_output_identity: [15; 32],
        candidate_output_identity: [16; 32],
        consumed_input_bytes: 64,
        consumed_output_bytes: 32,
        consumed_work_units: 20,
        terminal: ShadowTerminal::Compared,
    }
}

fn evaluation_fixture(contract: &ShadowContract) -> CandidateEvaluation {
    CandidateEvaluation {
        identity: [17; 32],
        subject_identity: contract.subject_identity,
        suite_identity: [18; 32],
        baseline: contract.baseline,
        candidate: contract.candidate,
        shared_input_set_identity: contract.shared_input_set_identity,
        shadow_run_identities: vec![[13; 32]],
        metrics: vec![EvaluationMetric {
            identity: "interpretation/agreement@1".into(),
            baseline_value_millionths: 750_000,
            candidate_value_millionths: 900_000,
        }],
        human_assessments: vec![HumanAssessment {
            evaluator_identity: [19; 32],
            evidence_identity: [20; 32],
            disposition: HumanAssessmentDisposition::SupportsCandidate,
        }],
        disposition: EvaluationDisposition::Sufficient,
    }
}

fn grant_fixture(contract: &ShadowContract, evaluation: &CandidateEvaluation) -> PromotionGrant {
    PromotionGrant {
        identity: [21; 32],
        authority_identity: [22; 32],
        subject_identity: contract.subject_identity,
        evaluation_identity: evaluation.identity,
        candidate: contract.candidate,
        rollback_target: contract.baseline,
        valid_from_tick: 100,
        valid_until_tick: Some(200),
        decision: PromotionDecision::Approved,
    }
}

#[test]
fn harmless_candidate_shadows_promotes_by_replacement_plan_and_rolls_back() {
    let contract = contract();
    contract.validate().unwrap();
    contract.validate_run(&run(&contract)).unwrap();

    let evaluation = evaluation_fixture(&contract);
    evaluation.validate(&contract).unwrap();
    let grant = grant_fixture(&contract, &evaluation);
    grant.admit(&evaluation, 150).unwrap();

    let promotion = PromotionReceipt {
        identity: [23; 32],
        grant_identity: grant.identity,
        subject_identity: contract.subject_identity,
        before_plan_identity: [24; 32],
        before_play_identity: Some([25; 32]),
        after_plan_identity: Some([26; 32]),
        after_play_identity: Some([27; 32]),
        selected: contract.candidate,
        rollback_target: contract.baseline,
        terminal: PromotionTerminal::Promoted,
    };
    promotion.validate(&grant).unwrap();

    let rollback = RollbackGrant {
        identity: [28; 32],
        authority_identity: [29; 32],
        subject_identity: contract.subject_identity,
        promotion_receipt_identity: promotion.identity,
        current: contract.candidate,
        target: contract.baseline,
        maximum_attempts: 1,
        valid_until_tick: Some(300),
    };
    rollback.admit(&promotion, 250).unwrap();
    RollbackReceipt {
        identity: [30; 32],
        grant_identity: rollback.identity,
        subject_identity: contract.subject_identity,
        before_plan_identity: [26; 32],
        after_plan_identity: Some([31; 32]),
        attempt: 1,
        selected: Some(contract.baseline),
        terminal: RollbackTerminal::RolledBack,
    }
    .validate(&rollback)
    .unwrap();
}

#[test]
fn shadow_candidate_cannot_share_a_protected_effect_route() {
    let mut value = contract();
    value.candidate_output_route = "effect/protected-actuator".into();
    assert_eq!(
        value.validate(),
        Err(LearnedLifecycleRefusal::EffectfulShadowRoute)
    );
}

#[test]
fn shadow_requires_exact_shared_input_and_stays_within_finite_resources() {
    let contract = contract();
    let mut value = run(&contract);
    value.shared_input_set_identity = [98; 32];
    assert_eq!(
        contract.validate_run(&value),
        Err(LearnedLifecycleRefusal::WrongInput)
    );
    let mut value = run(&contract);
    value.candidate_evidence.input_identities = vec![[99; 32]];
    assert_eq!(
        contract.validate_run(&value),
        Err(LearnedLifecycleRefusal::WrongInput)
    );
    let mut value = run(&contract);
    value.consumed_work_units = 101;
    assert_eq!(
        contract.validate_run(&value),
        Err(LearnedLifecycleRefusal::ResourceBoundExceeded)
    );
}

#[test]
fn favorable_metrics_do_not_grant_promotion_authority() {
    let contract = contract();
    let evaluation = evaluation_fixture(&contract);
    evaluation.validate(&contract).unwrap();
    let mut grant = grant_fixture(&contract, &evaluation);
    grant.decision = PromotionDecision::Denied;
    assert_eq!(
        grant.admit(&evaluation, 150),
        Err(LearnedLifecycleRefusal::ApprovalDenied)
    );
}

#[test]
fn stale_candidate_disagreement_and_mutated_plan_refuse_distinctly() {
    let contract = contract();
    let mut evaluation = evaluation_fixture(&contract);
    evaluation.disposition = EvaluationDisposition::Disagreement;
    let grant = grant_fixture(&contract, &evaluation);
    assert_eq!(
        grant.admit(&evaluation, 150),
        Err(LearnedLifecycleRefusal::EvidenceInsufficient)
    );

    let evaluation = evaluation_fixture(&contract);
    let mut grant = grant_fixture(&contract, &evaluation);
    grant.candidate.checkpoint = Some([88; 32]);
    assert_eq!(
        grant.admit(&evaluation, 150),
        Err(LearnedLifecycleRefusal::StaleCandidate)
    );

    let grant = grant_fixture(&contract, &evaluation);
    let receipt = PromotionReceipt {
        identity: [23; 32],
        grant_identity: grant.identity,
        subject_identity: grant.subject_identity,
        before_plan_identity: [24; 32],
        before_play_identity: None,
        after_plan_identity: Some([24; 32]),
        after_play_identity: None,
        selected: grant.candidate,
        rollback_target: grant.rollback_target,
        terminal: PromotionTerminal::Promoted,
    };
    assert_eq!(
        receipt.validate(&grant),
        Err(LearnedLifecycleRefusal::InvalidPlanTransition)
    );
}

#[test]
fn rollback_is_separately_authorized_bounded_and_cannot_revive_another_target() {
    let contract = contract();
    let evaluation = evaluation_fixture(&contract);
    let grant = grant_fixture(&contract, &evaluation);
    let promotion = PromotionReceipt {
        identity: [23; 32],
        grant_identity: grant.identity,
        subject_identity: grant.subject_identity,
        before_plan_identity: [24; 32],
        before_play_identity: None,
        after_plan_identity: Some([26; 32]),
        after_play_identity: None,
        selected: grant.candidate,
        rollback_target: grant.rollback_target,
        terminal: PromotionTerminal::Promoted,
    };
    let mut rollback = RollbackGrant {
        identity: [28; 32],
        authority_identity: [29; 32],
        subject_identity: grant.subject_identity,
        promotion_receipt_identity: promotion.identity,
        current: grant.candidate,
        target: realization(Some(77), 3),
        maximum_attempts: 1,
        valid_until_tick: Some(300),
    };
    assert_eq!(
        rollback.admit(&promotion, 250),
        Err(LearnedLifecycleRefusal::RollbackTargetMismatch)
    );
    rollback.target = grant.rollback_target;
    rollback.admit(&promotion, 250).unwrap();
    let receipt = RollbackReceipt {
        identity: [30; 32],
        grant_identity: rollback.identity,
        subject_identity: rollback.subject_identity,
        before_plan_identity: [26; 32],
        after_plan_identity: None,
        attempt: 2,
        selected: None,
        terminal: RollbackTerminal::Failed,
    };
    assert_eq!(
        receipt.validate(&rollback),
        Err(LearnedLifecycleRefusal::AttemptLimitExceeded)
    );
}
