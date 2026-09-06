//! Machine-readable operation failures, distinct from semantic completion.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureCode {
    InvalidInput,
    InvalidPort,
    InvalidLifecycle,
    /// Non-State storage needed by the operation cannot admit the value.
    StorageExhausted,
    /// A retained value does not fit the admitted State cell.
    StateCapacityExhausted,
    /// An explicit admitted transition/work allowance has been consumed.
    WorkBudgetExhausted,
    /// The next exact generation cannot be represented without wrapping.
    IdentityCapacityExhausted,
    HostOperationDenied,
    HostOperationFailed,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Failure {
    pub code: FailureCode,
    pub detail: u16,
}
