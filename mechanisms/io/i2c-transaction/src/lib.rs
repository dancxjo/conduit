//! Class-neutral finite I2C transactions and exact admitted Host Call custody.
//! Device register/protocol knowledge belongs in authored checked Plots.
#![no_std]
extern crate alloc;
pub mod contract;
pub mod decode;
#[cfg(feature = "operation-factory")]
mod factory;
pub mod installation;
pub mod owner;
pub mod result;
pub mod transaction;
#[cfg(feature = "operation-factory")]
pub use factory::I2cOperationFactory;
pub use transaction::{I2cDisposition, I2cProvider, I2cTransaction};
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;

#[cfg(test)]
#[macro_use]
extern crate std;
#[cfg(test)]
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[cfg(test)]
#[global_allocator]
static TEST_ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

#[cfg(test)]
extern crate self as conduit_i2c_transaction;
