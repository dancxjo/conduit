//! Kernel operation types for the S4 toggle-demo source fragment.
//!
//! `ToggleSourceOperation` covers both `interaction/trigger` (stdin await-trigger
//! requests) and `state/toggle` (stateful bool flip with exact ValueRef validation).

use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId, Operation,
    OperationAction, OperationInput, PortId, RequestId, ValueRef,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CapacitySeal {
    pub values: (usize, usize),
    pub sign: usize,
    pub drivers: usize,
    pub identity: (usize, usize, usize),
}

/// Kernel operation covering both `interaction/trigger` (await-trigger host-op) and
/// `state/toggle` (stateful bool flip). Each node in the source scheduler gets its own
/// driver of this enum.
pub(super) enum ToggleSourceOperation {
    /// Trigger: awaits one deliberate operator input per trigger.
    /// Each host-operation request carries a 1-byte sequence token as a correlation handle.
    /// The std adapter performs the actual `read_line` inside the host-op completion,
    /// not before the kernel issues the request.
    Trigger {
        /// Pre-stored 1-byte correlation tokens (one per trigger).
        tokens: Vec<ValueRef>,
        /// Pre-stored trigger payloads (emitted after each accepted host op).
        values: Vec<ValueRef>,
        next: usize,
        pending: Option<RequestId>,
    },
    /// Toggle: receives one trigger value, validates its exact ValueRef identity,
    /// then emits one Signal. Rejects wrong, duplicate, skipped, or reordered triggers.
    Toggle {
        /// Pre-stored Signal payloads (emitted one-for-one with accepted triggers).
        signals: Vec<ValueRef>,
        /// Expected trigger ValueRef references in order (same refs emitted by Trigger).
        expected_triggers: Vec<ValueRef>,
        next: usize,
    },
}

impl ToggleSourceOperation {
    pub(super) fn fail(detail: u16) -> OperationAction {
        OperationAction::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            detail,
        })
    }

    pub(super) fn allocation_capacity(&self) -> usize {
        match self {
            Self::Trigger { tokens, values, .. } => tokens.capacity() + values.capacity(),
            Self::Toggle {
                signals,
                expected_triggers,
                ..
            } => signals.capacity() + expected_triggers.capacity(),
        }
    }
}

impl Operation for ToggleSourceOperation {
    fn start(&mut self) -> OperationAction {
        match self {
            Self::Trigger {
                tokens,
                next,
                pending,
                ..
            } if !tokens.is_empty() => {
                let Some(token) = tokens.first().copied() else {
                    return Self::fail(10);
                };
                let request = RequestId(0);
                *pending = Some(request);
                *next = 0;
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(0),
                    input: BoundedValueRef::new(token, 1)
                        .expect("sealed token value is exactly admitted"),
                }
            }
            Self::Trigger { .. } => OperationAction::Complete,
            Self::Toggle { .. } => OperationAction::Await,
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match (self, input) {
            (
                Self::Trigger {
                    values,
                    next,
                    pending,
                    ..
                },
                OperationInput::HostOperationCompleted { request, outcome },
            ) if *pending == Some(request)
                && outcome.disposition == HostOperationDisposition::Completed
                && outcome.output.is_none()
                && outcome.failure.is_none() =>
            {
                *pending = None;
                values.get(*next).copied().map_or_else(
                    || Self::fail(11),
                    |value| OperationAction::Emit {
                        port: PortId(0),
                        value,
                    },
                )
            }
            (
                Self::Toggle {
                    signals,
                    expected_triggers,
                    next,
                },
                OperationInput::Value {
                    port: PortId(0),
                    value,
                },
            ) => {
                // Validate the exact received trigger against the pre-admitted expected ref.
                // This rejects wrong, duplicate, skipped, or reordered triggers by identity.
                let expected = match expected_triggers.get(*next).copied() {
                    Some(r) => r,
                    None => return Self::fail(16),
                };
                if value != expected {
                    // Wrong, duplicate, skipped, or reordered trigger ValueRef.
                    return Self::fail(17);
                }
                signals.get(*next).copied().map_or_else(
                    || Self::fail(12),
                    |value| OperationAction::Emit {
                        port: PortId(0),
                        value,
                    },
                )
            }
            (
                Self::Toggle {
                    next,
                    expected_triggers,
                    ..
                },
                OperationInput::Closed { port: PortId(0) },
            ) if *next == expected_triggers.len() => {
                // The Trigger operation completed and the local cord closed
                // after delivering all expected triggers.
                OperationAction::Complete
            }
            _ => Self::fail(13),
        }
    }

