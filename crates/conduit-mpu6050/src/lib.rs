//! Allocator-free MPU-6050 device protocol and deterministic body-frame derivation.
//!
//! This crate owns only mechanism truth below portable robotics meaning. Host,
//! Plan, authority, attachment, and Conduit LINE identities belong above it.

#![no_std]

mod derive;
mod device;

pub use derive::*;
pub use device::*;

#[cfg(test)]
extern crate std;
