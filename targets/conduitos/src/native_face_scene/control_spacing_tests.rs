//! Selected control insets remain expression, independent of input authority.
use super::*;
use conduit_face::{FaceActionAvailability, VisualUserConstraints, admit_visual_look};

#[test]
fn control_insets_reach_all_layouts_even_when_read_only_or_unavailable() {
    let original = structured_source_tests::todo();
    for available in [true, false] {
        let mut face = original.clone();
        if !available {
            for action in &mut face.actions {
                action.availability = FaceActionAvailability::Unavailable {
                    reason_code: "test/offline".into(),
                    explanation: "Source unavailable".into(),
                };
            }
            face = Face::new_with_semantics(
                face.revision,
                face.basis,
                face.subjects,
                face.relationships,
                face.properties,
                face.text,
                face.actions,
                face.disclosures,
            )
            .unwrap();
        }
        let structure = structured_source_tests::structure(
            conduit_face::TODO_LAYOUT_MASK_SOURCE,
            "todo-component-grouped",
            &face,
        );
        for (inline, block, scale) in [
            (0, 0, 100),
            (12, 10, 100),
            (64, 64, 100),
            (0, 0, 200),
            (12, 10, 200),
            (64, 64, 200),
        ] {
            let look = authored_look_metrics_tests::metric_proposal_with_insets(
                18, 16, 150, 4, inline, block,
            );
            let admitted = admit_visual_look(
                &look,
                VisualUserConstraints {
                    forced_colors: false,
                    reduced_motion: true,
                    minimum_text_scale_percent: scale,
                    maximum_supported_text_scale_percent: 200,
                },
            )
            .unwrap();
            for input in [true, false] {
                let (items, _) = document::prepare(&face, input).unwrap();
                let controls: Vec<_> = items
                    .iter()
                    .filter(|item| item.part == layout::NativePart::Control)
                    .collect();
                assert!(!controls.is_empty());
                for mut scene in [
                    NativeFaceScene::prepare_visual_look(face.clone(), 1024, 600, input, &admitted)
                        .unwrap(),
                    NativeFaceScene::prepare_configured_look(
                        face.clone(),
                        1024,
                        600,
                        input,
                        structure.recipe(),
                        &admitted,
                    )
                    .unwrap(),
                    NativeFaceScene::prepare_structured_look(
                        &structure, 1024, 600, input, &admitted,
                    )
                    .unwrap(),
                ] {
                    for item in &controls {
                        let start = scene
                            .primary
                            .iter()
                            .position(|row| {
                                row.subject_index == item.subject_index
                                    && !row.text.is_empty()
                                    && item.text.starts_with(&row.text)
                            })
                            .unwrap();
                        let mut remaining = item.text.as_str();
                        for row in &scene.primary[start..] {
                            assert_eq!(row.subject_index, item.subject_index);
                            assert!(remaining.starts_with(&row.text));
                            assert_eq!(row.insets.inline(), inline);
                            assert_eq!(row.insets.block(), block);
                            assert_eq!(row.insets.leading(), 8);
                            let text = row.text_bounds();
                            assert_eq!(text.y, row.bounds.y + block as i16);
                            assert!(text.x >= row.bounds.x + inline as i16 + 8);
                            assert_eq!(text.height + block * 2, row.bounds.height);
                            remaining = &remaining[row.text.len()..];
                            if remaining.is_empty() {
                                break;
                            }
                        }
                        assert!(
                            remaining.is_empty(),
                            "control content must remain reachable: {}",
                            item.text
                        );
                    }
                    if input && available {
                        let focused = scene.focus_next(true).unwrap();
                        let row = scene
                            .primary
                            .iter()
                            .find(|row| row.control == Some(focused))
                            .unwrap();
                        let marker = LayoutRect {
                            x: row.bounds.x + 3,
                            y: row.bounds.y + 3,
                            width: 2,
                            height: row.bounds.height - 6,
                        };
                        assert!(marker.x + marker.width as i16 <= row.text_bounds().x);
                        let frame = scene.frame().unwrap();
                        assert!(frame.scene.commands().iter().any(|command| command.kind
                            == conduit_face::GraphicsCommandKind::Rect
                            && command.bounds == marker
                            && command.style == GraphicsShapeStyle::Fill));
                    }
                    let mut hits = 0;
                    for _ in 0..scene.pages() {
                        let frame = scene.frame().unwrap();
                        assert!(
                            frame.scene.commands().len() <= conduit_face::MAX_GRAPHICS_COMMANDS
                        );
                        for command in frame.scene.commands() {
                            assert_eq!(
                                command.clip_class(),
                                conduit_face::GraphicsClipClass::FullyVisible
                            );
                        }
                        for hit in &frame.hits {
                            let resolved = frame
                                .hit_test(
                                    hit.bounds.x + (hit.bounds.width / 2) as i16,
                                    hit.bounds.y + (hit.bounds.height / 2) as i16,
                                )
                                .unwrap();
                            assert_eq!(resolved.control, hit.control);
                            hits += 1;
                        }
                        scene.turn_page(true);
                    }
                    assert_eq!(hits > 0, input && available);
                    assert_eq!(scene.face(), &face);
                }
            }
        }
    }
}
