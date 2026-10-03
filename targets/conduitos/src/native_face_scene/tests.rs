use super::*;
use alloc::{string::String, vec};
use conduit_human::{KeyEvent, KeyModifiers, KeyTransition};
use conduit_presentation::*;
#[path = "fixture.rs"]
mod fixture;

fn face(revision: u64) -> Presentation {
    let producer = fixture::producer_plan();
    Presentation::new_with_semantics(
        revision,
        PresentationBasis {
            body_id: Some(serde_json::from_str("\"body/native-face-fixture\"").unwrap()),
            wake_id: None,
            source_document_id: Some(producer.source_document_id),
            checked_plot_id: Some(producer.checked_plot_id),
            expanded_plot_id: Some(producer.expanded_plot_id),
            plan_id: Some(producer.plan_id),
            active_play_id: None,
            sign_ids: vec![],
        },
        vec![
            PresentationSubject {
                identity: "z-first".into(),
                role: PresentationRole::Document,
                name: "A readable encounter".into(),
            },
            PresentationSubject {
                identity: "a-field".into(),
                role: PresentationRole::TextEntry,
                name: "Friendly name".into(),
            },
        ],
        vec![PresentationRelationship {
            source: "z-first".into(),
            target: "a-field".into(),
            kind: PresentationRelationshipKind::Contains,
        }],
        vec![PresentationProperty {
            subject: "z-first".into(),
            name: "producer".into(),
            value: PresentationPropertyValue::Identity("opaque/sha256:producer".into()),
        }],
        vec![PresentationText {
            subject: "a-field".into(),
            text: "Give this encounter a name.".into(),
        }],
        vec![
            PresentationAction {
                identity: "edit".into(),
                intent: "test/edit".into(),
                target: "a-field".into(),
                name: "Change name".into(),
                arguments: vec![
                    FaceActionArgument::text("value".into(), "Name".into(), 0, 64).unwrap(),
                ],
                disclosure: PresentationDisclosureLevel::CurrentAction,
                availability: PresentationActionAvailability::Available,
            },
            PresentationAction {
                identity: "finish".into(),
                intent: "test/finish".into(),
                target: "z-first".into(),
                name: "Finish".into(),
                arguments: vec![],
                disclosure: PresentationDisclosureLevel::CurrentAction,
                availability: PresentationActionAvailability::Available,
            },
        ],
        vec![],
    )
    .unwrap()
}

fn press(usage: u8) -> KeyEvent {
    KeyEvent::new(usage, KeyTransition::Pressed, KeyModifiers::NONE).unwrap()
}

#[test]
fn modifier_press_on_argument_free_action_does_not_submit_or_refuse() {
    let original = face(2);
    let show = fixture::show(&original);
    let mut scene = NativeFaceScene::prepare(original, 640, 480).unwrap();
    scene
        .focus_named(&FaceFocusRequest {
            action_id: "finish".into(),
            argument_name: None,
        })
        .unwrap();
    let shift = KeyEvent::new(225, KeyTransition::Pressed, KeyModifiers::LEFT_SHIFT).unwrap();
    assert!(matches!(
        scene.key(shift, &show, 1),
        Ok(FaceSceneInput::Unchanged)
    ));
}

fn focus_input(scene: &mut NativeFaceScene) {
    scene
        .focus_named(&FaceFocusRequest {
            action_id: "edit".into(),
            argument_name: Some("value".into()),
        })
        .unwrap();
}

