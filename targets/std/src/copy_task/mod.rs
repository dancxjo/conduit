pub(crate) mod base;
mod driver;
mod executor;
mod model;
mod operation;
mod planning;
mod registry;
mod scheduler;

pub use model::{CopyRequestId, CopyResult, CopyRunReceipt, CopyStopToken};
#[cfg(all(target_os = "linux", feature = "isolated-file-base"))]
pub use planning::prepare_isolated_copy_task;
pub use planning::{prepare_copy_task, PreparedCopyTask};
pub use registry::{ProtectedFileAvailability, ProtectedFileRegistry};

#[cfg(test)]
mod tests;
