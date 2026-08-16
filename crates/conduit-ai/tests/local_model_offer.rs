use conduit_ai::{
    LlmDeterminismProfile, LlmWorkBounds, LocalModelCachePolicy, LocalModelFailure,
    LocalModelIdentity, LocalModelKindProfile, LocalModelLifecycleState, LocalModelLimits,
    LocalModelOffer, LocalModelOfferInvalidity, LocalModelRefusal, LocalModelRequestAdmission,
    LocalModelSession, LocalModelTerminal, LOCAL_MODEL_MEMORY_RESOURCE, LOCAL_MODEL_OPERATION,
};

fn offer() -> LocalModelOffer {
    LocalModelOffer {
        identity: LocalModelIdentity {
            runtime_name: "ollama".into(),
            runtime_version: "0.23.0".into(),
            runtime_build_identity: "ollama/0.23.0/linux-amd64".into(),
            model_name: "llama3.2:latest".into(),
            model_content_identity: "a80c4f17acd5".into(),
            architecture: "llama".into(),
            parameter_profile: "3.2B".into(),
            quantization: "Q4_K_M".into(),
        },
        limits: LocalModelLimits {
            work: LlmWorkBounds {
                maximum_input_bytes: 4_096,
                maximum_context_items: 1,
                maximum_output_bytes: 1_024,
                maximum_work_units: 4_096,
                maximum_history_items: 0,
            },
            model_bytes: 2_000_000_000,
            admitted_memory_mib: 8_192,
            maximum_in_flight: 1,
            maximum_queue_items: 4,
            maximum_queue_bytes: 16_384,
            cancellation_supported: true,
            cache_policy: LocalModelCachePolicy::OneLoadedModelUntilShutdown,
        },
        supported_profiles: vec![
            LocalModelKindProfile::Generate,
            LocalModelKindProfile::ClassifyFiniteLabels,
            LocalModelKindProfile::ExtractValidatedInfo,
            LocalModelKindProfile::EmbedFiniteVector,
            LocalModelKindProfile::InterpretSignEvidence,
        ],
        initialized: true,
        lifecycle: LocalModelLifecycleState::Ready,
        determinism: LlmDeterminismProfile::ProviderNondeterministic,
    }
}

#[test]
fn initialized_offer_exposes_five_exact_finite_l0_capabilities() {
    let offers = offer().capability_offers().unwrap();
    assert_eq!(offers.len(), 5);
    assert_eq!(offers[0].kind_id.as_str(), "llm/generate");
    assert_eq!(offers[1].kind_id.as_str(), "llm/classify");
    assert_eq!(offers[2].kind_id.as_str(), "llm/extract");
    assert_eq!(offers[3].kind_id.as_str(), "llm/embed");
    assert_eq!(offers[4].kind_id.as_str(), "llm/interpret");
    for capability in offers {
        assert_eq!(capability.host_operations.len(), 1);
        assert_eq!(
            capability.host_operations[0].contract_id.as_str(),
            LOCAL_MODEL_OPERATION
        );
        assert_eq!(capability.resource_requirements.len(), 1);
        assert_eq!(
            capability.resource_requirements[0].class_id.as_str(),
            LOCAL_MODEL_MEMORY_RESOURCE
        );
        assert_eq!(capability.limits.max_active_instances, 1);
        assert_eq!(capability.limits.max_queue_items, 4);
        assert_eq!(capability.limits.max_queue_bytes, 16_384);
        assert!(capability.authority_requirements.is_empty());
        assert_eq!(
            capability.implementation.implementation_id.as_str(),
            "std/local-open-weight-model@1"
        );
        assert!(capability
            .implementation
            .artifact_id
            .as_str()
            .ends_with("/a80c4f17acd5"));
    }
}

#[test]
fn discovery_is_not_an_offer_until_initialization_reaches_ready() {
    let mut candidate = offer();
    candidate.initialized = false;
    candidate.lifecycle = LocalModelLifecycleState::Discovered;
    assert_eq!(
        candidate.validate(),
        Err(LocalModelOfferInvalidity::NotReady)
    );
    assert!(candidate.capability_offers().is_err());
}

