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
    ambient_resident_aspect_name, ambient_resident_contributor_kind, ambient_resident_observe_kind,
    AmbientUpdatePolicy, AMBIENT_RESIDENT_OBSERVE_KIND, AMBIENT_RESIDENT_OBSERVE_REVISION,
};
mod aspect;
pub use aspect::{ambient_aspect_contributor_kind, AmbientDisplayAspect};
mod availability;
mod civil;
mod contributor;
mod face;
mod subject_identity;
pub use subject_identity::{ambient_clock_subject_refs, AmbientClockSubjectRefs};
mod home;
mod observation_source;
pub use contributor::*;
pub use observation_source::*;
mod types;

pub use face::{AmbientFaceRefusal, PreparedAmbientFaces};
pub use types::{
    install_ambient_display_types, prepare_ambient_display_types, AMBIENT_DISPLAY_TYPES_SOURCE,
};

#[cfg(test)]
mod tests;

mod projection_memory;
pub use projection_memory::{
    ambient_fragment_backing_bytes, ambient_home_fragment_backing_bound,
    ambient_home_workspace_bytes,
};

#[cfg(test)]
#[path = "tests/fixtures.rs"]
mod test_fixtures;
