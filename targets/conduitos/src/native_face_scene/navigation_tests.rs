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
