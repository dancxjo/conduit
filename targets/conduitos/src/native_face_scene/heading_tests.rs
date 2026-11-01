//! Human encounter wording without guessed Body context or new authority.
use super::*;
use alloc::vec;
use conduit_face::{FaceRole, FaceSubject};

fn named_bodies(names: &[&str]) -> Face {
    let original = tests::face(8);
    Face::new_with_semantics(
        original.revision,
        original.basis.clone(),
        if names.is_empty() {
            vec![FaceSubject {
                identity: "fixture/info".into(),
                role: FaceRole::Info,
                name: "Reading".into(),
            }]
        } else {
            names
                .iter()
                .enumerate()
                .map(|(i, name)| FaceSubject {
                    identity: alloc::format!("fixture/body/{i}"),
                    role: FaceRole::Body,
                    name: (*name).into(),
                })
                .collect()
        },
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
    )
    .unwrap()
}

fn heading(scene: &NativeFaceScene) -> String {
    scene.frame().unwrap().scene.commands()[1].payload().into()
}

#[test]
fn one_explicit_body_names_both_input_modes_without_mutating_face_or_hits() {
    let face = named_bodies(&["Malo Lefèvre Laurent"]);
    for admitted in [false, true] {
        let mut scene =
            NativeFaceScene::prepare_with_input(face.clone(), 1024, 600, admitted).unwrap();
        for details in [false, true] {
            scene.show_details(details);
            assert_eq!(heading(&scene), "Malo Lefèvre Laurent");
            assert_eq!(scene.face(), &face);
            let frame = scene.frame().unwrap();
            assert!(frame.hits.is_empty());
            assert!(frame.scene.commands().len() <= conduit_face::MAX_GRAPHICS_COMMANDS);
            assert!(frame.scene.commands().iter().all(
                |command| command.clip_class() == conduit_face::GraphicsClipClass::FullyVisible
            ));
        }
    }
}

#[test]
fn missing_and_ambiguous_body_names_never_select_an_arbitrary_body() {
    for names in [vec![], vec!["One", "Two"]] {
        let face = named_bodies(&names);
        let mut scene =
            NativeFaceScene::prepare_with_input(face.clone(), 1024, 600, false).unwrap();
        assert_eq!(heading(&scene), "Overview");
        scene.show_details(true);
        assert_eq!(heading(&scene), "Details");
        assert_eq!(scene.face(), &face);
    }
}

#[test]
fn oversized_unicode_heading_keeps_its_complete_name_in_the_paged_document() {
    let name = "Élan ".repeat(60);
    let face = named_bodies(&[&name]);
    let mut scene = NativeFaceScene::prepare_with_input(face.clone(), 320, 240, false).unwrap();
    assert_eq!(heading(&scene), "View");
    let words: String = scene.primary.iter().map(|row| row.text.as_str()).collect();
    assert_eq!(words, name);
    for _ in 0..scene.pages() {
        let frame = scene.frame().unwrap();
        assert!(
            frame
                .scene
                .commands()
                .iter()
                .all(|command| command.payload().len() <= conduit_face::MAX_GRAPHICS_TEXT_BYTES)
        );
        scene.turn_page(true);
    }
    assert_eq!(scene.face(), &face);
}

// Explicit fixture composition around actual Todo producer output. This is not
// a joined/live Body or a source of membership/interaction authority.
fn with_body(mut original: Face, name: &str) -> Face {
    assert!(original.subjects.iter().all(|s| s.role != FaceRole::Body));
    original.subjects.push(FaceSubject {
        identity: "fixture/body-heading".into(),
        role: FaceRole::Body,
        name: name.into(),
    });
    Face::new_with_semantics(
        original.revision,
        original.basis,
        original.subjects,
        original.relationships,
        original.properties,
        original.text,
        original.actions,
        original.disclosures,
    )
    .unwrap()
}

fn audit_named_pages(
    mut scene: NativeFaceScene,
    face: &Face,
    name: &str,
) -> alloc::collections::BTreeSet<(usize, Option<usize>)> {
    let legacy =
        chrome::Chrome::prepare_selected(scene.screen, scene.text_scale_percent, scene.typography)
            .unwrap();
    assert!(
        scene.header_top > legacy.top,
        "test name must need additional measured chrome"
    );
    assert!(
        scene
            .primary
            .iter()
            .chain(&scene.details)
            .all(|row| row.bounds.y >= scene.header_top as i16)
    );
    let mut controls = alloc::collections::BTreeSet::new();
    for details in [false, true] {
        scene.show_details(details);
        for _ in 0..scene.pages() {
            let frame = scene.frame().unwrap();
            assert_eq!(frame.scene.commands()[1].payload(), name);
            assert_eq!(
                frame.scene.commands()[1].bounds.height,
                scene.header_top - 16
            );
            assert!(frame.scene.commands().len() <= conduit_face::MAX_GRAPHICS_COMMANDS);
            assert!(
                frame
                    .scene
                    .commands()
                    .iter()
                    .all(|c| c.clip_class() == conduit_face::GraphicsClipClass::FullyVisible)
            );
            for hit in &frame.hits {
                assert_eq!(
                    frame
                        .hit_test(hit.bounds.x + 1, hit.bounds.y + 1)
                        .unwrap()
                        .control,
                    hit.control
                );
                controls.insert((hit.control.action, hit.control.argument));
            }
            scene.turn_page(true);
        }
    }
    assert_eq!(scene.face(), face);
    controls
}

