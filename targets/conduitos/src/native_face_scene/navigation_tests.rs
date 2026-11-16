//! Focus and complete overflow reading under the acknowledged native Mask Show.
use super::*;
use alloc::{format, vec};
use conduit_face::*;
use conduit_human::{KeyEvent, KeyModifiers, KeyTransition};

fn key(usage: u8, shift: bool) -> KeyEvent {
    KeyEvent::new(
        usage,
        KeyTransition::Pressed,
        if shift {
            KeyModifiers::LEFT_SHIFT
        } else {
            KeyModifiers::NONE
        },
    )
    .unwrap()
}

fn crowded_face() -> Face {
    let original = tests::face(31);
    let mut actions = original.actions.clone();
    for index in 0..12 {
        let mut action = original.actions[1].clone();
        action.identity = format!("extra-{index}");
        action.name = format!("Action {index}: {}", "read every word é ".repeat(12));
        actions.push(action);
    }
    actions[1].availability = FaceActionAvailability::Unavailable {
        reason_code: "waiting".into(),
        explanation: "Waiting for source".into(),
    };
    actions[2].availability = FaceActionAvailability::Refused {
        reason_code: "denied".into(),
        explanation: "Owner denied request".into(),
    };
    Face::new_with_semantics(
        original.revision,
        original.basis,
        original.subjects,
        original.relationships,
        original.properties,
        original.text,
        actions,
        original.disclosures,
    )
    .unwrap()
}

#[test]
fn keyboard_reaches_every_available_control_and_every_detail_page() {
    let face = crowded_face();
    let show = tests::fixture::show(&face);
    for (width, height) in [(320, 240), (640, 480), (1280, 800)] {
        let mut scene = NativeFaceScene::prepare(face.clone(), width, height).unwrap();
        for details in [false, true] {
            if details {
                scene.key(key(59, false), &show, 1).unwrap();
            }
            assert_eq!(scene.showing_details(), details);
            scene.focus = None;
            let mut expected = vec![];
            for row in scene.rows() {
                if let Some(control) = row.control
                    && face.actions[control.action].availability.is_available()
                    && !expected.contains(&control)
                {
                    expected.push(control);
                }
            }
            for control in &expected {
                scene.key(key(43, false), &show, 1).unwrap();
                assert_eq!(scene.focused(), Some(*control));
                let frame = scene.frame().unwrap();
                assert!(frame.hits.iter().any(|hit| hit.control == *control));
                // Two distinct outlines remain distinct if all paints are equal.
                let outlines: Vec<_> = frame
                    .scene
                    .commands()
                    .iter()
                    .filter(|command| {
                        command.paint == GraphicsPaintRole::Focus
                            && command.kind == GraphicsCommandKind::Rect
                    })
                    .collect();
                assert!(outlines.len() >= 2);
                assert_ne!(outlines[0].bounds, outlines[1].bounds);
            }
            scene.key(key(43, false), &show, 1).unwrap();
            assert_eq!(scene.focused(), expected.first().copied());
            scene.key(key(43, true), &show, 1).unwrap();
            assert_eq!(scene.focused(), expected.last().copied());
            while scene.page() > 0 {
                scene.key(key(75, false), &show, 1).unwrap();
            }
            let mut visible = alloc::string::String::new();
            for page in 0..scene.pages() {
                assert_eq!(scene.page(), page);
                let frame = scene.frame().unwrap();
                assert!(frame.scene.commands().len() <= MAX_GRAPHICS_COMMANDS);
                for row in scene.rows().iter().filter(|row| row.page == page) {
                    visible.push_str(&row.text);
                }
                scene.key(key(78, false), &show, 1).unwrap();
            }
            assert!(visible.to_lowercase().contains("unavailable"));
            assert!(visible.to_lowercase().contains("refused"));
            for action in &face.actions {
                assert!(visible.contains(&action.name), "lost {}", action.identity);
            }
            assert_eq!(scene.face(), &face);
        }
    }
}

