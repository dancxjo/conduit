use conduit_core::{
    bind_active_play, bind_sign, kind_id, ArtifactId, AuthorityContractId, AuthorityRequirement,
    Back, BootId, CapabilityId, CapabilityLimits, ExecutionProfileId, ExternalEffectBehavior,
    HostCallContractId, HostCallRequirement, HostId, ImplementationId, Kind, KindIdentity,
    KindSemanticLaw, PlacementId, PlanId, PureExpressionFact, PureExpressionRefusal,
    ReplayBehavior, ResourceClassId, ResourceRequirement, RetainedRetryEvidence,
    RetainedRetryProof, RetainedTransactionDisposition, RetryAttemptProvenance,
    RetryEvidenceLedger, RetryEvidenceRefusal, RetryFingerprint, RetryOperationIdentity,
    SemanticDependence, SuspensionBehavior, TemporalStateBehavior, TransformationRefusal,
    VariabilityBehavior, WorkTransformation,
};

fn pure_laws() -> Vec<KindSemanticLaw> {
    vec![
        KindSemanticLaw::ExternalEffects(ExternalEffectBehavior::None),
        KindSemanticLaw::TemporalState(TemporalStateBehavior::None),
        KindSemanticLaw::TimeDependence(SemanticDependence::None),
        KindSemanticLaw::RandomDependence(SemanticDependence::None),
        KindSemanticLaw::ResourceDependence(SemanticDependence::None),
        KindSemanticLaw::Suspension(SuspensionBehavior::Never),
        KindSemanticLaw::Variability(VariabilityBehavior::DeterministicFromInputs),
        KindSemanticLaw::Replay(ReplayBehavior::Exact),
    ]
}

fn kind(laws: Vec<KindSemanticLaw>) -> Kind {
    Kind {
        startup_parameters: Vec::new(),
        shorthand: None,
        kind_id: kind_id("math/sine"),
        kind_contract_revision: KindIdentity::from("conduit.math/sine@1"),
        inputs: Vec::new(),
        outputs: Vec::new(),
        configuration: Vec::new(),
        semantic_laws: laws,
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: 16,
        },
    }
}

fn back() -> Back {
    Back {
        capability_id: CapabilityId::from("capability"),
        execution_profile_id: ExecutionProfileId::from("profile"),
        implementation_id: ImplementationId::from("implementation"),
        artifact_id: ArtifactId::from("artifact"),
        host_calls: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
    }
}

fn retry_operation() -> RetryOperationIdentity {
    RetryOperationIdentity {
        kind_id: kind_id("math/sine"),
        kind_contract_revision: KindIdentity::from("conduit.math/sine@1"),
        implementation_id: ImplementationId::from("implementation"),
        operation_key_kind: kind_id("payment/operation-key@1"),
        operation_key_fingerprint: RetryFingerprint::derive(
            "payment/operation-key@1",
            b"invoice/42",
        )
        .unwrap(),
    }
}

fn retained_retry_evidence(
    replay_law: ReplayBehavior,
    proof: RetainedRetryProof,
) -> RetainedRetryEvidence {
    let active_play = bind_active_play(
        &PlanId::from("plan/prior"),
        &HostId::from("host/provider"),
        &BootId::from("boot/provider/1"),
        7,
    );
    let supporting_sign = bind_sign(
        &active_play.host_id,
        &active_play.boot_id,
        Some(&active_play.active_play_id),
        9,
    );
    RetainedRetryEvidence {
        operation: retry_operation(),
        replay_law,
        attempt: RetryAttemptProvenance {
            active_play,
            placement_id: PlacementId::from("placement/payment"),
            host_call_contract_id: HostCallContractId::from("payment/provider@1"),
            request_sequence: 3,
        },
        supporting_sign,
        proof,
    }
}

fn fingerprint(domain: &str) -> RetryFingerprint {
    RetryFingerprint::derive(domain, b"exact-retained-receipt").unwrap()
}

