#![no_std]
//! Shared immutable snapshot records; no storage executor or authority issuer.
extern crate alloc;
mod record;
pub use record::*;
#[cfg(test)]
mod tests;

#[cfg(feature = "host-call")]
mod operation;
#[cfg(feature = "host-call")]
mod reference;
#[cfg(feature = "host-call")]
pub use operation::{PreparedSnapshotOperation, SnapshotStorageRequest};
#[cfg(feature = "host-call")]
pub use reference::snapshot_reference;
#[cfg(test)]
extern crate std;