    fn advance(&mut self) -> OperationAction {
        match self {
            Self::Trigger {
                tokens,
                next,
                pending,
                ..
            } => {
                *next += 1;
                if *next >= tokens.len() {
                    // All admitted triggers have been emitted.
                    return OperationAction::Complete;
                }
                // Use token[next] (the token for the upcoming request).
                let Some(token) = tokens.get(*next).copied() else {
                    return Self::fail(14);
                };
                let Ok(sequence) = u32::try_from(*next) else {
                    return Self::fail(15);
                };
                let request = RequestId(sequence);
                *pending = Some(request);
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(0),
                    input: BoundedValueRef::new(token, 1)
                        .expect("sealed token value is exactly admitted"),
                }
            }
            Self::Toggle { next, .. } => {
                *next += 1;
                OperationAction::Await
            }
        }
    }
}

#[cfg(test)]
mod tests {
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

    /// Toggle with wrong ValueRef identity is rejected (fail detail 17).
    #[test]
    fn toggle_rejects_wrong_trigger_ref() {
        let mut store = make_value_store();
        let correct = store_bytes(&mut store, &0u64.to_le_bytes());
        let wrong = store_bytes(&mut store, &1u64.to_le_bytes());
        let signal = store_bytes(&mut store, &[0u8; 9]);

        let mut op = ToggleSourceOperation::Toggle {
            signals: vec![signal],
            expected_triggers: vec![correct],
            next: 0,
        };
        assert_eq!(op.start(), OperationAction::Await);
        let action = op.resume(OperationInput::Value {
            port: PortId(0),
            value: wrong,
        });
        assert_eq!(
            action,
            OperationAction::Fail(Failure {
                code: FailureCode::InvalidLifecycle,
                detail: 17,
            })
        );
    }

    /// Toggle with correct ValueRef emits the pre-stored signal.
    #[test]
    fn toggle_accepts_correct_trigger_ref_and_emits() {
        let mut store = make_value_store();
        let trigger = store_bytes(&mut store, &0u64.to_le_bytes());
        let signal = store_bytes(&mut store, &[0u8; 9]);

        let mut op = ToggleSourceOperation::Toggle {
            signals: vec![signal],
            expected_triggers: vec![trigger],
            next: 0,
        };
        let action = op.resume(OperationInput::Value {
            port: PortId(0),
            value: trigger,
        });
        assert_eq!(
            action,
            OperationAction::Emit {
                port: PortId(0),
                value: signal,
            }
        );
    }

    /// Toggle with out-of-order ref (skipped) is rejected (fail detail 17).
    #[test]
    fn toggle_rejects_skipped_trigger() {
        let mut store = make_value_store();
        let first = store_bytes(&mut store, &0u64.to_le_bytes());
        let second = store_bytes(&mut store, &1u64.to_le_bytes());
        let signal = store_bytes(&mut store, &[0u8; 9]);

        let mut op = ToggleSourceOperation::Toggle {
            signals: vec![signal, signal],
            expected_triggers: vec![first, second],
            next: 0,
        };
        let action = op.resume(OperationInput::Value {
            port: PortId(0),
            value: second,
        });
        assert_eq!(
            action,
            OperationAction::Fail(Failure {
                code: FailureCode::InvalidLifecycle,
                detail: 17,
            })
        );
    }

    /// Toggle with duplicate (same ref twice) is rejected after advance.
    #[test]
    fn toggle_rejects_duplicate_trigger() {
        let mut store = make_value_store();
        let first = store_bytes(&mut store, &0u64.to_le_bytes());
        let second = store_bytes(&mut store, &1u64.to_le_bytes());
        let signal = store_bytes(&mut store, &[0u8; 9]);

        let mut op = ToggleSourceOperation::Toggle {
            signals: vec![signal, signal],
            expected_triggers: vec![first, second],
            next: 0,
        };
        let _ = op.resume(OperationInput::Value {
            port: PortId(0),
            value: first,
        });
        op.advance();
        let action = op.resume(OperationInput::Value {
            port: PortId(0),
            value: first,
        });
        assert_eq!(
            action,
            OperationAction::Fail(Failure {
                code: FailureCode::InvalidLifecycle,
                detail: 17,
            })
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
        let sig = store_bytes(&mut store, &[0x02]);

        let mut op = ToggleSourceOperation::Toggle {
            signals: vec![sig],
            expected_triggers: vec![act],
            next: 0,
        };

        // Start: Toggle awaits input.
        assert_eq!(op.start(), OperationAction::Await);

        // Receive the expected trigger value.
        let action = op.resume(OperationInput::Value {
            port: PortId(0),
            value: act,
        });
        assert_eq!(
            action,
            OperationAction::Emit {
                port: PortId(0),
                value: sig,
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
        let sig1 = store_bytes(&mut store, &[0x11]);
        let sig2 = store_bytes(&mut store, &[0x12]);

        let mut op = ToggleSourceOperation::Toggle {
            signals: vec![sig1, sig2],
            expected_triggers: vec![act1, act2],
            next: 0,
        };

        assert_eq!(op.start(), OperationAction::Await);

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
}
