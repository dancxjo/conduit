#![no_std]
//! Shared immutable snapshot records; no storage executor or authority issuer.
extern crate alloc;
pub mod nor;
mod record;
pub mod nor;
pub use record::*;
#[cfg(test)]
mod tests;