#[test]
fn every_independent_semantic_fact_is_required_for_pure_calls() {
    assert!(conduit_core::pure_expression_facts(&kind(pure_laws())).is_ok());

    let mut missing = pure_laws();
    missing.pop();
    assert_eq!(
        conduit_core::pure_expression_facts(&kind(missing)),
        Err(PureExpressionRefusal::MissingFact(
            PureExpressionFact::Replay
        ))
    );

    let mut duplicate = pure_laws();
    duplicate.push(KindSemanticLaw::Replay(ReplayBehavior::Exact));
    assert_eq!(
        conduit_core::pure_expression_facts(&kind(duplicate)),
        Err(PureExpressionRefusal::DuplicateFact(
            PureExpressionFact::Replay
        ))
    );
}

#[test]
fn each_impure_semantic_dimension_refuses_separately() {
    let cases = [
        (
            0,
            KindSemanticLaw::ExternalEffects(ExternalEffectBehavior::Observable),
            PureExpressionRefusal::ExternalEffect,
        ),
        (
            1,
            KindSemanticLaw::TemporalState(TemporalStateBehavior::Retained),
            PureExpressionRefusal::TemporalState,
        ),
        (
            2,
            KindSemanticLaw::TimeDependence(SemanticDependence::Ambient),
            PureExpressionRefusal::TimeDependence,
        ),
        (
            3,
            KindSemanticLaw::RandomDependence(SemanticDependence::Ambient),
            PureExpressionRefusal::RandomDependence,
        ),
        (
            4,
            KindSemanticLaw::ResourceDependence(SemanticDependence::Ambient),
            PureExpressionRefusal::ResourceDependence,
        ),
        (
            5,
            KindSemanticLaw::Suspension(SuspensionBehavior::MaySuspend),
            PureExpressionRefusal::Suspension,
        ),
        (
            6,
            KindSemanticLaw::Variability(VariabilityBehavior::AdmittedVariability),
            PureExpressionRefusal::Variability,
        ),
        (
            7,
            KindSemanticLaw::Replay(ReplayBehavior::Ineligible),
            PureExpressionRefusal::ReplayIneligible,
        ),
    ];
    for (index, law, refusal) in cases {
        let mut laws = pure_laws();
        laws[index] = law;
        assert_eq!(
            conduit_core::pure_expression_facts(&kind(laws)),
            Err(refusal)
        );
    }
}

#[test]
fn explicit_semantic_inputs_do_not_become_hidden_ambient_dependencies() {
    let mut laws = pure_laws();
    laws[2] = KindSemanticLaw::TimeDependence(SemanticDependence::ExplicitInput);
    laws[3] = KindSemanticLaw::RandomDependence(SemanticDependence::ExplicitInput);
    laws[4] = KindSemanticLaw::ResourceDependence(SemanticDependence::ExplicitInput);
    let facts = conduit_core::pure_expression_facts(&kind(laws)).unwrap();
    assert_eq!(facts.time_dependence, SemanticDependence::ExplicitInput);
    assert_eq!(facts.random_dependence, SemanticDependence::ExplicitInput);
    assert_eq!(facts.resource_dependence, SemanticDependence::ExplicitInput);
}

#[test]
fn selected_back_cannot_hide_effect_authority_or_resource_envelopes() {
    assert_eq!(conduit_core::check_pure_expression_back(&back()), Ok(()));

    let mut host_call = back();
    host_call.host_calls.push(HostCallRequirement {
        contract_id: HostCallContractId::from("test/host-call"),
        target_kind: None,
        maximum_in_flight: 1,
        maximum_input_bytes: 1,
        maximum_output_bytes: 1,
    });
    assert_eq!(
        conduit_core::check_pure_expression_back(&host_call),
        Err(PureExpressionRefusal::BackHostCall)
    );

    let mut resource = back();
    resource.resource_requirements.push(ResourceRequirement {
        class_id: ResourceClassId::from("test/resource"),
        units: 1,
        protected_role: None,
        compute: None,
        content: None,
    });
    assert_eq!(
        conduit_core::check_pure_expression_back(&resource),
        Err(PureExpressionRefusal::BackResource)
    );

    let mut authority = back();
    authority.authority_requirements.push(AuthorityRequirement {
        contract_id: AuthorityContractId::from("test/authority"),
        host_call_contract_id: HostCallContractId::from("test/host-call"),
        subject_kind: kind_id("test/subject"),
    });
    assert_eq!(
        conduit_core::check_pure_expression_back(&authority),
        Err(PureExpressionRefusal::BackAuthority)
    );
}

