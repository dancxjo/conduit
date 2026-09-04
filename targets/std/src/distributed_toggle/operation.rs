//! Kernel operation types for the distributed toggle source fragment.
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
    /// Canonical toggle: emits its initial Boolean, then validates and flips for each Tick.
    Toggle {
        /// One exact pre-stored Boolean payload per admitted emission.
        values: Vec<ValueRef>,
        /// Expected trigger ValueRef references in order (same refs emitted by Trigger).
        expected_triggers: Vec<ValueRef>,
        next: usize,
        initial_emitted: bool,
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
                values,
                expected_triggers,
                ..
            } => values.capacity() + expected_triggers.capacity(),
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
            Self::Toggle { values, .. } => OperationAction::Emit {
                port: PortId(0),
                value: values[0],
            },
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
                    values,
                    expected_triggers,
                    next,
                    initial_emitted,
                },
                OperationInput::Value {
                    port: PortId(0),
                    value,
                },
            ) if *initial_emitted => {
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
                values.get(*next + 1).copied().map_or_else(
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
                    initial_emitted,
                    ..
                },
                OperationInput::Closed { port: PortId(0) },
            ) if *initial_emitted && *next == expected_triggers.len() => {
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
            Self::Toggle {
                next,
                initial_emitted,
                ..
            } => {
                if *initial_emitted {
                    *next += 1;
                } else {
                    *initial_emitted = true;
                }
                OperationAction::Await
            }
        }
    }
}

#[cfg(test)]
mod tests;
