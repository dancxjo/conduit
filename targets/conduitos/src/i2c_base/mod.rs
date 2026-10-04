//! Low-level I2C realization. Device protocol knowledge belongs in checked plots.

pub mod contract;
pub mod decode;
mod factory;
pub mod i801;
pub use factory::I2cOperationFactory;
pub mod installation;
pub mod owner;
pub mod result;
pub mod transaction;
pub use transaction::{I2cDisposition, I2cProvider, I2cTransaction};

#[cfg(test)]
mod tests;
