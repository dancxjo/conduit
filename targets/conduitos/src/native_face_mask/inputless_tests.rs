use super::*;
use conduit_human::KeyModifiers;

#[test]
fn inputless_host_preserves_face_and_show_but_refuses_local_routes() {
    let (service, draft) = tests::fixture();
    let mut service = service.without_local_input().unwrap();
    let face = draft
        .face(&service.birth_basis("arrival/no-input"))
        .unwrap();
    let mut display = tests::CountingDisplay::default();
    service.present(face.clone(), 1, 1, &mut display).unwrap();
    let show_id = service.show().unwrap().show_id.clone();
    assert_eq!(service.scene().unwrap().face(), &face);
    assert!(!service.route_keyboard().unwrap());
    let frame = service.scene().unwrap().frame().unwrap();
    assert!(frame.hits.is_empty());
    assert!(
        frame
            .scene
            .commands()
            .iter()
            .any(|command| { command.payload() == "Read only · No local input" })
    );
    let page = KeyEvent::new(78, KeyTransition::Pressed, KeyModifiers::NONE).unwrap();
    assert!(matches!(
        service.key(page, 2, &mut display),
        Err(NativeFaceMaskError::InputUnavailable)
    ));
    assert!(matches!(
        service.pointer(0, 0, 3, &mut display),
        Err(NativeFaceMaskError::InputUnavailable)
    ));
    assert_eq!(
        service.pointer_motion(0, 0, &mut display),
        Err(NativeFaceMaskError::InputUnavailable)
    );
    assert_eq!(service.show().unwrap().show_id, show_id);
    assert_eq!(service.scene().unwrap().face(), &face);
}

#[test]
fn inputless_chrome_is_admitted_at_narrow_and_wide_extents_without_key_hints() {
    let (_, draft) = tests::fixture();
    let (service, _) = tests::fixture();
    let face = draft
        .face(&service.birth_basis("arrival/no-input-layout"))
        .unwrap();
    for width in [320, 640, 800] {
        let mut scene =
            NativeFaceScene::prepare_with_input(face.clone(), width, 600, false).unwrap();
        scene.disable_local_input();
        let frame = scene.frame().unwrap();
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
                .any(|command| command.payload() == expected)
        );
        assert!(!frame.scene.commands().iter().any(|command| {
            command.payload().contains("PgUp/PgDn") || command.payload().contains("F2 facts")
        }));
        assert_eq!(scene.face(), &face);
    }
}
