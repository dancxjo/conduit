use super::*;
use conduit_core::kind_id;

#[test]
fn exact_abnormal_terminal_is_not_execution_failure_or_success() {
    let terminal = ValuePayload {
        value_kind: kind_id("test/transform-terminal"),
        encoded: vec![7],
    };
    assert_eq!(
        activation_terminal_state(
            4,
            Some(KernelCompositeTerminal::Abnormal),
            Some(terminal.clone()),
            false,
        ),
        BoundedActivationState::Abnormal {
            sequence: 4,
            terminal,
        }
    );
}

#[test]
fn absent_prepared_abnormal_buffer_is_a_malformed_execution_terminal() {
    assert_eq!(
        activation_terminal_state(5, Some(KernelCompositeTerminal::Abnormal), None, false),
        BoundedActivationState::Faulted {
            sequence: 5,
            fault: BoundedActivationFault::MissingOutput,
        }
    );
}

#[test]
fn input_abnormal_propagation_requires_exact_matching_promises() {
    let terminal = kind_id("test/input-terminal");
    assert_eq!(
        validate_propagated_input_terminal(Some(&terminal), Some(&terminal), &terminal),
        Ok(())
    );
    assert_eq!(
        validate_propagated_input_terminal(None, Some(&terminal), &terminal),
        Err(BoundedActivationError::InputTerminalKindMismatch {
            expected: None,
            actual: terminal.clone(),
        })
    );
    assert_eq!(
        validate_propagated_input_terminal(Some(&terminal), None, &terminal),
        Err(BoundedActivationError::OutputCannotPropagateInputTerminal {
            input: terminal,
            output: None,
        })
    );
}
