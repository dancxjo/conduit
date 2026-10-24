//! Browser snapshot operation over the shared immutable Resource record codec.
pub use conduit_resource_snapshot::*;
mod operation;
pub use operation::{PreparedSnapshotOperation, SnapshotStorageRequest};
#[cfg(test)]
#[path = "../../../../mechanisms/io/resource-snapshot/src/tests.rs"]
pub(crate) mod tests;
