//! Bounded brownfield adaptation of one exact Tongues starter.

#![cfg(feature = "speech")]

mod contract;
mod execution;
mod pcm;
mod planning;
mod realization;
mod signs;
mod specimen;

pub use contract::*;
pub use execution::*;
pub use planning::*;
pub use realization::*;
pub use signs::*;
pub use specimen::*;
