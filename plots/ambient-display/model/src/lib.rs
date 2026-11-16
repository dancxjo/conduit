#![no_std]

//! Finite ambient display meaning and explicit human presentation.
//!
//! These projections allocate during preparation. They acquire no observation,
//! resource, clock, network or display authority and do not authenticate the
//! realization basis supplied by their owner. Civil labels are calendar-checked
//! and exposed as the original typed LocalDateTime; timezone resolution remains
//! the responsibility of the selected clock projection.

extern crate alloc;

mod availability;
mod civil;
mod contributor;
mod face;
mod home;
pub use contributor::*;
mod types;

pub use face::{AmbientFaceRefusal, PreparedAmbientFaces};
pub use types::{
    install_ambient_display_types, prepare_ambient_display_types, AMBIENT_DISPLAY_TYPES_SOURCE,
};

#[cfg(test)]
mod tests;
