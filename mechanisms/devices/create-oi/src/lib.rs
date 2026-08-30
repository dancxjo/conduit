//! Allocator-free iRobot Create Open Interface mechanism and local safety.
//!
//! This crate owns device/protocol truth below portable robotics meaning. It
//! deliberately has no Host, Plan, Form, operating-system, board, or Conduit
//! LINE knowledge, so std and constrained embedded providers use one codec and
//! one non-bypassable drive supervisor.

#![no_std]

mod battery;
mod contact_withdrawal;
mod device;
mod drive;
mod mode;
mod observation;
mod power;
mod presentation;
mod safety;
mod safety_latch;
mod stream;

pub use battery::*;
pub use contact_withdrawal::*;
pub use device::*;
pub use drive::*;
pub use mode::*;
pub use observation::*;
pub use power::*;
pub use presentation::*;
pub use safety::*;
pub use safety_latch::*;
pub use stream::*;

#[cfg(test)]
extern crate std;
