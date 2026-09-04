use super::*;
use conduit_kernel::{HostOperationOutcome, HostedValueStore, ValueStorage};

fn make_value_store() -> HostedValueStore {
    // 32 items × 9 bytes max = 288 bytes capacity
    HostedValueStore::new(32, 9, 288).expect("test store")
}

fn store_bytes(store: &mut HostedValueStore, bytes: &[u8]) -> ValueRef {
    store.store(bytes).expect("store bytes")
}

/// Trigger with zero tokens completes immediately.
#[test]
fn trigger_with_no_tokens_completes() {
    let mut op = ToggleSourceOperation::Trigger {
        tokens: vec![],
        values: vec![],
        next: 0,
        pending: None,
    };
    assert_eq!(op.start(), OperationAction::Complete);
}

fn toggle(values: Vec<ValueRef>, expected_triggers: Vec<ValueRef>) -> ToggleSourceOperation {
    ToggleSourceOperation::Toggle {
        values,
        expected_triggers,
        next: 0,
        initial_emitted: false,
    }
}

#[test]
fn canonical_toggle_emits_initial_then_flips_and_rejects_wrong_identity() {
    let mut store = make_value_store();
    let correct = store_bytes(&mut store, &0u64.to_le_bytes());
    let wrong = store_bytes(&mut store, &1u64.to_le_bytes());
    let false_value = store_bytes(&mut store, &[0]);
    let true_value = store_bytes(&mut store, &[1]);
    let mut op = toggle(vec![false_value, true_value], vec![correct]);

    assert_eq!(
        op.start(),
        OperationAction::Emit {
            port: PortId(0),
            value: false_value,
        }
    );
    assert_eq!(op.advance(), OperationAction::Await);
    assert_eq!(
        op.resume(OperationInput::Value {
            port: PortId(0),
            value: wrong,
        }),
        OperationAction::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            detail: 17,
        })
    );
    assert_eq!(
        op.resume(OperationInput::Value {
            port: PortId(0),
            value: correct,
        }),
        OperationAction::Emit {
            port: PortId(0),
            value: true_value,
        }
    );
    assert_eq!(op.advance(), OperationAction::Await);
    assert_eq!(
        op.resume(OperationInput::Closed { port: PortId(0) }),
        OperationAction::Complete
    );
}

/// Trigger host-operation completion with wrong request ID fails.
#[test]
fn trigger_rejects_wrong_request_id() {
    let mut store = make_value_store();
    let token = store_bytes(&mut store, &[0u8]);
    let value = store_bytes(&mut store, &0u64.to_le_bytes());

    let mut op = ToggleSourceOperation::Trigger {
        tokens: vec![token],
        values: vec![value],
        next: 0,
        pending: None,
    };
    op.start();
    let action = op.resume(OperationInput::HostOperationCompleted {
        request: RequestId(1),
        outcome: HostOperationOutcome {
            disposition: HostOperationDisposition::Completed,
            output: None,
            failure: None,
        },
    });
    assert_eq!(
        action,
        OperationAction::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            detail: 13,
        })
    );
}

/// All sixteen admitted request/completion/emission cycles reach `Complete` without error.
/// Also verifies each cycle uses a distinct token (not token[0] repeated).
#[test]
fn trigger_full_sixteen_cycles_reach_complete() {
    const N: usize = 16;
    let mut store =
        HostedValueStore::new((N * 2) as u16, 8, (N * 2 * 8) as u32).expect("test store");

    let mut tokens: Vec<ValueRef> = Vec::with_capacity(N);
    let mut values: Vec<ValueRef> = Vec::with_capacity(N);
    for seq in 0..N {
        tokens.push(store.store(&[seq as u8]).expect("store token"));
        values.push(
            store
                .store(&(seq as u64).to_le_bytes())
                .expect("store value"),
        );
    }

    let mut op = ToggleSourceOperation::Trigger {
        tokens: tokens.clone(),
        values: values.clone(),
        next: 0,
        pending: None,
    };

    let action = op.start();
    assert!(
        matches!(
            action,
            OperationAction::RequestHostOperation {
                request: RequestId(0),
                ..
            }
        ),
        "start should request with RequestId(0)"
    );

    for cycle in 0..N {
        let complete_action = op.resume(OperationInput::HostOperationCompleted {
            request: RequestId(cycle as u32),
            outcome: HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: None,
                failure: None,
            },
        });
        assert_eq!(
            complete_action,
            OperationAction::Emit {
                port: PortId(0),
                value: values[cycle],
            },
            "cycle {cycle}: resume should emit values[{cycle}]"
        );

        let advance_action = op.advance();
        if cycle + 1 == N {
            assert_eq!(
                advance_action,
                OperationAction::Complete,
                "cycle {cycle}: final advance should Complete"
            );
        } else {
            match advance_action {
                OperationAction::RequestHostOperation { request, input, .. } => {
                    assert_eq!(
                        request,
                        RequestId((cycle + 1) as u32),
                        "cycle {cycle}: advance should request RequestId({})",
                        cycle + 1
                    );
                    assert_eq!(
                        input.value,
                        tokens[cycle + 1],
                        "cycle {cycle}: advance should use token[{}]",
                        cycle + 1
                    );
                }
                other => {
                    panic!("cycle {cycle}: advance should RequestHostOperation, got {other:?}")
                }
            }
        }
    }
}

/// Toggle operation completes when it receives Closed after consuming all expected triggers.
#[test]
fn toggle_completes_on_closed_after_all_triggers() {
    let mut store = make_value_store();
    let act = store_bytes(&mut store, &[0x01]);
    let false_value = store_bytes(&mut store, &[0]);
    let true_value = store_bytes(&mut store, &[1]);
    let mut op = toggle(vec![false_value, true_value], vec![act]);

    assert!(matches!(op.start(), OperationAction::Emit { .. }));
    assert_eq!(op.advance(), OperationAction::Await);

    // Receive the expected trigger value.
    let action = op.resume(OperationInput::Value {
        port: PortId(0),
        value: act,
    });
    assert_eq!(
        action,
        OperationAction::Emit {
            port: PortId(0),
            value: true_value,
        },
    );
    let advance = op.advance();
    assert_eq!(advance, OperationAction::Await);

    // Now the Trigger producer closes the cord. Toggle should complete.
    let closed = op.resume(OperationInput::Closed { port: PortId(0) });
    assert_eq!(closed, OperationAction::Complete);
}

/// Toggle operation fails (13) if Closed arrives before all triggers are consumed.
#[test]
fn toggle_rejects_early_closed() {
    let mut store = make_value_store();
    let act1 = store_bytes(&mut store, &[0x01]);
    let act2 = store_bytes(&mut store, &[0x02]);
    let false_value = store_bytes(&mut store, &[0]);
    let true_value = store_bytes(&mut store, &[1]);
    let another_false = store_bytes(&mut store, &[0]);
    let mut op = toggle(
        vec![false_value, true_value, another_false],
        vec![act1, act2],
    );

    assert!(matches!(op.start(), OperationAction::Emit { .. }));
    assert_eq!(op.advance(), OperationAction::Await);

    // Only receive one of two expected triggers.
    let action = op.resume(OperationInput::Value {
        port: PortId(0),
        value: act1,
    });
    assert!(matches!(action, OperationAction::Emit { .. }));
    op.advance();

    // Closed arrives before second trigger — should fail.
    let closed = op.resume(OperationInput::Closed { port: PortId(0) });
    assert_eq!(
        closed,
        OperationAction::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            detail: 13,
        }),
    );
}
