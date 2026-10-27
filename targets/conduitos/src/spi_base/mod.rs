//! Peripheral-agnostic SPI mechanisms and explicitly admitted operation backs.

pub mod bcm2835;

pub mod contract;
pub mod transaction;
pub use transaction::{SpiDisposition, SpiProvider, SpiTransaction};
pub mod decode;
pub mod result;

#[cfg(test)]
mod tests;

mod factory;
pub mod installation;
pub mod owner;
pub use factory::SpiOperationFactory;
