//! Finite Pulse operation used by the production R1 source kernel.

use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId, Operation,
    OperationAction, OperationInput, PortId, RequestId, ValueRef,
};

pub(super) const MAXIMUM_VALUES: usize = 16;
pub(super) const MAXIMUM_WAITS: usize = MAXIMUM_VALUES - 1;

pub(super) struct PulseOperation {
    values: Vec<ValueRef>,
    waits: Vec<ValueRef>,
    next: usize,
    pending: Option<RequestId>,
}

impl PulseOperation {
    pub(super) fn new(values: Vec<ValueRef>, waits: Vec<ValueRef>) -> Self {
        Self {
            values,
            waits,
            next: 0,
            pending: None,
        }
    }

    fn fail(detail: u16) -> OperationAction {
        OperationAction::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            detail,
        })
    }

    fn emit_current(&self) -> OperationAction {
        self.values
            .get(self.next)
            .copied()
            .map_or(OperationAction::Complete, |value| OperationAction::Emit {
                port: PortId(0),
                value,
            })
    }

    pub(super) fn allocation_capacity(&self) -> usize {
        self.values.capacity() + self.waits.capacity()
    }
}

impl Operation for PulseOperation {
    fn start(&mut self) -> OperationAction {
        self.emit_current()
    }

    fn advance(&mut self) -> OperationAction {
        self.next += 1;
        if self.next >= self.values.len() {
            return OperationAction::Complete;
        }
        let Some(wait) = self.waits.get(self.next - 1).copied() else {
            return Self::fail(1);
        };
        let request = RequestId(self.next as u32);
        self.pending = Some(request);
        OperationAction::RequestHostOperation {
            request,
            operation: HostOperationId(0),
            input: BoundedValueRef::new(wait, 8).expect("planned wait is exactly eight bytes"),
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request)
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.output.is_none()
                    && outcome.failure.is_none() =>
            {
                self.pending = None;
                self.emit_current()
            }
            _ => Self::fail(2),
        }
    }
}
