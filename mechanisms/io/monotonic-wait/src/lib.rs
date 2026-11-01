#![no_std]
//! Owned native execution of the existing Tick wait Host Call. This installs
//! no clock, advertises no availability, issues no authority and starts no Body.
extern crate alloc;
mod installation;
pub use installation::*;
mod owner;
pub use owner::*;

/// Affine cooperative arbitration during an actual pending monotonic wait.
/// No clock, peripheral ownership or physical timing guarantee is installed.
pub mod flash_window;
