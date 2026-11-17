#![no_std]

//! Finite ambient display meaning and explicit human presentation.
//!
//! These projections allocate during preparation. They acquire no observation,
//! resource, clock, network or display authority and do not authenticate the
//! realization basis supplied by their owner. Civil labels are calendar-checked
//! and exposed as the original typed LocalDateTime; timezone resolution remains
//! the responsibility of the selected clock projection.

extern crate alloc;

mod resident;
pub use resident::{
    AMBIENT_RESIDENT_OBSERVE_KIND, AMBIENT_RESIDENT_OBSERVE_REVISION, AmbientUpdatePolicy,
    ambient_resident_aspect_name, ambient_resident_contributor_kind, ambient_resident_observe_kind,
};
mod aspect;
pub use aspect::{AmbientDisplayAspect, ambient_aspect_contributor_kind};
mod availability;
mod civil;
mod contributor;
mod face;
mod home;
mod observation_source;
pub use contributor::*;
pub use observation_source::*;
mod types;

pub use face::{AmbientFaceRefusal, PreparedAmbientFaces};
pub use types::{
    AMBIENT_DISPLAY_TYPES_SOURCE, install_ambient_display_types, prepare_ambient_display_types,
};

#[cfg(test)]
mod tests;

mod projection_memory;
pub use projection_memory::{
    ambient_fragment_backing_bytes, ambient_home_fragment_backing_bound,
    ambient_home_workspace_bytes,
};
