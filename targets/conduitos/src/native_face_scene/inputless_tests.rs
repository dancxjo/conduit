//! A display-only Host cannot navigate or submit through the scene directly.
use super::*;
use conduit_human::{KeyEvent, KeyModifiers, KeyTransition};

#[test]
fn absent_local_input_preserves_face_and_page_and_removes_navigation_hints() {
    let face = tests::face(23);
    let show = tests::fixture::show(&face);
    for width in [320, 640, 800] {
        let mut scene = NativeFaceScene::prepare(face.clone(), width, 600).unwrap();
        scene.disable_local_input();
        let original_page = scene.page();
        let frame = scene.frame().unwrap();
        assert!(frame.hits.is_empty());
        let expected = if width < 640 {
            "No input"
        } else {
            "Read only · No local input"
        };
        assert!(
            frame
                .scene
                .commands()
                .iter()
                .any(|c| c.payload() == expected)
        );
        assert!(
            !frame
                .scene
                .commands()
                .iter()
                .any(|c| { c.payload().contains("PgUp/PgDn") || c.payload().contains("F2 facts") })
        );
        for usage in [43, 59, 75, 78] {
            let key = KeyEvent::new(usage, KeyTransition::Pressed, KeyModifiers::NONE).unwrap();
            assert!(matches!(
                scene.key(key, &show, 2),
                Err(FaceSceneError::UnsupportedInput)
            ));
        }
        assert!(matches!(
            scene.pointer(0, 0, &show, 3),
            Err(FaceSceneError::UnsupportedInput)
        ));
        assert_eq!(scene.page(), original_page);
        assert!(!scene.showing_details());
        assert_eq!(scene.face(), &face);
    }
}
