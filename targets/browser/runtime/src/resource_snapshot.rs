//! Browser public path for the shared immutable Resource record codec.
pub use conduit_resource_snapshot::*;
#[cfg(test)]
#[path = "../../../../mechanisms/io/resource-snapshot/src/tests.rs"]
pub(crate) mod tests;
