#![no_std]
//! Shared immutable snapshot records; no storage executor or authority issuer.
extern crate alloc;
#[cfg(test)]
#[macro_use]
extern crate std;
pub mod append;
pub mod append_contract;
pub mod append_result;
pub mod nor;
pub mod read_contract;
pub mod read_result;
mod record;
pub mod service_contract;
pub use record::*;
#[cfg(feature = "native-host-call")]
pub mod native;
#[cfg(test)]
mod tests;

#[cfg(feature = "native-host-call")]
pub mod native_append;

#[cfg(feature = "native-host-call")]
pub mod native_service;

#[cfg(all(test, feature = "native-host-call"))]
#[path = "../../../../targets/conduitos/src/machine_membrane/selection_fixture.rs"]
mod selection_fixture;