#[test]
fn pure_bounded_work_is_eligible_for_every_observationally_exact_transformation() {
    let eligibility =
        conduit_core::derive_transformation_eligibility(&kind(pure_laws()), &back()).unwrap();
    for transformation in [
        WorkTransformation::Recompute,
        WorkTransformation::Fusion,
        WorkTransformation::Replay,
        WorkTransformation::Retry,
        WorkTransformation::Memoize,
        WorkTransformation::Move,
    ] {
        assert_eq!(eligibility.require(transformation), Ok(()));
    }

    let mut effectful = pure_laws();
    effectful[0] = KindSemanticLaw::ExternalEffects(ExternalEffectBehavior::Observable);
    let effectful =
        conduit_core::derive_transformation_eligibility(&kind(effectful), &back()).unwrap();
    assert_eq!(
        effectful.require(WorkTransformation::Memoize),
        Err(TransformationRefusal::ExternalEffect)
    );
    assert_eq!(
        effectful.require(WorkTransformation::Retry),
        Err(TransformationRefusal::MissingEffectRetryLaw)
    );
}

#[test]
fn effectful_retry_requires_retained_consumer_evidence() {
    let operation_key = kind_id("payment/operation-key@1");
    let transaction = kind_id("payment/transaction@1");
    let compensation = kind_id("payment/compensation@1");
    for law in [
        ReplayBehavior::Idempotent {
            operation_key_kind: operation_key,
        },
        ReplayBehavior::Transactional {
            transaction_contract: transaction,
        },
        ReplayBehavior::Compensatable {
            compensation_contract: compensation,
        },
    ] {
        let mut laws = pure_laws();
        laws[0] = KindSemanticLaw::ExternalEffects(ExternalEffectBehavior::Observable);
        laws[7] = KindSemanticLaw::Replay(law.clone());
        let eligibility =
            conduit_core::derive_transformation_eligibility(&kind(laws), &back()).unwrap();
        assert_eq!(
            eligibility.require(WorkTransformation::Replay),
            Err(TransformationRefusal::ExternalEffect)
        );
        assert_eq!(
            eligibility.require(WorkTransformation::Retry),
            Err(TransformationRefusal::RetainedRetryEvidenceRequired(law))
        );
    }
}

#[test]
fn stronger_effect_law_does_not_erase_other_retry_axes() {
    let mut base = pure_laws();
    base[0] = KindSemanticLaw::ExternalEffects(ExternalEffectBehavior::Observable);
    base[7] = KindSemanticLaw::Replay(ReplayBehavior::Idempotent {
        operation_key_kind: kind_id("payment/operation-key@1"),
    });

    for (index, law, refusal) in [
        (
            1,
            KindSemanticLaw::TemporalState(TemporalStateBehavior::Retained),
            TransformationRefusal::TemporalState,
        ),
        (
            2,
            KindSemanticLaw::TimeDependence(SemanticDependence::Ambient),
            TransformationRefusal::AmbientTime,
        ),
        (
            3,
            KindSemanticLaw::RandomDependence(SemanticDependence::Ambient),
            TransformationRefusal::AmbientRandom,
        ),
        (
            4,
            KindSemanticLaw::ResourceDependence(SemanticDependence::Ambient),
            TransformationRefusal::AmbientResource,
        ),
        (
            5,
            KindSemanticLaw::Suspension(SuspensionBehavior::MaySuspend),
            TransformationRefusal::Suspension,
        ),
        (
            6,
            KindSemanticLaw::Variability(VariabilityBehavior::AdmittedVariability),
            TransformationRefusal::Variability,
        ),
    ] {
        let mut laws = base.clone();
        laws[index] = law;
        let eligibility =
            conduit_core::derive_transformation_eligibility(&kind(laws), &back()).unwrap();
        assert_eq!(eligibility.require(WorkTransformation::Retry), Err(refusal));
    }
}

