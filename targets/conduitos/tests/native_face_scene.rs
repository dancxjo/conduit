#![cfg(feature = "native-compositor")]
extern crate alloc;
use conduitos::{display, front_door};
mod native_workset {
    pub use conduitos::native_workset::{
        NativePlot, WorksetRefusal, candidate_inventory, profile, resident,
    };
}
#[allow(
    dead_code,
    reason = "shared production scene also exposes navigation used by the compositor"
)]
#[path = "../src/native_face_scene.rs"]
mod native_face_scene;
