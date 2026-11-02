use super::*;

#[test]
fn static_owner_keeps_complete_semantic_face_and_identical_initial_frame() {
    let original = super::super::tests::face(1);
    let ordinary = NativeFaceScene::prepare_with_input(original.clone(), 640, 480, false).unwrap();
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
