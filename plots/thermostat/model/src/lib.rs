#![no_std]
//! Portable thermostat meaning. Fixed Forms and allocation-free transitions;
//! requested settings never assert a sensor reading or physical actuator effect.
extern crate alloc;
mod back;
mod contract;
mod state;
pub use back::ThermostatBack;
pub use contract::*;
pub use state::*;
