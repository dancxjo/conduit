#[cfg(feature = "form-catalog")]
use conduit_ai::install_llm_semantic_catalog;
use conduit_ai::{
    llm_contract, llm_semantic_catalog, ConfidencePermille, LlmDeterminismProfile,
    LlmImplementationControl, LlmTerminalOutcome, ModelDerivedResult, ModelFailure, ModelRefusal,
    ModelResultDisposition, ModelResultInvalidity, ModelResultProvenance, ModelWorkAccounting,
    LLM_CLASSIFY_KIND, LLM_COMPOSE_KIND, LLM_EMBED_KIND, LLM_EXTRACT_KIND, LLM_GENERATE_KIND,
    LLM_INTERPRET_KIND, LLM_JUDGE_KIND, LLM_PROPOSE_KIND,
};
use conduit_core::PortDirection;

fn produced(kind: &str) -> ModelDerivedResult {
    let contract = llm_contract(kind).unwrap();
    let payload = b"deterministic-fixture-result".to_vec();
    ModelDerivedResult {
        provenance: ModelResultProvenance::ModelDerived,
        payload_kind: contract.result_payload_kind.as_str().to_string(),
        accounting: ModelWorkAccounting {
            input_bytes: 128,
            context_items: 2,
            output_bytes: payload.len() as u64,
            work_units: 500,
            history_items: 1,
        },
        payload,
        implementation_identity: "fixture/model-implementation@sha256:01".to_string(),
        request_identity: "request/0001@sha256:02".to_string(),
        run_identity: "run/0001@sha256:03".to_string(),
        confidence: Some(ConfidencePermille(850)),
        disposition: ModelResultDisposition::Produced,
        determinism: LlmDeterminismProfile::DeterministicValidationFixture,
    }
}

#[test]
fn eight_machine_readable_semantic_contracts_have_exact_distinct_faces() {
    let contracts = llm_semantic_catalog();
    let expected = [
        LLM_GENERATE_KIND,
        LLM_CLASSIFY_KIND,
        LLM_EXTRACT_KIND,
        LLM_EMBED_KIND,
        LLM_INTERPRET_KIND,
        LLM_PROPOSE_KIND,
        LLM_COMPOSE_KIND,
        LLM_JUDGE_KIND,
    ];
    assert_eq!(contracts.len(), expected.len());
    for (contract, expected_kind) in contracts.iter().zip(expected) {
        assert_eq!(contract.kind_id.as_str(), expected_kind);
        assert_eq!(contract.inputs.len(), 1);
        assert_eq!(contract.outputs.len(), 1);
        assert_eq!(contract.inputs[0].direction, PortDirection::Input);
        assert_eq!(contract.outputs[0].direction, PortDirection::Output);
        assert_ne!(
            contract.inputs[0].value_kind,
            contract.outputs[0].value_kind
        );
        assert!(contract.bounds.valid());
        assert_eq!(contract.terminal_outcomes.len(), 6);
        assert_eq!(contract.excluded_implementation_controls.len(), 7);
    }
    let encoded = serde_json::to_string(&contracts).unwrap();
    for forbidden in [
        "OpenAI",
        "Anthropic",
        "ChatCompletion",
        "function_call",
        "prompt",
        "completion",
    ] {
        assert!(
            !encoded.contains(forbidden),
            "forbidden vocabulary: {forbidden}"
        );
    }
}

#[test]
#[cfg(feature = "form-catalog")]
fn all_contracts_install_and_check_as_ordinary_provider_free_forms() {
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    install_llm_semantic_catalog(&mut startup, &mut profile).unwrap();
    for contract in llm_semantic_catalog() {
        let name = contract.kind_id.as_str().replace('/', "-");
        let source = format!("form {name} {{\n gear: {}\n}}\n", contract.kind_id.as_str());
        let syntax = conduit_form::parse_syntax_document(&source);
        let checked = conduit_form::check_syntax_document(&syntax, &startup).unwrap();
        let expanded = conduit_form::expand_canonical_form(&checked, &name, &profile).unwrap();
        assert_eq!(expanded.gears[0].kind_id, contract.kind_id);
    }
}

