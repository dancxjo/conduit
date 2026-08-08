mod executor;
mod model;
mod operation;
mod planning;
mod provider;
mod registry;

pub use model::{CopyRequestId, CopyResult, CopyRunReceipt, CopyStopToken};
pub use planning::{prepare_copy_task, PreparedCopyTask};
pub use registry::{ProtectedFileAvailability, ProtectedFileRegistry};

#[cfg(test)]
mod tests;
