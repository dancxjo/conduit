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

#[test]
fn interactive_scene_bound_covers_its_retained_lifetime() {
    let bounds = native_face_scene::StandardNativeSceneBounds {
        preparation_peak_bytes: 7,
        retained_bytes: 3,
        input_transient_bytes: 8,
        frame_transient_bytes: 2,
    };
    assert_eq!(bounds.lifetime_peak_bytes(), Some(11));
    assert_eq!(
        native_face_scene::StandardNativeSceneBounds {
            retained_bytes: usize::MAX,
            ..bounds
        }
        .lifetime_peak_bytes(),
        None
    );
}
