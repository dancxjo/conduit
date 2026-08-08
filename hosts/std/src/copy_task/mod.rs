mod executor;
mod model;
mod operation;
mod provider;
mod registry;

pub use model::{CopyRequestId, CopyResult, CopyRunReceipt, CopyStopToken};
pub use registry::{ProtectedFileAvailability, ProtectedFileRegistry};

#[cfg(test)]
mod tests;
