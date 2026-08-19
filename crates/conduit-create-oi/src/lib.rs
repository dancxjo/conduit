//! Allocator-free iRobot Create Open Interface mechanism and local safety.
//!
//! This crate owns device/protocol truth below portable robotics meaning. It
//! deliberately has no Host, Plan, Form, operating-system, board, or Conduit
//! LINE knowledge, so std and constrained embedded providers use one codec and
//! one non-bypassable drive supervisor.

#![no_std]

mod device;
mod drive;
mod safety;

pub use device::*;
pub use drive::*;
pub use safety::*;

#[cfg(test)]
extern crate std;
