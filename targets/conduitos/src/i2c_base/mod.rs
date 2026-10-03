//! Low-level I2C realization. Device protocol knowledge belongs in checked plots.

pub mod contract;
pub mod decode;
pub mod i801;
pub mod installation;
pub mod owner;
pub mod result;
pub mod transaction;
pub use transaction::{I2cDisposition, I2cProvider, I2cTransaction};

#[cfg(test)]
mod tests;
