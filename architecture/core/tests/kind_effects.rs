use conduit_core::{
    kind_id, ArtifactId, AuthorityContractId, AuthorityRequirement, Back, CapabilityId,
    CapabilityLimits, ExecutionProfileId, ExternalEffectBehavior, HostCallContractId,
    HostCallRequirement, ImplementationId, Kind, KindIdentity, KindSemanticLaw, PureExpressionFact,
    PureExpressionRefusal, ReplayBehavior, ResourceClassId, ResourceRequirement,
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
