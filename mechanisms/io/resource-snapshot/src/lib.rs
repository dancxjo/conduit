#![no_std]
//! Shared immutable snapshot records; no storage executor or authority issuer.
extern crate alloc;
#[cfg(test)]
#[macro_use]
extern crate std;
pub mod append;
pub mod nor;
mod record;
pub use record::*;
#[cfg(test)]
mod tests;

#[cfg(feature = "kernel-step")]
mod publication;
#[cfg(feature = "kernel-step")]
pub use publication::{SnapshotCommitObserver, SnapshotContentValidator, SnapshotPublicationBack};
