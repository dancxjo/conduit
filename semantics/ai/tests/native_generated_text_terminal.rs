use conduit_ai::GeneratedTextFlowTerminal;
use conduit_form::rust_binding::NativeRustBinding;

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
        let structured = terminal.into_structured().unwrap();
        assert_eq!(
            GeneratedTextFlowTerminal::from_structured(structured).unwrap(),
            terminal
        );
    }
}
