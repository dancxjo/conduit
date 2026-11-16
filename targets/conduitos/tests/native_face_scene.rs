#![cfg(feature = "native-compositor")]
extern crate alloc;
#[path = "../../../architecture/plot/tests/prepared_structured_payload/allocation.rs"]
mod test_allocations;
use conduitos::{display, front_door};
mod native_workset {
    pub use conduitos::native_workset::{
        NativePlot, WorksetRefusal, candidate_inventory, profile, resident,
    };
}
#[path = "../src/native_face_scene.rs"]
mod native_face_scene;
