use super::super::PreparedStaticNativeFaceScene;
use super::NativeFaceScene;

#[test]
fn static_owner_keeps_complete_semantic_face_and_identical_initial_frame() {
    let original = super::super::tests::face(1);
    let mut ordinary =
        NativeFaceScene::prepare_with_input(original.clone(), 640, 480, false).unwrap();
    // Compare the same installed-input truth. Retain its graph projection to
    // prove that omitting that inactive owner preserves the read-only frame.
    ordinary.local_input_available = false;
    let static_scene = PreparedStaticNativeFaceScene::prepare(original.clone(), 640, 480).unwrap();
    assert_eq!(static_scene.scene.face, original);
    let static_frame = static_scene.frame().unwrap();
    assert_eq!(static_frame.scene, ordinary.frame().unwrap().scene);
    // Read-only presentation still owns its finite hit buffer; its IMAGE
    // accounting must not mistake absence of active input for zero backing.
    assert!(static_frame.retained_backing_bytes().unwrap() > 0);
    assert!(
        static_scene.retained_backing_bytes().unwrap()
            >= original.retained_backing_bytes().unwrap()
    );
    assert!(!static_scene.scene.local_input_available);
    assert!(static_scene.scene.graph_reading.is_none());
}
