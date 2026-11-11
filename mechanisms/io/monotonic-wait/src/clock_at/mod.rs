//! Finite monotonic deadline execution through exact admitted possession.
pub mod codec;
pub mod contract;
pub mod owner;

#[cfg(all(test, feature = "clock-at-catalog"))]
mod schema_tests;

pub mod assigned;
pub mod retained;
