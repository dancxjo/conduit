#![cfg(feature = "native-compositor")]
extern crate alloc;
use conduitos::display;
mod native_workset {
    pub use conduitos::native_workset::{profile, resident};
}
#[path = "../src/native_face_scene.rs"]
mod native_face_scene;