#[test]
fn matching_retained_evidence_authorizes_only_the_promised_effect_retry() {
    let operation_key_kind = kind_id("payment/operation-key@1");
    let transaction_contract = kind_id("payment/transaction@1");
    let compensation_contract = kind_id("payment/compensation@1");
    let cases = [
        (
            ReplayBehavior::Idempotent {
                operation_key_kind: operation_key_kind.clone(),
            },
            RetainedRetryProof::IdempotencyKeyRetained {
                receipt: fingerprint("idempotency-receipt"),
            },
        ),
        (
            ReplayBehavior::Transactional {
                transaction_contract: transaction_contract.clone(),
            },
            RetainedRetryProof::TransactionRetained {
                transaction_contract,
                transaction_fingerprint: fingerprint("transaction"),
                disposition: RetainedTransactionDisposition::NotCommitted,
                receipt: fingerprint("transaction-receipt"),
            },
        ),
        (
            ReplayBehavior::Compensatable {
                compensation_contract: compensation_contract.clone(),
            },
            RetainedRetryProof::CompensationCommitted {
                compensation_contract,
                compensation_fingerprint: fingerprint("compensation"),
                receipt: fingerprint("compensation-receipt"),
            },
        ),
    ];
    for (law, proof) in cases {
        let mut laws = pure_laws();
        laws[0] = KindSemanticLaw::ExternalEffects(ExternalEffectBehavior::Observable);
        laws[7] = KindSemanticLaw::Replay(law.clone());
        let eligibility =
            conduit_core::derive_transformation_eligibility(&kind(laws), &back()).unwrap();
        let evidence = retained_retry_evidence(law, proof);
        let authorization = eligibility
            .require_retry_with_evidence(&evidence.operation, &evidence)
            .unwrap();
        assert_eq!(authorization.retained_evidence(), Some(&evidence));
    }
}

#[test]
fn exact_not_committed_receipt_is_distinct_from_failure_or_unknown_commit_truth() {
    let law = ReplayBehavior::Transactional {
        transaction_contract: kind_id("payment/transaction@1"),
    };
    let mut laws = pure_laws();
    laws[0] = KindSemanticLaw::ExternalEffects(ExternalEffectBehavior::Observable);
    laws[7] = KindSemanticLaw::Replay(law.clone());
    let eligibility =
        conduit_core::derive_transformation_eligibility(&kind(laws), &back()).unwrap();

    let not_committed = retained_retry_evidence(
        law.clone(),
        RetainedRetryProof::EffectNotCommitted {
            receipt: fingerprint("provider-not-committed"),
        },
    );
    assert!(eligibility
        .require_retry_with_evidence(&not_committed.operation, &not_committed)
        .is_ok());

    for disposition in [
        RetainedTransactionDisposition::Committed,
        RetainedTransactionDisposition::Unknown,
    ] {
        let retained = retained_retry_evidence(
            law.clone(),
            RetainedRetryProof::TransactionRetained {
                transaction_contract: kind_id("payment/transaction@1"),
                transaction_fingerprint: fingerprint("transaction"),
                disposition,
                receipt: fingerprint("transaction-receipt"),
            },
        );
        assert_eq!(
            eligibility.require_retry_with_evidence(&retained.operation, &retained),
            Err(TransformationRefusal::RetryEvidence(
                RetryEvidenceRefusal::DispositionDoesNotAuthorizeRetry
            ))
        );
    }
}

