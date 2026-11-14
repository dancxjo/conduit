#![no_std]
//! Owned ESP32 electrical implementations of class-neutral Host Calls.
//!
//! Constructing these providers does not admit a Plot, grant equipment custody,
//! decode a sensor protocol, or select a Face. The composition root must bind
//! them through the ordinary selected-operation owners before Play.
mod pulse;
mod tm1637;

pub use pulse::{
    last_capture_observation, CaptureObservation, CapturePhase, Esp32PulseCapture, PULSE_LIMITS,
};
pub use tm1637::Esp32Tm1637;

pub mod platform;
#[cfg(feature = "host-carrier")]
pub mod uart;
