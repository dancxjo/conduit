#![no_std]
//! Finite typed retention beneath the ordinary kernel's commit boundary.
//!
//! This implementation declares no Kind, State initialization, resource grant,
//! durable recording, clock, transport acknowledgment or producer authority.
extern crate alloc;
mod order;
mod retention;
pub use order::CommitOrder;
pub use retention::{PreparedTypedRetention, RetainedSnapshot};