#[test]
fn stale_contract_back_operation_and_law_evidence_refuse_distinctly() {
    let law = ReplayBehavior::Idempotent {
        operation_key_kind: kind_id("payment/operation-key@1"),
    };
    let mut laws = pure_laws();
    laws[0] = KindSemanticLaw::ExternalEffects(ExternalEffectBehavior::Observable);
    laws[7] = KindSemanticLaw::Replay(law.clone());
    let eligibility =
        conduit_core::derive_transformation_eligibility(&kind(laws), &back()).unwrap();
    let evidence = retained_retry_evidence(
        law.clone(),
        RetainedRetryProof::IdempotencyKeyRetained {
            receipt: fingerprint("idempotency-receipt"),
        },
    );

    let mut changed = evidence.operation.clone();
    changed.kind_contract_revision = KindIdentity::from("conduit.math/sine@2");
    assert_eq!(
        eligibility.require_retry_with_evidence(&changed, &evidence),
        Err(TransformationRefusal::RetryEvidence(
            RetryEvidenceRefusal::KindRevisionMismatch
        ))
    );
    let mut changed = evidence.operation.clone();
    changed.implementation_id = ImplementationId::from("other-implementation");
    assert_eq!(
        eligibility.require_retry_with_evidence(&changed, &evidence),
        Err(TransformationRefusal::RetryEvidence(
            RetryEvidenceRefusal::ImplementationMismatch
        ))
    );
    let mut changed = evidence.operation.clone();
    changed.operation_key_fingerprint =
        RetryFingerprint::derive("payment/operation-key@1", b"invoice/43").unwrap();
    assert_eq!(
        eligibility.require_retry_with_evidence(&changed, &evidence),
        Err(TransformationRefusal::RetryEvidence(
            RetryEvidenceRefusal::OperationMismatch
        ))
    );
    let mut wrong_law = evidence.clone();
    wrong_law.replay_law = ReplayBehavior::Transactional {
        transaction_contract: kind_id("payment/transaction@1"),
    };
    assert_eq!(
        eligibility.require_retry_with_evidence(&wrong_law.operation, &wrong_law),
        Err(TransformationRefusal::RetryEvidence(
            RetryEvidenceRefusal::ReplayLawMismatch
        ))
    );
    let mut wrong_key_kind = evidence.clone();
    wrong_key_kind.operation.operation_key_kind = kind_id("payment/other-key@1");
    assert_eq!(
        eligibility.require_retry_with_evidence(&wrong_key_kind.operation, &wrong_key_kind),
        Err(TransformationRefusal::RetryEvidence(
            RetryEvidenceRefusal::ReplayLawMismatch
        ))
    );
}

#[test]
fn retry_evidence_retention_is_bounded_append_only_and_operation_exact() {
    let law = ReplayBehavior::Idempotent {
        operation_key_kind: kind_id("payment/operation-key@1"),
    };
    let first = retained_retry_evidence(
        law.clone(),
        RetainedRetryProof::IdempotencyKeyRetained {
            receipt: fingerprint("first-receipt"),
        },
    );
    let mut second = retained_retry_evidence(
        law,
        RetainedRetryProof::IdempotencyKeyRetained {
            receipt: fingerprint("second-receipt"),
        },
    );
    second.operation.operation_key_fingerprint =
        RetryFingerprint::derive("payment/operation-key@1", b"invoice/43").unwrap();

    let mut later = first.clone();
    later.proof = RetainedRetryProof::IdempotencyKeyRetained {
        receipt: fingerprint("later-receipt"),
    };
    later.supporting_sign = bind_sign(
        &later.attempt.active_play.host_id,
        &later.attempt.active_play.boot_id,
        Some(&later.attempt.active_play.active_play_id),
        10,
    );

    let mut ledger = RetryEvidenceLedger::new(2).unwrap();
    ledger.admit(first.clone()).unwrap();
    assert_eq!(
        ledger.evidence_for(&first.operation).collect::<Vec<_>>(),
        vec![&first]
    );
    assert_eq!(
        ledger.admit(first.clone()),
        Err(RetryEvidenceRefusal::DuplicateEvidence)
    );
    ledger.admit(later.clone()).unwrap();
    assert_eq!(
        ledger.evidence_for(&first.operation).collect::<Vec<_>>(),
        vec![&first, &later]
    );
    assert_eq!(
        ledger.admit(second),
        Err(RetryEvidenceRefusal::CapacityExhausted)
    );
    assert_eq!(ledger.entries(), &[first, later]);
}
