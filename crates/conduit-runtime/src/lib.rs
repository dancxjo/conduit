//! Current exact-plan lowering plus explicitly fenced compatibility execution.
//!
//! Production hosts depend on this crate with default features disabled and can
//! use only [`lowering`]. Legacy fixture and composite paths must deliberately
//! opt into `compatibility-executor`.

pub mod lowering;

#[cfg(feature = "compatibility-executor")]
mod shared_pool_validation;

#[cfg(feature = "compatibility-executor")]
mod compatibility_executor;
#[cfg(feature = "compatibility-executor")]
pub use compatibility_executor::*;
#[cfg(feature = "compatibility-executor")]
pub mod providers;