#[test]
fn pointer_selection_survives_keyboard_details_and_return() {
    let face = tests::face(32);
    let show = tests::fixture::show(&face);
    let mut scene = NativeFaceScene::prepare(face.clone(), 640, 480).unwrap();
    scene
        .focus_named(&FaceFocusRequest {
            action_id: "edit".into(),
            argument_name: Some("value".into()),
        })
        .unwrap();
    let (control, bounds) = {
        let frame = scene.frame().unwrap();
        let hit = frame
            .hits
            .iter()
            .find(|hit| hit.control.argument == Some(0))
            .unwrap();
        (hit.control, hit.bounds)
    };
    scene.pointer(bounds.x + 1, bounds.y + 1, &show, 1).unwrap();
    for details in [true, false] {
        scene.key(key(59, false), &show, 1).unwrap();
        assert_eq!(scene.showing_details(), details);
        assert_eq!(scene.focused(), Some(control));
        assert!(
            scene
                .frame()
                .unwrap()
                .hits
                .iter()
                .any(|hit| hit.control == control)
        );
    }
    assert_eq!(scene.face(), &face);
}

#[test]
fn exact_reading_capture_and_restore_allocate_no_storage() {
    let face = crowded_face();
    let show = tests::fixture::show(&face);
    let mut scene = NativeFaceScene::prepare(face.clone(), 640, 480).unwrap();
    scene.key(key(78, false), &show, 1).unwrap();
    assert!(scene.page() > 0);
    let mut fresh = NativeFaceScene::prepare(face, 640, 480).unwrap();
    let allocations = crate::test_allocations::allocations(|| {
        let position: NativeReadingPosition = scene.reading_position().unwrap();
        assert!(fresh.restore_reading_position(position));
    });
    assert_eq!(allocations, 0);
    assert_eq!(fresh.page(), scene.page());
}

#[test]
fn reading_restore_refuses_changed_document_extent_and_typography_without_mutation() {
    let face = crowded_face();
    let mut scene = NativeFaceScene::prepare(face.clone(), 640, 480).unwrap();
    scene.turn_page(true);
    let position = scene.reading_position().unwrap();
    assert!(!core::mem::needs_drop::<NativeReadingPosition>());
    let mut changed_extent = NativeFaceScene::prepare(face.clone(), 320, 240).unwrap();
    assert!(!changed_extent.restore_reading_position(position));
    assert_eq!(changed_extent.page(), 0);
    let mut changed = NativeFaceScene::prepare(face, 640, 480).unwrap();
    changed.face.revision += 1;
    assert!(!changed.restore_reading_position(position));
    assert_eq!(changed.page(), 0);
    changed.face.revision -= 1;
    changed.text_scale_percent += 1;
    assert!(!changed.restore_reading_position(position));
    assert_eq!(changed.page(), 0);
    changed.text_scale_percent -= 1;
    changed.typography = Typography::new(VisualTypography::Engineering);
    assert!(!changed.restore_reading_position(position));
    assert_eq!(changed.page(), 0);
    changed.typography = scene.typography;
    changed.local_notice = Some("notice".into());
    assert!(changed.reading_position().is_none());
    assert!(!changed.restore_reading_position(position));
    assert_eq!(changed.page(), 0);
}

#[test]
fn document_cursor_cannot_replace_active_graph_inspection() {
    let face: Face = serde_json::from_str(include_str!(
        "../../../../proof/conformance/face-waist/domain-derived/patchbay-face.json"
    ))
    .unwrap();
    let mut scene = NativeFaceScene::prepare(face.clone(), 1280, 800).unwrap();
    let position = scene.reading_position().unwrap();
    let subject = face
        .subjects
        .iter()
        .find(|subject| subject.role == conduit_face::FaceRole::Gear)
        .unwrap();
    scene
        .inspect_subject(&face.identity, face.revision, &subject.identity)
        .unwrap();
    let inspection = scene.graph_projection().unwrap().cursor.clone();
    let page = scene.page();
    assert!(scene.reading_position().is_none());
    assert!(!scene.restore_reading_position(position));
    assert_eq!(scene.graph_projection().unwrap().cursor, inspection);
    assert_eq!(scene.page(), page);
    assert_eq!(scene.face(), &face);
}