#[test]
fn finite_memory_concurrency_queue_and_identity_fail_closed() {
    let mut candidate = offer();
    candidate.limits.admitted_memory_mib = 1_000;
    assert_eq!(
        candidate.validate(),
        Err(LocalModelOfferInvalidity::ModelExceedsMemoryCeiling)
    );

    let mut candidate = offer();
    candidate.limits.maximum_in_flight = 2;
    assert_eq!(
        candidate.validate(),
        Err(LocalModelOfferInvalidity::InvalidConcurrency)
    );

    let mut candidate = offer();
    candidate.limits.maximum_queue_bytes = 16_383;
    assert_eq!(
        candidate.validate(),
        Err(LocalModelOfferInvalidity::InvalidQueue)
    );

    let mut candidate = offer();
    candidate.identity.model_content_identity.clear();
    assert_eq!(
        candidate.validate(),
        Err(LocalModelOfferInvalidity::MissingIdentity)
    );
}

#[test]
fn profiles_are_exact_unique_and_never_claim_fixture_determinism() {
    let mut candidate = offer();
    candidate
        .supported_profiles
        .push(LocalModelKindProfile::Generate);
    assert_eq!(
        candidate.validate(),
        Err(LocalModelOfferInvalidity::MissingProfile)
    );

    let mut candidate = offer();
    candidate.supported_profiles = vec![
        LocalModelKindProfile::Generate,
        LocalModelKindProfile::Generate,
    ];
    assert_eq!(
        candidate.validate(),
        Err(LocalModelOfferInvalidity::DuplicateProfile)
    );

    let mut candidate = offer();
    candidate.determinism = LlmDeterminismProfile::DeterministicValidationFixture;
    assert_eq!(
        candidate.validate(),
        Err(LocalModelOfferInvalidity::DeterministicClaim)
    );
}

fn request() -> LocalModelRequestAdmission {
    LocalModelRequestAdmission {
        input_bytes: 128,
        context_items: 1,
        output_bytes: 256,
        work_units: 512,
        history_items: 0,
    }
}

#[test]
fn admission_refuses_each_finite_bound_before_inference() {
    let mut session = LocalModelSession::new(offer()).unwrap();
    let mut oversized = request();
    oversized.input_bytes = 4_097;
    assert_eq!(
        session.admit(LocalModelKindProfile::Generate, oversized),
        Err(LocalModelRefusal::InputOverflow)
    );
    assert_eq!(session.state(), LocalModelLifecycleState::Ready);

    let mut oversized = request();
    oversized.context_items = 2;
    assert_eq!(
        session.admit(LocalModelKindProfile::Generate, oversized),
        Err(LocalModelRefusal::ContextOverflow)
    );

    let mut oversized = request();
    oversized.output_bytes = 1_025;
    assert_eq!(
        session.admit(LocalModelKindProfile::Generate, oversized),
        Err(LocalModelRefusal::OutputOverflow)
    );

    let mut oversized = request();
    oversized.work_units = 4_097;
    assert_eq!(
        session.admit(LocalModelKindProfile::Generate, oversized),
        Err(LocalModelRefusal::UnsupportedProfile)
    );
}

#[test]
fn inference_cancellation_loss_resource_failure_and_shutdown_stay_distinct() {
    let mut session = LocalModelSession::new(offer()).unwrap();
    session
        .admit(LocalModelKindProfile::Generate, request())
        .unwrap();
    assert_eq!(session.state(), LocalModelLifecycleState::Inference);
    assert_eq!(session.cancel().unwrap(), LocalModelTerminal::Cancelled);
    assert_eq!(session.state(), LocalModelLifecycleState::Ready);

    session
        .admit(LocalModelKindProfile::ClassifyFiniteLabels, request())
        .unwrap();
    session
        .finish(LocalModelTerminal::Failed(
            LocalModelFailure::ResourceExhausted,
        ))
        .unwrap();
    assert_eq!(session.state(), LocalModelLifecycleState::Ready);

    session
        .admit(LocalModelKindProfile::Generate, request())
        .unwrap();
    assert_eq!(session.provider_lost(), LocalModelTerminal::ProviderLost);
    assert_eq!(session.state(), LocalModelLifecycleState::Lost);
    assert_eq!(
        session.admit(LocalModelKindProfile::Generate, request()),
        Err(LocalModelRefusal::NotInitialized)
    );
    session.shutdown().unwrap();
    assert_eq!(session.state(), LocalModelLifecycleState::Shutdown);
}
