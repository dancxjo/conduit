//! Preparation diagnostics and finite runtime refusals.
use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KernelCompositeError {
    Empty,
    DuplicateChild(HostId),
    Lowering {
        child: HostId,
        error: LoweringError,
    },
    InvalidBoundary(String),
    InvalidHostCallToken,
    HostCallDispatchMismatch,
    HostCallOutputExceeded,
    StaleHostCallChild,
    ChildRefused {
        child: HostId,
        reason: String,
    },
    /// `child` indexes the immutable prepared child identities.
    /// Resolve it with `KernelCompositeHost::child_identity` outside or during Play.
    Execution {
        child: usize,
        reason: ChildExecutionError,
    },
    HostCallCompletionCleanup {
        child: usize,
        completion: ChildExecutionError,
        cleanup: conduit_kernel::scheduler::HostValueDiscardRefusal,
    },
    UnknownFront,
    StaleChild(HostId),
    StaleRuntimeChild {
        child: usize,
    },
    MissingHostCallObligation,
    InvalidLifecycle,
    CancellationRefused {
        failed_children: usize,
    },
    InternalTransport {
        link: usize,
        reason: ChildTransportError,
    },
    Terminal(ChildTerminalError),
}

impl core::fmt::Display for KernelCompositeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl core::error::Error for KernelCompositeError {}
