//! One admitted Body heading never hides another subject or source wording.
use super::*;
use conduit_face::FaceText;

fn reseal(face: Face) -> Face {
    Face::new_with_semantics(
        face.revision,
        face.basis,
        face.subjects,
        face.relationships,
        face.properties,
        face.text,
        face.actions,
        face.disclosures,
    )
    .unwrap()
}

#[test]
fn single_body_heading_preserves_same_named_host_source_text_and_actions() {
    let name = "Élan 雪";
    let mut face = with_body(structured_source_tests::todo_with_items(2), name);
    face.subjects.push(FaceSubject {
        identity: "fixture/host".into(),
        role: FaceRole::Host,
        name: name.into(),
    });
    face.text.push(FaceText {
        subject: "fixture/body-heading".into(),
        text: name.into(),
    });
    let face = reseal(face);
    let body = face
        .subjects
        .iter()
        .position(|s| s.role == FaceRole::Body)
        .unwrap();
    let host = face
        .subjects
        .iter()
        .position(|s| s.role == FaceRole::Host)
        .unwrap();
    let structure = structured_source_tests::structure(
        conduit_face::TODO_LAYOUT_MASK_SOURCE,
        "todo-component-flat",
        &face,
    );
    for scale in [100, 200] {
        let mut reference = None;
        for night in [false, true] {
            let look = look(night, scale);
            for input in [false, true] {
                let scenes = [
                    NativeFaceScene::prepare_visual_look(face.clone(), 1280, 800, input, &look)
                        .unwrap(),
                    NativeFaceScene::prepare_configured_look(
                        face.clone(),
                        1280,
                        800,
                        input,
                        structure.recipe(),
                        &look,
                    )
                    .unwrap(),
                    NativeFaceScene::prepare_structured_look(&structure, 1280, 800, input, &look)
                        .unwrap(),
                ];
                let controls: Vec<_> =
                    scenes
                        .into_iter()
                        .map(|mut scene| {
                            assert_eq!(heading(&scene), name);
                            assert_eq!(scene.face(), &face);
                            assert!(!scene.primary.iter().any(|r| r.subject_index == Some(body)
                                && r.role == GraphicsTextRole::Title));
                            assert!(scene.primary.iter().any(|r| r.subject_index == Some(body)
                                && r.text == name
                                && r.role == GraphicsTextRole::Status));
                            assert!(scene.primary.iter().any(|r| r.subject_index == Some(host)
                                && r.text == name
                                && r.role == GraphicsTextRole::Title));
                            let (_, exact) = document::prepare(&face, input).unwrap();
                            assert_eq!(
                                scene
                                    .details
                                    .iter()
                                    .map(|r| r.text.as_str())
                                    .collect::<String>(),
                                exact.iter().map(|r| r.text.as_str()).collect::<String>()
                            );
                            let mut controls = Vec::new();
                            for _ in 0..scene.pages() {
                                let frame = scene.frame().unwrap();
                                assert!(
                                    frame.scene.commands().len()
                                        <= conduit_face::MAX_GRAPHICS_COMMANDS
                                );
                                assert!(frame.scene.commands().iter().all(|c| c.clip_class()
                                    == conduit_face::GraphicsClipClass::FullyVisible));
                                for hit in &frame.hits {
                                    assert_eq!(
                                        frame
                                            .hit_test(hit.bounds.x + 1, hit.bounds.y + 1)
                                            .unwrap()
                                            .control,
                                        hit.control
                                    );
                                    controls.push((hit.control.action, hit.control.argument));
                                }
                                scene.turn_page(true);
                            }
                            controls
                        })
                        .collect();
                if input {
                    assert!(controls.iter().all(|c| !c.is_empty()));
                    assert!(controls.iter().all(|c| c == &controls[0]));
                    if let Some(reference) = &reference {
                        assert_eq!(&controls, reference);
                    }
                    reference = Some(controls);
                } else {
                    assert!(controls.iter().all(|c| c.is_empty()));
                }
            }
        }
    }
}

#[test]
fn single_body_heading_handles_body_only_blank_names_and_ambiguity() {
    for names in [vec!["Élan 雪"], vec!["  "], vec!["One", "Two"]] {
        let face = named_bodies(&names);
        for input in [false, true] {
            for mut scene in [
                NativeFaceScene::prepare_with_input(face.clone(), 1024, 600, input).unwrap(),
                NativeFaceScene::prepare_visual_look(
                    face.clone(),
                    1024,
                    600,
                    input,
                    &look(false, 100),
                )
                .unwrap(),
                NativeFaceScene::prepare_visual_look(
                    face.clone(),
                    1024,
                    600,
                    input,
                    &look(true, 200),
                )
                .unwrap(),
            ] {
                if names.len() == 1 && !names[0].trim().is_empty() {
                    assert_eq!(heading(&scene), names[0]);
                    assert!(scene.primary.is_empty());
                    assert_eq!(scene.pages(), 1);
                } else {
                    assert_eq!(heading(&scene), "Overview");
                    for (index, _) in face.subjects.iter().enumerate() {
                        // Blank wording is not a usable encounter title.
                        if !face.subjects[index].name.trim().is_empty() {
                            assert!(scene.primary.iter().any(|r| r.subject_index == Some(index)));
                        }
                    }
                }
                scene.frame().unwrap();
                scene.show_details(true);
                assert!(!scene.details.is_empty());
                assert_eq!(scene.face(), &face);
            }
        }
    }
}

fn look(night: bool, scale: u16) -> conduit_face::AdmittedVisualLook {
    let value = if night {
        conduit_face::field_instrument_night_look()
    } else {
        conduit_face::field_instrument_day_look()
    }
    .unwrap();
    conduit_face::admit_visual_look(
        &value,
        conduit_face::VisualUserConstraints {
            forced_colors: false,
            reduced_motion: true,
            minimum_text_scale_percent: scale,
            maximum_supported_text_scale_percent: 200,
        },
    )
    .unwrap()
}
