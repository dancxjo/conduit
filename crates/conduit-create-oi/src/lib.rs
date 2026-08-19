//! Allocator-free iRobot Create Open Interface mechanism and local safety.
//!
//! This crate owns device/protocol truth below portable robotics meaning. It
//! deliberately has no Host, Plan, Form, operating-system, board, or Conduit
//! LINE knowledge, so std and constrained embedded providers use one codec and
//! one non-bypassable drive supervisor.

#![no_std]

mod device;
mod drive;
mod mode;
mod power;
mod safety;
mod safety_latch;

pub use device::*;
pub use drive::*;
pub use mode::*;
pub use power::*;
pub use safety::*;
pub use safety_latch::*;

#[cfg(test)]
extern crate std;