#[test]
fn model_derived_envelope_is_bounded_correlated_and_not_sign_evidence() {
    let contract = llm_contract(LLM_CLASSIFY_KIND).unwrap();
    let result = produced(LLM_CLASSIFY_KIND);
    result.validate(&contract).unwrap();
    let encoded = serde_json::to_string(&result).unwrap();
    assert!(encoded.contains("ModelDerived"));
    assert!(!encoded.contains("SignId"));
    assert_eq!(
        result.disposition.terminal_outcome(),
        LlmTerminalOutcome::Produced
    );
}

#[test]
fn malformed_oversized_and_unsupported_results_fail_distinctly() {
    let contract = llm_contract(LLM_EXTRACT_KIND).unwrap();
    let mut result = produced(LLM_EXTRACT_KIND);
    result.payload_kind = "llm/unrelated-result@1".to_string();
    assert_eq!(
        result.validate(&contract),
        Err(ModelResultInvalidity::UnsupportedPayloadKind)
    );

    let mut result = produced(LLM_EXTRACT_KIND);
    result.confidence = Some(ConfidencePermille(1_001));
    assert_eq!(
        result.validate(&contract),
        Err(ModelResultInvalidity::InvalidConfidence)
    );

    let mut result = produced(LLM_EXTRACT_KIND);
    result.accounting.output_bytes = contract.bounds.maximum_output_bytes + 1;
    assert_eq!(
        result.validate(&contract),
        Err(ModelResultInvalidity::OutputBoundExceeded)
    );

    let mut result = produced(LLM_EXTRACT_KIND);
    result.request_identity.clear();
    assert_eq!(
        result.validate(&contract),
        Err(ModelResultInvalidity::MissingExactIdentity)
    );
}

#[test]
fn refusal_failure_cancellation_and_provider_loss_are_terminal_without_payload() {
    let contract = llm_contract(LLM_JUDGE_KIND).unwrap();
    for disposition in [
        ModelResultDisposition::Refused(ModelRefusal::UnsupportedRequest),
        ModelResultDisposition::Failed(ModelFailure::ImplementationFailure),
        ModelResultDisposition::Cancelled,
        ModelResultDisposition::ProviderLost,
    ] {
        let mut result = produced(LLM_JUDGE_KIND);
        result.payload.clear();
        result.accounting.output_bytes = 0;
        result.disposition = disposition;
        result.validate(&contract).unwrap();
    }
}

#[test]
fn implementation_controls_and_determinism_claims_stay_outside_portable_semantics() {
    let contract = llm_contract(LLM_GENERATE_KIND).unwrap();
    assert!(contract
        .excluded_implementation_controls
        .contains(&LlmImplementationControl::Temperature));
    assert!(contract
        .excluded_implementation_controls
        .contains(&LlmImplementationControl::ProviderFunctionJson));
    assert!(LlmDeterminismProfile::DeterministicValidationFixture
        .permits_semantic_output_equality_claim());
    for profile in [
        LlmDeterminismProfile::SeededImplementationBestEffort,
        LlmDeterminismProfile::StochasticInference,
        LlmDeterminismProfile::ProviderNondeterministic,
    ] {
        assert!(!profile.permits_semantic_output_equality_claim());
    }
}

#[test]
fn exact_revision_and_face_compatibility_rejects_mutation() {
    let expected = llm_contract(LLM_EMBED_KIND).unwrap();
    let mut mutated = expected.clone();
    mutated.result_payload_kind = conduit_core::kind_id("llm/wrong-result@1");
    assert!(!expected.is_exactly_compatible_with(&mutated));
    assert!(expected.is_exactly_compatible_with(&expected));
}