#[test]
fn full_face_retained_and_semantic_order_is_not_action_index_order() {
    let original = face(1);
    let mut scene = NativeFaceScene::prepare(original.clone(), 640, 480).unwrap();
    let frame = scene.frame().unwrap();
    assert_eq!(frame.presentation_id, &original.identity);
    assert_eq!(frame.revision, 1);
    assert_eq!(frame.page, scene.page());
    assert_eq!(frame.pages, scene.pages());
    assert!(!scene.showing_details());
    assert!(scene.focused().is_none());
    assert_eq!(frame.scene.commands()[1].payload(), "A readable encounter");
    assert!(
        !scene
            .primary
            .iter()
            .any(|row| row.text.contains("opaque/sha256"))
    );
    assert!(
        scene
            .details
            .iter()
            .any(|row| row.text.contains("opaque/sha256"))
    );
    assert_eq!(
        scene.focus_next(true),
        Some(FaceControl {
            action: 1,
            argument: None
        })
    );
    assert_eq!(
        scene.focus_next(true),
        Some(FaceControl {
            action: 0,
            argument: None
        })
    );
    assert_eq!(
        scene.focus_next(true),
        Some(FaceControl {
            action: 0,
            argument: Some(0)
        })
    );
    assert_eq!(
        scene.focus_next(false),
        Some(FaceControl {
            action: 0,
            argument: None
        })
    );
    scene.show_details(true);
    scene.turn_page(true);
    assert_eq!(scene.presentation(), &original);
}

#[test]
fn every_utf8_byte_survives_layout_pages_with_bounded_commands() {
    let mut original = face(2);
    let long = "é中 long readable words ".repeat(30);
    original.text = vec![PresentationText {
        subject: "a-field".into(),
        text: long.clone(),
    }];
    // Use the canonical constructor to recompute identity after replacing text.
    original = Presentation::new_with_semantics(
        2,
        original.basis,
        original.subjects,
        original.relationships,
        original.properties,
        original.text,
        original.actions,
        original.disclosures,
    )
    .unwrap();
    let mut scene = NativeFaceScene::prepare(original.clone(), 320, 240).unwrap();
    let joined: String = scene
        .primary
        .iter()
        .filter(|row| row.control.is_none() && row.role == GraphicsTextRole::Body)
        .map(|row| row.text.as_str())
        .collect();
    assert_eq!(joined, long);
    for _ in 0..scene.pages() {
        let frame = scene.frame().unwrap();
        assert!(frame.scene.commands().len() <= MAX_GRAPHICS_COMMANDS);
        assert!(
            frame
                .scene
                .commands()
                .iter()
                .all(|command| command.clip_class() == GraphicsClipClass::FullyVisible)
        );
        scene.turn_page(true);
    }
    assert_eq!(scene.presentation(), &original);
}

#[test]
fn typing_and_tab_commit_exact_argument_then_refuse_more_input_on_old_show() {
    let original = face(3);
    let show = fixture::show(&original);
    let mut scene = NativeFaceScene::prepare(original.clone(), 640, 480).unwrap();
    focus_input(&mut scene);
    for usage in [11, 12] {
        assert!(matches!(
            scene.key(press(usage), &show, 1).unwrap(),
            FaceSceneInput::Changed
        ));
    }
    assert!(
        scene
            .frame()
            .unwrap()
            .scene
            .commands()
            .iter()
            .any(|command| command.payload().contains("Draft: hi"))
    );
    let FaceSceneInput::Submitted {
        interaction,
        next_focus,
    } = scene.key(press(43), &show, 7).unwrap()
    else {
        panic!("expected commit");
    };
    assert_eq!(interaction.action_id, "edit");
    assert_eq!(interaction.face_revision, 3);
    assert_eq!(interaction.arguments[0].value, b"hi");
    assert!(next_focus.is_some());
    assert!(matches!(
        scene.key(press(40), &show, 8),
        Err(FaceSceneError::StaleFace)
    ));
    assert_eq!(scene.presentation(), &original);
}

