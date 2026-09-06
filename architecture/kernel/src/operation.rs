//! Shared operation lifecycle contract used by the one kernel scheduler.
use crate::{HostOperationOutcome, OperationAction, OperationInput, PortId, RequestId, ValueRef};

/// Shared state-machine boundary for hosted and fixed-storage execution.
pub trait Operation {
    /// Called only after the scheduler commits the step's admitted I/O.
    /// This must be infallible and allocation-free; preparation/step validates
    /// any bounds before proposing the transaction.
    fn step_committed(&mut self) {}
    fn start(&mut self) -> OperationAction;
    fn resume(&mut self, input: OperationInput) -> OperationAction;
    /// Whether this operation may consume another input while one exact host
    /// request remains pending. The default preserves backpressure for
    /// operations whose host interaction must complete before more input.
    fn accepts_input_while_host_operation_pending(&self) -> bool {
        false
    }
    /// Returns one exact pending host request that this operation wants the
    /// adapter to cancel. The scheduler validates ownership and dispatch state
    /// before exposing the cancellation to the host.
    fn take_host_operation_cancellation(&mut self) -> Option<RequestId> {
        None
    }
    /// Resumes one exact admitted value with its canonical bytes borrowed
    /// read-only for this call. The default preserves the opaque-value
    /// contract used by operations that only route or retain identity.
    fn resume_value(&mut self, port: PortId, value: ValueRef, canonical: &[u8]) -> OperationAction {
        let _ = canonical;
        self.resume(OperationInput::Value { port, value })
    }
    /// Resumes one exact host completion with the completed output's canonical
    /// bytes borrowed read-only for this call. The default preserves the
    /// opaque host-output contract.
    fn resume_host_operation(
        &mut self,
        request: RequestId,
        outcome: HostOperationOutcome,
        canonical: Option<&[u8]>,
    ) -> OperationAction {
        let _ = canonical;
        self.resume(OperationInput::HostOperationCompleted { request, outcome })
    }
    fn advance(&mut self) -> OperationAction {
        OperationAction::Await
    }
    fn retains_resumed_value(&self) -> bool {
        false
    }
    /// Returns scheduler-owned values released by the current transition.
    ///
    /// The operation driver calls this repeatedly until it returns `None`, up
    /// to the admitted node's port capacity. Returning more identities than
    /// that finite bound is a protocol violation.
    fn take_released_value(&mut self) -> Option<ValueRef> {
        None
    }
    fn cancel(&mut self) {}
}
