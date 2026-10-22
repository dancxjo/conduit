#![cfg(feature = "native-compositor")]
extern crate alloc;
use conduitos::{display, front_door};
mod native_workset {
    pub use conduitos::native_workset::{
        NativePlot, WorksetRefusal, candidate_inventory, profile, resident,
    };
}
#[path = "../src/native_face_scene.rs"]
mod native_face_scene;
