//! Host-neutral bounded Patchbay graph facts and checked-Form projection.
#![no_std]

#[macro_use]
extern crate alloc;

mod prelude {
    pub use alloc::{borrow::ToOwned, string::String, vec::Vec};
}
mod construction;
mod front_controls;
mod graph;
mod inspection;
mod recursive_graph;
mod recursive_projection;
mod types;
pub use recursive_projection::{
    project_recursive_form_gear, RecursiveFormGearProjection, RecursiveFormProjectionError,
};

pub use front_controls::{
    project_controls as project_front_controls, FaceControl, FaceControlKind, FaceInteraction,
    MAX_FACE_CONTROLS,
};
pub use types::*;
