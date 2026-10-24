#![no_std]
//! Shared immutable snapshot records; no storage executor or authority issuer.
extern crate alloc;
mod record;
pub use record::*;
#[cfg(test)]
mod tests;
