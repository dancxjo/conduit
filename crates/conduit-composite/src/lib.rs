//! Kernel-backed execution for exact composite definitions.
//!
//! The production surface is deliberately independent of the retained
//! compatibility executor. Enable `compatibility-fixture` only to compare the
//! migration path with the former hosted fixture.

mod boundary;
mod child;
mod definition;
mod kernel_executor;
mod operation;

pub use definition::*;
pub use kernel_executor::*;
pub use operation::*;

#[cfg(feature = "compatibility-fixture")]
mod compatibility;
#[cfg(feature = "compatibility-fixture")]
pub use compatibility::*;
