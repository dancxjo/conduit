#[cfg(feature = "plot-catalog")]
use conduit_ai::install_llm_semantic_catalog;
use conduit_ai::{
    llm_contract, llm_semantic_catalog, ConfidencePermille, LlmDeterminismProfile,
    LlmImplementationControl, LlmTerminalOutcome, LlmWorkBounds, ModelDerivedResult, ModelFailure,
    ModelRefusal, ModelResultDisposition, ModelResultInvalidity, ModelResultProvenance,
    ModelWorkAccounting, LLM_CLASSIFY_KIND, LLM_COMPOSE_KIND, LLM_EMBED_KIND, LLM_EXTRACT_KIND,
    LLM_GENERATE_FLOW_KIND, LLM_GENERATE_KIND, LLM_GENERATE_RECOVERABLE_FLOW_KIND,
    LLM_INTERPRET_KIND, LLM_JUDGE_KIND, LLM_PRESENT_KIND, LLM_PROPOSE_KIND,
    LLM_STREAM_GENERATE_KIND,
};
use conduit_core::PortDirection;
use conduit_core::PortTemporal;
use conduit_plot::rust_binding::NativeRustBinding;

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
        confidence: Some(ConfidencePermille::new(850).unwrap()),
        disposition: ModelResultDisposition::Produced,
        determinism: LlmDeterminismProfile::DeterministicValidationFixture,
    }
}

#[test]
fn confidence_and_work_accounting_are_native_values() {
    for value in [0, 1_000] {
        let confidence = ConfidencePermille::new(value).unwrap();
        assert_eq!(*confidence.get(), value);
        assert_eq!(
            ConfidencePermille::from_structured(confidence.into_structured().unwrap()).unwrap(),
            confidence
        );
    }
    assert!(ConfidencePermille::new(1_001).is_err());

    let accounting = ModelWorkAccounting {
        input_bytes: 1,
        context_items: 2,
        output_bytes: 3,
        work_units: 4,
        history_items: 5,
    };
    assert_eq!(
        ModelWorkAccounting::from_structured(accounting.into_structured().unwrap()).unwrap(),
        accounting
    );

    let source = include_str!("../src/model_result.rs");
    assert!(!source.contains("pub struct ConfidencePermille"));
    assert!(!source.contains("pub struct ModelWorkAccounting"));
}

#[test]
fn llm_work_bounds_are_native_and_exactly_bounded() {
    let minimum = LlmWorkBounds::new(1, 0, 1, 1, 0).unwrap();
    let maximum = LlmWorkBounds::new(262_144, 128, 65_536, 1_000_000, 64).unwrap();
    for bounds in [minimum, maximum] {
        assert!(bounds.valid());
        assert_eq!(
            LlmWorkBounds::from_structured(bounds.into_structured().unwrap()).unwrap(),
            bounds
        );
    }
    for invalid in [
        LlmWorkBounds::new(0, 0, 1, 1, 0),
        LlmWorkBounds::new(262_145, 0, 1, 1, 0),
        LlmWorkBounds::new(1, 129, 1, 1, 0),
        LlmWorkBounds::new(1, 0, 65_537, 1, 0),
        LlmWorkBounds::new(1, 0, 1, 1_000_001, 0),
        LlmWorkBounds::new(1, 0, 1, 1, 65),
    ] {
        assert!(invalid.is_err());
    }
    assert!(
        !include_str!("../src/llm_contract.rs").contains(concat!("pub struct ", "LlmWorkBounds"))
    );
}

#[test]
fn streaming_generation_is_a_distinct_closing_delta_flow() {
    let contract = conduit_ai::llm_contract(conduit_ai::LLM_STREAM_GENERATE_KIND).unwrap();
    assert_eq!(contract.inputs[0].temporal, PortTemporal::Value);
    assert_eq!(
        contract.outputs[0].temporal,
        PortTemporal::Flow { closes: true }
    );
    assert_eq!(
        contract.outputs[0].value_kind.as_str(),
        conduit_ai::GENERATED_TEXT_CHUNK_VALUE_KIND
    );
    assert_eq!(contract.limits.max_queue_items, 8);
    assert_eq!(
        conduit_ai::llm_contract(conduit_ai::LLM_GENERATE_KIND)
            .unwrap()
            .outputs[0]
            .temporal,
        PortTemporal::Value
    );
}

