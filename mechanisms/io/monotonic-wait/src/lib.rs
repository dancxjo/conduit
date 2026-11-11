#![no_std]
//! Owned native execution of the existing Tick wait Host Call. This installs
//! no clock, advertises no availability, issues no authority and starts no Body.
extern crate alloc;
#[cfg(test)]
#[macro_use]
extern crate std;
#[cfg(feature = "wait")]
mod installation;
#[cfg(feature = "wait")]
pub use installation::*;
#[cfg(feature = "wait")]
mod owner;
#[cfg(feature = "wait")]
pub use owner::*;

#[cfg(feature = "clock-at")]
pub mod clock_at;
