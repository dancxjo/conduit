//! Compatibility path for shared checked Face controls.
#[cfg(test)]
pub(crate) use patchbay_graph::project_face_controls as project_controls;
pub use patchbay_graph::{FaceControl, FaceControlKind, FaceInteraction, MAX_FACE_CONTROLS};
