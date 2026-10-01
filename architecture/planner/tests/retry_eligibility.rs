use conduit_core::{
    bind_active_play, bind_sign, kind_id, ArtifactId, Back, BootId, CapabilityId, CapabilityLimits,
    ExecutionProfileId, ExternalEffectBehavior, HostCallContractId, HostId, ImplementationId, Kind,
    KindIdentity, KindSemanticLaw, PlacementId, PlanId, ReplayBehavior, RetainedRetryEvidence,
    RetainedRetryProof, RetainedTransactionDisposition, RetryAttemptProvenance,
    RetryEvidenceRefusal, RetryFingerprint, RetryOperationIdentity, SemanticDependence,
    SuspensionBehavior, TemporalStateBehavior, TransformationRefusal, VariabilityBehavior,
};
use conduit_planner::{admit_explicit_retry, RetryAdmissionBasis};

fn effectful_kind(replay: ReplayBehavior) -> Kind {
    Kind {
        startup_parameters: Vec::new(),
        shorthand: None,
        kind_id: kind_id("payment/charge"),
        kind_contract_revision: KindIdentity::from("payment/charge@1"),
        inputs: Vec::new(),
        outputs: Vec::new(),
        configuration: Vec::new(),
        semantic_laws: vec![
            KindSemanticLaw::ExternalEffects(ExternalEffectBehavior::Observable),
            KindSemanticLaw::TemporalState(TemporalStateBehavior::None),
            KindSemanticLaw::TimeDependence(SemanticDependence::None),
            KindSemanticLaw::RandomDependence(SemanticDependence::None),
            KindSemanticLaw::ResourceDependence(SemanticDependence::None),
            KindSemanticLaw::Suspension(SuspensionBehavior::Never),
            KindSemanticLaw::Variability(VariabilityBehavior::DeterministicFromInputs),
            KindSemanticLaw::Replay(replay),
        ],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: 64,
        },
    }
}

fn back() -> Back {
    Back {
        capability_id: CapabilityId::from("capability/payment"),
        execution_profile_id: ExecutionProfileId::from("profile/payment"),
        implementation_id: ImplementationId::from("provider/payment"),
        artifact_id: ArtifactId::from("artifact/payment"),
        host_calls: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
    }
}

fn evidence(disposition: RetainedTransactionDisposition) -> RetainedRetryEvidence {
    let active_play = bind_active_play(
        &PlanId::from("plan/first-attempt"),
        &HostId::from("host/provider"),
        &BootId::from("boot/provider/1"),
        4,
    );
    let supporting_sign = bind_sign(
        &active_play.host_id,
        &active_play.boot_id,
        Some(&active_play.active_play_id),
        8,
    );
    let fingerprint = |domain| RetryFingerprint::derive(domain, b"invoice/42").unwrap();
    RetainedRetryEvidence {
        operation: RetryOperationIdentity {
            kind_id: kind_id("payment/charge"),
            kind_contract_revision: KindIdentity::from("payment/charge@1"),
            implementation_id: ImplementationId::from("provider/payment"),
            operation_key_kind: kind_id("payment/invoice-id@1"),
            operation_key_fingerprint: fingerprint("operation-key"),
        },
        replay_law: ReplayBehavior::Transactional {
            transaction_contract: kind_id("payment/transaction@1"),
        },
        attempt: RetryAttemptProvenance {
            active_play,
            placement_id: PlacementId::from("placement/charge"),
            host_call_contract_id: HostCallContractId::from("payment/provider@1"),
            request_sequence: 2,
        },
        supporting_sign,
        proof: RetainedRetryProof::TransactionRetained {
            transaction_contract: kind_id("payment/transaction@1"),
            transaction_fingerprint: fingerprint("transaction"),
            disposition,
            receipt: fingerprint("transaction-receipt"),
        },
    }
}

#[test]
fn planner_seals_the_exact_retained_evidence_without_creating_a_retry_loop() {
    let replay = ReplayBehavior::Transactional {
        transaction_contract: kind_id("payment/transaction@1"),
    };
    let evidence = evidence(RetainedTransactionDisposition::NotCommitted);
    let admission = admit_explicit_retry(
        &effectful_kind(replay),
        &back(),
        &evidence.operation,
        &evidence,
    )
    .unwrap();
    assert_eq!(admission.operation, evidence.operation);
    assert_eq!(
        admission.basis,
        RetryAdmissionBasis::RetainedEffect {
            supporting_sign_id: evidence.supporting_sign.sign_id.clone(),
            evidence: Box::new(evidence),
        }
    );
}

#[test]
fn a_committed_transaction_cannot_be_replayed_by_replanning() {
    let replay = ReplayBehavior::Transactional {
        transaction_contract: kind_id("payment/transaction@1"),
    };
    let evidence = evidence(RetainedTransactionDisposition::Committed);
    assert_eq!(
        admit_explicit_retry(
            &effectful_kind(replay),
            &back(),
            &evidence.operation,
            &evidence,
        ),
        Err(TransformationRefusal::RetryEvidence(
            RetryEvidenceRefusal::DispositionDoesNotAuthorizeRetry
        ))
    );
}
