//! Checked ambient observation contracts and bounded Face projection.
#![no_std]
extern crate alloc;
mod types;
pub use types::{
    install_ambient_display_types, prepare_ambient_display_types, AMBIENT_DISPLAY_TYPES_SOURCE,
};

mod availability;
mod civil;
mod face;
mod home;
mod projection_memory;
mod subject_identity;

pub const AMBIENT_HOME_MAXIMUM_BYTES: u32 = 8192;
pub use face::{AmbientFaceRefusal, PreparedAmbientFaces};
pub use projection_memory::{
    ambient_fragment_backing_bytes, ambient_home_fragment_backing_bound,
    ambient_home_workspace_bytes,
};
pub use subject_identity::{ambient_clock_subject_refs, AmbientClockSubjectRefs};

#[cfg(test)]
mod tests;
