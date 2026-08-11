//! Kernel operation for the S4 toggle-demo browser sink.
//!
//! `ToggleShowOperation` awaits canonical Boolean values over the remote cord
//! and drives `presentation/bool` through the browser kernel.

use conduit_core::BOOL_ENCODED_LEN;
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId, Operation,
    OperationAction, OperationInput, PortId, RequestId,
};

/// Maximum Boolean values that the toggle sink will receive (must match `lib.rs`).
pub(super) const MAXIMUM_RECEIPTS: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CapacitySeal {
    pub(super) values: (usize, usize),
    pub(super) sign: usize,
    pub(super) identity: (usize, usize, usize),
    pub(super) projections: usize,
}

pub(super) struct ToggleShowOperation {
    pub(super) next: usize,
    pub(super) pending: Option<RequestId>,
}

impl ToggleShowOperation {
    fn fail(detail: u16) -> OperationAction {
        OperationAction::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            detail,
        })
    }
}

impl Operation for ToggleShowOperation {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.pending.is_none() => {
                let Ok(sequence) = u32::try_from(self.next) else {
                    return Self::fail(1);
                };
                let request = RequestId(0x8000_0000 | sequence);
                self.pending = Some(request);
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(0),
                    input: BoundedValueRef::new(value, BOOL_ENCODED_LEN as u32)
                        .expect("remote Boolean was admitted at its exact byte bound"),
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request)
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.output.is_none()
                    && outcome.failure.is_none() =>
            {
                self.pending = None;
                self.next += 1;
                OperationAction::Await
            }
            OperationInput::Closed { port: PortId(0) }
                if self.pending.is_none() && self.next == MAXIMUM_RECEIPTS =>
            {
                OperationAction::Complete
            }
            _ => Self::fail(2),
        }
    }
}
