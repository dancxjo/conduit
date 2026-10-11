//! Transport-neutral initial read-only block contracts; no storage driver.
pub mod contract;
pub mod owner;
pub mod request;
pub mod result;
#[cfg(test)]
mod tests;