#[test]
fn reading_position_refuses_changed_identity_and_live_keyboard_draft() {
    let face = tests::face(1);
    let mut scene = NativeFaceScene::prepare(face.clone(), 640, 480).unwrap();
    let position = scene.reading_position().unwrap();
    let original = scene.face.identity.clone();
    scene.face.identity = tests::face(2).identity;
    assert_ne!(scene.face.identity, original);
    assert!(!scene.restore_reading_position(position));
    assert_eq!(scene.page(), 0);
    scene.face.identity = original;
    scene
        .focus_named(&FaceFocusRequest {
            action_id: "edit".into(),
            argument_name: Some("value".into()),
        })
        .unwrap();
    scene
        .key(key(4, false), &tests::fixture::show(&face), 1)
        .unwrap();
    assert!(scene.has_pending_input());
    let focus = scene.focus;
    assert!(scene.reading_position().is_none());
    assert!(!scene.restore_reading_position(position));
    assert!(scene.has_pending_input());
    assert_eq!(scene.focus, focus);
    assert_eq!(scene.page(), 0);
}

#[test]
fn restored_reading_cancels_tap_until_a_new_physical_press_edge() {
    let face = tests::face(1);
    let show = tests::fixture::show(&face);
    let mut scene = NativeFaceScene::prepare(face, 640, 480).unwrap();
    scene
        .focus_named(&FaceFocusRequest {
            action_id: "finish".into(),
            argument_name: None,
        })
        .unwrap();
    let control = scene.focused().unwrap();
    let position = scene.reading_position().unwrap();
    let frame = scene.frame().unwrap();
    let hit = frame
        .hits
        .iter()
        .find(|hit| hit.control == control)
        .unwrap();
    let (x, y) = (hit.bounds.x + 4, hit.bounds.y + 4);
    scene.pointer_sample(x, y, true, true, &show, 1).unwrap();
    assert_eq!(scene.pressed_control(), Some(control));
    assert!(scene.restore_reading_position(position));
    assert_eq!(scene.pressed_control(), None);
    for (pressed, sequence) in [(true, 2), (false, 3)] {
        assert!(matches!(
            scene
                .pointer_sample(x, y, pressed, false, &show, sequence)
                .unwrap(),
            FaceSceneInput::Unchanged
        ));
        assert_eq!(scene.pressed_control(), None);
    }
    scene.pointer_sample(x, y, true, true, &show, 4).unwrap();
    assert_eq!(scene.pressed_control(), Some(control));
    assert!(matches!(
        scene.pointer_sample(x, y, false, false, &show, 5).unwrap(),
        FaceSceneInput::Submitted { .. }
    ));
}

#[test]
fn refused_reading_restore_preserves_the_pending_tap() {
    let face = tests::face(1);
    let show = tests::fixture::show(&face);
    let wrong = NativeFaceScene::prepare(face.clone(), 320, 240)
        .unwrap()
        .reading_position()
        .unwrap();
    let mut scene = NativeFaceScene::prepare(face, 640, 480).unwrap();
    scene
        .focus_named(&FaceFocusRequest {
            action_id: "finish".into(),
            argument_name: None,
        })
        .unwrap();
    let control = scene.focused().unwrap();
    let page = scene.page();
    let frame = scene.frame().unwrap();
    let hit = frame
        .hits
        .iter()
        .find(|hit| hit.control == control)
        .unwrap();
    let (x, y) = (hit.bounds.x + 4, hit.bounds.y + 4);
    scene.pointer_sample(x, y, true, true, &show, 1).unwrap();
    assert!(!scene.restore_reading_position(wrong));
    assert_eq!(scene.pressed_control(), Some(control));
    assert_eq!(scene.focused(), Some(control));
    assert_eq!(scene.page(), page);
    assert!(matches!(
        scene.pointer_sample(x, y, false, false, &show, 2).unwrap(),
        FaceSceneInput::Submitted { .. }
    ));
}
