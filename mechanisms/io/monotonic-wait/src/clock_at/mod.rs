//! Finite monotonic deadline execution through exact admitted possession.
pub mod codec;
pub mod contract;
pub mod owner;
pub mod polling;

#[cfg(all(test, feature = "clock-at-catalog"))]
mod schema_tests;

pub mod assigned;
pub mod retained;
