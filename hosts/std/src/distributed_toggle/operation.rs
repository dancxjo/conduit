//! Kernel operation types for the S4 toggle-demo source fragment.
//!
//! `ToggleSourceOperation` covers both `interaction/activate` (stdin await-activation
//! requests) and `state/toggle` (stateful bool flip with exact ValueRef validation).

use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    Operation, OperationAction, OperationInput, PortId, RequestId, ValueRef,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CapacitySeal {
    pub values: (usize, usize),
    pub evidence: usize,
    pub drivers: usize,
    pub identity: (usize, usize, usize),
}

/// Kernel operation covering both `interaction/activate` (await-activation host-op) and
/// `state/toggle` (stateful bool flip). Each node in the source scheduler gets its own
/// driver of this enum.
pub(super) enum ToggleSourceOperation {
    /// Activate: awaits one deliberate operator input per activation.
    /// Each host-operation request carries a 1-byte sequence token as a correlation handle.
    /// The std adapter performs the actual `read_line` inside the host-op completion,
    /// not before the kernel issues the request.
    Activate {
        /// Pre-stored 1-byte correlation tokens (one per activation).
        tokens: Vec<ValueRef>,
        /// Pre-stored activation payloads (emitted after each accepted host op).
        values: Vec<ValueRef>,
        next: usize,
        pending: Option<RequestId>,
    },
    /// Toggle: receives one activation value, validates its exact ValueRef identity,
    /// then emits one Signal. Rejects wrong, duplicate, skipped, or reordered activations.
    Toggle {
        /// Pre-stored Signal payloads (emitted one-for-one with accepted activations).
        signals: Vec<ValueRef>,
        /// Expected activation ValueRef references in order (same refs emitted by Activate).
        expected_activations: Vec<ValueRef>,
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
            Self::Activate { tokens, values, .. } => tokens.capacity() + values.capacity(),
            Self::Toggle {
                signals,
                expected_activations,
                ..
            } => signals.capacity() + expected_activations.capacity(),
        }
    }
}

impl Operation for ToggleSourceOperation {
    fn start(&mut self) -> OperationAction {
        match self {
            Self::Activate {
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
            Self::Activate { .. } => OperationAction::Complete,
            Self::Toggle { .. } => OperationAction::Await,
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match (self, input) {
            (
                Self::Activate {
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
                    expected_activations,
                    next,
                },
                OperationInput::Value {
                    port: PortId(0),
                    value,
                },
            ) => {
                // Validate the exact received activation against the pre-admitted expected ref.
                // This rejects wrong, duplicate, skipped, or reordered activations by identity.
                let expected = match expected_activations.get(*next).copied() {
                    Some(r) => r,
                    None => return Self::fail(16),
                };
                if value != expected {
                    // Wrong, duplicate, skipped, or reordered activation ValueRef.
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
            _ => Self::fail(13),
        }
    }

    fn advance(&mut self) -> OperationAction {
        match self {
            Self::Activate {
                tokens,
                next,
                pending,
                ..
            } => {
                *next += 1;
                if *next > tokens.len() {
                    return OperationAction::Complete;
                }
                let Some(token) = tokens.get(*next - 1).copied() else {
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

    /// Activate with zero tokens completes immediately.
    #[test]
    fn activate_with_no_tokens_completes() {
        let mut op = ToggleSourceOperation::Activate {
            tokens: vec![],
            values: vec![],
            next: 0,
            pending: None,
        };
        assert_eq!(op.start(), OperationAction::Complete);
    }

    /// Toggle with wrong ValueRef identity is rejected (fail detail 17).
    #[test]
    fn toggle_rejects_wrong_activation_ref() {
        let mut store = make_value_store();
        let correct = store_bytes(&mut store, &0u64.to_le_bytes());
        let wrong = store_bytes(&mut store, &1u64.to_le_bytes());
        let signal = store_bytes(&mut store, &[0u8; 9]);

        let mut op = ToggleSourceOperation::Toggle {
            signals: vec![signal],
            expected_activations: vec![correct],
            next: 0,
        };
        // Start returns Await
        assert_eq!(op.start(), OperationAction::Await);
        // Presenting the wrong ref fails with detail 17
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
    fn toggle_accepts_correct_activation_ref_and_emits() {
        let mut store = make_value_store();
        let activation = store_bytes(&mut store, &0u64.to_le_bytes());
        let signal = store_bytes(&mut store, &[0u8; 9]);

        let mut op = ToggleSourceOperation::Toggle {
            signals: vec![signal],
            expected_activations: vec![activation],
            next: 0,
        };
        let action = op.resume(OperationInput::Value {
            port: PortId(0),
            value: activation,
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
    fn toggle_rejects_skipped_activation() {
        let mut store = make_value_store();
        let first = store_bytes(&mut store, &0u64.to_le_bytes());
        let second = store_bytes(&mut store, &1u64.to_le_bytes());
        let signal = store_bytes(&mut store, &[0u8; 9]);

        let mut op = ToggleSourceOperation::Toggle {
            signals: vec![signal, signal],
            expected_activations: vec![first, second],
            next: 0,
        };
        // Present second before first
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
    fn toggle_rejects_duplicate_activation() {
        let mut store = make_value_store();
        let first = store_bytes(&mut store, &0u64.to_le_bytes());
        let second = store_bytes(&mut store, &1u64.to_le_bytes());
        let signal = store_bytes(&mut store, &[0u8; 9]);

        let mut op = ToggleSourceOperation::Toggle {
            signals: vec![signal, signal],
            expected_activations: vec![first, second],
            next: 0,
        };
        // Accept first
        let _ = op.resume(OperationInput::Value {
            port: PortId(0),
            value: first,
        });
        op.advance();
        // Present first again (duplicate)
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

    /// Activate host-operation completion with wrong request ID fails.
    #[test]
    fn activate_rejects_wrong_request_id() {
        let mut store = make_value_store();
        let token = store_bytes(&mut store, &[0u8]);
        let value = store_bytes(&mut store, &0u64.to_le_bytes());

        let mut op = ToggleSourceOperation::Activate {
            tokens: vec![token],
            values: vec![value],
            next: 0,
            pending: None,
        };
        op.start();
        // Complete with wrong request id (1 instead of 0)
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
}
