use super::{DebuggerExecutionControl, DebuggerExecutionControlState, DebuggerExecutionIdentity};

fn execution(byte: u8) -> DebuggerExecutionIdentity {
    DebuggerExecutionIdentity {
        body: [byte; 32],
        plan: [byte.wrapping_add(1); 32],
        play: [byte.wrapping_add(2); 32],
    }
}

#[test]
fn replacement_execution_makes_exact_breakpoint_state_stale_without_label_remap() {
    let mut control = DebuggerExecutionControl::new(execution(1), vec!["gear/friendly".into()]);
    control.suspended("gear/friendly");
    control.replace_execution(execution(4));
    assert_eq!(control.state, DebuggerExecutionControlState::Stale);
    assert!(control.breakpoint_subject.is_none());
    assert!(control.suspended_subject.is_none());
    assert!(control
        .reason
        .as_deref()
        .unwrap()
        .contains("was not remapped"));
    assert_eq!(control.execution, execution(1));
}
