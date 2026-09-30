use conduit_ai::{
    GeneratedTextFlowTerminal, LlmDeterminismProfile, LlmImplementationControl, LlmTerminalOutcome,
};
use conduit_form::rust_binding::NativeRustBinding;

fn assert_round_trip<T>(value: T)
where
    T: NativeRustBinding + Copy + core::fmt::Debug + PartialEq,
{
    let structured = value.into_structured().unwrap();
    assert_eq!(T::from_structured(structured).unwrap(), value);
}

#[test]
fn generated_text_terminal_round_trips_through_its_exact_native_type() {
    for terminal in [
        GeneratedTextFlowTerminal::Completed,
        GeneratedTextFlowTerminal::OutputBoundExhausted,
        GeneratedTextFlowTerminal::Backpressured,
        GeneratedTextFlowTerminal::Cancelled,
        GeneratedTextFlowTerminal::ProviderLost,
        GeneratedTextFlowTerminal::NonMonotonic,
    ] {
        assert_round_trip(terminal);
    }
}

#[test]
fn llm_contract_vocabularies_round_trip_through_their_native_types() {
    for value in [
        LlmDeterminismProfile::DeterministicValidationFixture,
        LlmDeterminismProfile::SeededImplementationBestEffort,
        LlmDeterminismProfile::StochasticInference,
        LlmDeterminismProfile::ProviderNondeterministic,
    ] {
        assert_round_trip(value);
    }
    for value in [
        LlmTerminalOutcome::Produced,
        LlmTerminalOutcome::Truncated,
        LlmTerminalOutcome::Refused,
        LlmTerminalOutcome::Failed,
        LlmTerminalOutcome::Cancelled,
        LlmTerminalOutcome::ProviderLost,
    ] {
        assert_round_trip(value);
    }
    for value in [
        LlmImplementationControl::Temperature,
        LlmImplementationControl::Seed,
        LlmImplementationControl::Sampler,
        LlmImplementationControl::Quantization,
        LlmImplementationControl::PromptTemplate,
        LlmImplementationControl::ChatRoleEncoding,
        LlmImplementationControl::ProviderFunctionJson,
    ] {
        assert_round_trip(value);
    }
}
