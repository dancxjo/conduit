use super::*;

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
    assert_eq!(
        static_scene.frame().unwrap().scene,
        ordinary.frame().unwrap().scene
    );
    assert!(
        static_scene.retained_backing_bytes().unwrap()
            >= original.retained_backing_bytes().unwrap()
    );
    assert!(!static_scene.scene.local_input_available);
    assert!(static_scene.scene.graph_reading.is_none());
}