#[test]
fn full_body_name_wraps_in_generic_configured_and_mechanical_field_looks() {
    use conduit_face::{VisualUserConstraints, admit_visual_look};
    let name = "W".repeat(conduit_body::MAX_BODY_FRIENDLY_NAME_BYTES);
    let face = with_body(structured_source_tests::todo_with_items(20), &name);
    for source in ["todo-component-flat", "todo-component-grouped"] {
        let structure = structured_source_tests::structure(
            conduit_face::TODO_LAYOUT_MASK_SOURCE,
            source,
            &face,
        );
        for (width, height, scale) in [(1024, 600, 100), (1280, 800, 200)] {
            let mut reference = None;
            for look in [
                conduit_face::field_instrument_day_look().unwrap(),
                conduit_face::field_instrument_night_look().unwrap(),
            ] {
                let look = admit_visual_look(
                    &look,
                    VisualUserConstraints {
                        forced_colors: false,
                        reduced_motion: true,
                        minimum_text_scale_percent: scale,
                        maximum_supported_text_scale_percent: 200,
                    },
                )
                .unwrap();
                let generic =
                    NativeFaceScene::prepare_visual_look(face.clone(), width, height, true, &look)
                        .unwrap();
                let configured = NativeFaceScene::prepare_configured_look(
                    face.clone(),
                    width,
                    height,
                    true,
                    structure.recipe(),
                    &look,
                )
                .unwrap();
                let mechanical = NativeFaceScene::prepare_structured_look(
                    &structure, width, height, true, &look,
                )
                .unwrap();
                let controls: Vec<_> = [generic, configured, mechanical]
                    .into_iter()
                    .map(|scene| audit_named_pages(scene, &face, &name))
                    .collect();
                assert!(controls.iter().all(|controls| !controls.is_empty()));
                if let Some(expected) = &reference {
                    assert_eq!(&controls, expected);
                }
                reference = Some(controls);
            }
        }
    }
}

#[test]
fn unfit_heading_keeps_complete_content_at_large_legacy_text_scale() {
    use conduit_face::{VisualExpression, VisualUserConstraints, admit_visual_expression};
    let name = "W".repeat(conduit_body::MAX_BODY_FRIENDLY_NAME_BYTES);
    let face = named_bodies(&[&name]);
    let mut requested = VisualExpression::phosphor();
    requested.text_scale_percent = 200;
    let expression = admit_visual_expression(
        Ok(requested),
        VisualUserConstraints {
            forced_colors: false,
            reduced_motion: true,
            minimum_text_scale_percent: 100,
            maximum_supported_text_scale_percent: 200,
        },
    )
    .unwrap();
    let mut scene =
        NativeFaceScene::prepare_visual_fallback(face.clone(), 320, 240, false, &expression)
            .unwrap();
    let legacy = chrome::Chrome::prepare_selected(scene.screen, 200, scene.typography).unwrap();
    assert_eq!(scene.header_top, legacy.top);
    assert_eq!(heading(&scene), "View");
    assert_eq!(
        scene
            .primary
            .iter()
            .map(|row| row.text.as_str())
            .collect::<String>(),
        name
    );
    for _ in 0..scene.pages() {
        let frame = scene.frame().unwrap();
        assert!(
            frame
                .scene
                .commands()
                .iter()
                .all(|c| c.clip_class() == conduit_face::GraphicsClipClass::FullyVisible)
        );
        scene.turn_page(true);
    }
    assert_eq!(scene.face(), &face);
}

#[test]
fn active_editor_uses_the_same_admitted_heading_extent() {
    use conduit_face::{FaceProperty, FacePropertyValue};
    let name = "W".repeat(conduit_body::MAX_BODY_FRIENDLY_NAME_BYTES);
    let mut original = tests::face(7);
    original.properties.push(FaceProperty {
        subject: "z-first".into(),
        name: "text-editing-active".into(),
        value: FacePropertyValue::Flag(true),
    });
    let face = with_body(original, &name);
    let scene = NativeFaceScene::prepare_with_input(face.clone(), 1024, 600, true).unwrap();
    assert!(scene.header_top > chrome::HEADER);
    let frame = scene.frame().unwrap();
    assert_eq!(frame.scene.commands()[1].payload(), name);
    assert!(
        frame
            .scene
            .commands()
            .iter()
            .any(|c| c.kind == conduit_face::GraphicsCommandKind::Rect
                && c.paint == GraphicsPaintRole::Focus
                && c.bounds.y == scene.header_top as i16)
    );
    assert_eq!(scene.face(), &face);
}
