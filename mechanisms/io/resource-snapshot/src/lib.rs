#![no_std]
//! Shared immutable snapshot records; no storage executor or authority issuer.
extern crate alloc;
pub mod nor;
mod record;
pub use record::*;
#[cfg(feature = "native-host-call")]
pub mod native;
#[cfg(test)]
mod tests;