#[test]
fn stale_show_unacknowledged_show_and_stale_mapping_refuse() {
    let original = face(4);
    let show = fixture::show(&original);
    let mut scene = NativeFaceScene::prepare(face(5), 640, 480).unwrap();
    assert!(matches!(
        scene.key(press(43), &show, 1),
        Err(FaceSceneError::StaleFace)
    ));
    assert_eq!(
        scene
            .resolve(
                &original.identity,
                4,
                FaceControl {
                    action: 0,
                    argument: Some(0)
                }
            )
            .unwrap_err(),
        FaceSceneError::StaleFace
    );
    scene = NativeFaceScene::prepare(original.clone(), 640, 480).unwrap();
    let prepared = fixture::prepared(&original);
    assert!(matches!(
        scene.key(press(43), &prepared, 1),
        Err(FaceSceneError::StaleFace)
    ));
}

#[test]
fn pointer_mapping_retains_contract_and_never_invokes_an_unavailable_action() {
    let mut original = face(6);
    original.actions[1].availability = PresentationActionAvailability::Unavailable {
        reason_code: "test/no".into(),
        explanation: "Not ready".into(),
    };
    original = Presentation::new_with_semantics(
        6,
        original.basis,
        original.subjects,
        original.relationships,
        original.properties,
        original.text,
        original.actions,
        original.disclosures,
    )
    .unwrap();
    let show = fixture::show(&original);
    let mut scene = NativeFaceScene::prepare(original, 640, 480).unwrap();
    focus_input(&mut scene);
    let frame = scene.frame().unwrap();
    let hit = frame
        .hits
        .iter()
        .find(|hit| hit.argument.is_some())
        .unwrap();
    assert_eq!(hit.argument.unwrap().contract.maximum_bytes, 64);
    assert!(frame.hits.iter().all(|hit| hit.action.identity != "finish"));
    let (x, y) = (hit.bounds.x + 1, hit.bounds.y + 1);
    assert!(matches!(
        scene.pointer(x, y, &show, 1).unwrap(),
        FaceSceneInput::Changed
    ));
}

#[test]
fn text_bound_and_backspace_preserve_valid_replacement() {
    let original = face(8);
    let show = fixture::show(&original);
    let mut scene = NativeFaceScene::prepare(original, 640, 480).unwrap();
    focus_input(&mut scene);
    for _ in 0..64 {
        scene.key(press(4), &show, 1).unwrap();
    }
    assert!(matches!(
        scene.key(press(4), &show, 1),
        Err(FaceSceneError::InputBound)
    ));
    scene.key(press(42), &show, 1).unwrap();
    let FaceSceneInput::Submitted { interaction, .. } = scene.key(press(40), &show, 1).unwrap()
    else {
        panic!()
    };
    assert_eq!(interaction.arguments[0].value.len(), 63);
}

#[test]
fn face_words_render_as_nonuniform_scene_pixels() {
    use crate::display::{DisplayError, DisplayFormat, PixelTarget, RetainedPixelTarget};
    struct Pixels(Vec<u32>);
    impl PixelTarget for Pixels {
        fn format(&self) -> DisplayFormat {
            DisplayFormat {
                width: 640,
                height: 480,
                pitch: 2560,
                bits_per_pixel: 32,
                red_shift: 16,
                green_shift: 8,
                blue_shift: 0,
            }
        }
        fn write_pixel(&mut self, x: u32, y: u32, value: u32) -> Result<(), DisplayError> {
            self.0[(y * 640 + x) as usize] = value;
            Ok(())
        }
    }
    impl RetainedPixelTarget for Pixels {
        fn read_pixel(&self, x: u32, y: u32) -> Result<u32, DisplayError> {
            Ok(self.0[(y * 640 + x) as usize])
        }
    }
    let mut pixels = Pixels(vec![0; 640 * 480]);
    let scene = NativeFaceScene::prepare(face(9), 640, 480).unwrap();
    crate::display::typography::render_scene(
        &mut pixels,
        &scene.frame().unwrap().scene,
        |_, command| command.text_role().into(),
    )
    .unwrap();
    assert!(pixels.0.windows(2).any(|pair| pair[0] != pair[1]));
}