#[test]
fn machine_readable_semantic_contracts_have_exact_distinct_fronts() {
    let contracts = llm_semantic_catalog();
    let expected = [
        LLM_GENERATE_KIND,
        LLM_GENERATE_FLOW_KIND,
        LLM_GENERATE_RECOVERABLE_FLOW_KIND,
        LLM_CLASSIFY_KIND,
        LLM_EXTRACT_KIND,
        LLM_EMBED_KIND,
        LLM_INTERPRET_KIND,
        LLM_PROPOSE_KIND,
        LLM_COMPOSE_KIND,
        LLM_JUDGE_KIND,
        LLM_PRESENT_KIND,
        LLM_STREAM_GENERATE_KIND,
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
fn generative_face_is_structured_and_distinct_from_input_interpretation() {
    let present = llm_contract(LLM_PRESENT_KIND).unwrap();
    let interpret = llm_contract(LLM_INTERPRET_KIND).unwrap();
    assert_eq!(
        present.inputs[0].value_kind.as_str(),
        "conduit.face/generative-presenter-input@1"
    );
    assert_eq!(
        present.outputs[0].value_kind.as_str(),
        "conduit.face/generated-manifestation-candidate@2"
    );
    assert_eq!(
        present.kind_contract_revision.as_str(),
        "conduit.llm/present@3"
    );
    assert_ne!(present.kind_id, interpret.kind_id);
    assert_ne!(present.inputs, interpret.inputs);
    assert_ne!(present.outputs, interpret.outputs);
}

#[test]
#[cfg(feature = "plot-catalog")]
fn all_contracts_install_and_check_as_ordinary_provider_free_plots() {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    install_llm_semantic_catalog(&mut startup, &mut profile).unwrap();
    for contract in llm_semantic_catalog() {
        let name = contract.kind_id.as_str().replace('/', "-");
        let source = format!("plot {name} {{\n gear: {}\n}}\n", contract.kind_id.as_str());
        let syntax = conduit_plot::parse_syntax_document(&source);
        let checked = conduit_plot::check_syntax_document(&syntax, &startup).unwrap();
        let expanded = conduit_plot::expand_canonical_plot(&checked, &name, &profile).unwrap();
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
    result.accounting.output_bytes = contract.bounds.maximum_output_bytes() + 1;
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
fn exact_revision_and_front_compatibility_rejects_mutation() {
    let expected = llm_contract(LLM_EMBED_KIND).unwrap();
    let mut mutated = expected.clone();
    mutated.result_payload_kind = conduit_core::kind_id("llm/wrong-result@1");
    assert!(!expected.is_exactly_compatible_with(&mutated));
    assert!(expected.is_exactly_compatible_with(&expected));
}

#[test]
fn recoverable_generation_keeps_work_contract_and_exposes_exact_terminal_envelope() {
    let ordinary = llm_contract(LLM_GENERATE_FLOW_KIND).unwrap();
    let recoverable = llm_contract(LLM_GENERATE_RECOVERABLE_FLOW_KIND).unwrap();
    assert_eq!(
        recoverable.kind_contract_revision.as_str(),
        "conduit.llm/generate-recoverable-flow@1"
    );
    assert_eq!(recoverable.inputs, ordinary.inputs);
    assert_eq!(recoverable.bounds, ordinary.bounds);
    assert_eq!(recoverable.limits, ordinary.limits);
    assert_eq!(
        recoverable.result_payload_kind,
        ordinary.result_payload_kind
    );
    assert_eq!(recoverable.terminal_outcomes, ordinary.terminal_outcomes);
    assert_eq!(
        recoverable.outputs[0].value_kind,
        ordinary.outputs[0].value_kind
    );
    assert_eq!(
        recoverable.outputs[0].temporal,
        PortTemporal::Flow { closes: true }
    );
    assert_eq!(
        recoverable.outputs[0].abnormal_kind,
        Some(conduit_core::kind_id(conduit_core::TERMINAL_INFO_ID))
    );
    assert!(ordinary.outputs[0].abnormal_kind.is_none());
    assert!(!recoverable.is_exactly_compatible_with(&ordinary));
    let kind = recoverable.into_capability_contract();
    kind.validate().unwrap();
    let terminal = kind
        .value_contracts()
        .iter()
        .find(|value| {
            value.location
                == conduit_core::FrontValueLocation::OutputAbnormal(conduit_core::port_id("result"))
        })
        .unwrap();
    assert_eq!(
        terminal.contract.maximum_bytes,
        conduit_core::TERMINAL_INFO_ENCODED_LEN as u32
    );
    let loss = conduit_core::TerminalInfo::new(
        conduit_core::TerminalCategory::ProviderOrResourceLoss,
        [9; 32],
    );
    assert_eq!(
        loss.category(),
        conduit_core::TerminalCategory::ProviderOrResourceLoss
    );
    assert_eq!(
        conduit_ai::LocalModelKindProfile::GenerateRecoverableFlow.kind(),
        LLM_GENERATE_RECOVERABLE_FLOW_KIND
    );
}
