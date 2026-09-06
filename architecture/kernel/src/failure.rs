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

impl FailureCode {
    /// Stable machine-readable spelling for failure reports across Host boundaries.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::InvalidPort => "invalid_port",
            Self::InvalidLifecycle => "invalid_lifecycle",
            Self::StorageExhausted => "storage_exhausted",
            Self::StateCapacityExhausted => "state_capacity_exhausted",
            Self::WorkBudgetExhausted => "work_budget_exhausted",
            Self::IdentityCapacityExhausted => "identity_capacity_exhausted",
            Self::HostOperationDenied => "host_operation_denied",
            Self::HostOperationFailed => "host_operation_failed",
            Self::Cancelled => "cancelled",
        }
    }
}
