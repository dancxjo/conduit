//! Exact selected panel insets over explicit checked regions and continuations.
use super::*;
use alloc::collections::BTreeSet;
use conduit_face::{VisualUserConstraints, admit_visual_look};

#[test]
fn selected_panel_padding_keeps_all_regions_controls_and_empty_region_behavior() {
    for count in [0, 3, 20] {
        let face = structured_source_tests::todo_with_items(count);
        let structure = structured_source_tests::structure(
            conduit_face::TODO_LAYOUT_MASK_SOURCE,
            "todo-component-grouped",
            &face,
        );
        for scale in [100, 200] {
            let mut reference = None;
            let mut generic_reference = None;
            for panel in [0, 16, 64] {
                let look = authored_look_metrics_tests::metric_proposal_with_panel(
                    18, 16, 150, 4, 12, 10, panel,
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
                let generic =
                    NativeFaceScene::prepare_visual_look(face.clone(), 1024, 600, true, &admitted)
                        .unwrap();
                let generic_geometry: Vec<_> = generic
                    .primary
                    .iter()
                    .map(|row| (row.bounds, row.region_inset, row.panel_bounds))
                    .collect();
                assert!(
                    generic
                        .primary
                        .iter()
                        .all(|row| row.region.is_none() && row.panel_bounds.is_none())
                );
                if let Some(expected) = &generic_reference {
                    assert_eq!(&generic_geometry, expected);
                }
                generic_reference = Some(generic_geometry);
                let configured = NativeFaceScene::prepare_configured_look(
                    face.clone(),
                    1024,
                    600,
                    true,
                    structure.recipe(),
                    &admitted,
                )
                .unwrap();
                assert!(
                    configured
                        .primary
                        .iter()
                        .any(|row| row.panel_bounds.is_some())
                );
                for row in &configured.primary {
                    if let Some(bounds) = row.panel_bounds {
                        assert_eq!(row.bounds.y - bounds.y, panel as i16);
                        assert!(row.bounds.x >= bounds.x + panel as i16);
                    }
                    if row.region == Some(2) {
                        assert_eq!(row.region_inset, panel);
                    }
                }
                let mechanical = NativeFaceScene::prepare_structured_look(
                    &structure, 1024, 600, true, &admitted,
                )
                .unwrap();
                assert!(
                    mechanical
                        .primary
                        .iter()
                        .filter(|row| row.region.is_some())
                        .all(|row| row.region_inset >= panel)
                );
                let mut seen = Vec::new();
                for (mode, mut scene) in [configured, mechanical].into_iter().enumerate() {
                    if count == 0 {
                        assert!(scene.primary.iter().all(|row| row.region != Some(1)));
                    }
                    let mut controls = BTreeSet::new();
                    let mut text = String::new();
                    for details in [false, true] {
                        scene.show_details(details);
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
                                if command.kind == conduit_face::GraphicsCommandKind::Text {
                                    text.push_str(command.payload());
                                }
                            }
                            for hit in &frame.hits {
                                assert_eq!(
                                    frame
                                        .hit_test(
                                            hit.bounds.x + (hit.bounds.width / 2) as i16,
                                            hit.bounds.y + (hit.bounds.height / 2) as i16
                                        )
                                        .unwrap()
                                        .control,
                                    hit.control
                                );
                                controls.insert((hit.control.action, hit.control.argument));
                            }
                            scene.turn_page(true);
                        }
                    }
                    for id in 1..=count {
                        assert!(
                            text.contains(&alloc::format!("Élan task {id}")),
                            "count={count} scale={scale} panel={panel} mode={mode} missing={id} text={text:?}"
                        );
                    }
                    assert!(!controls.is_empty());
                    assert_eq!(scene.face(), &face);
                    seen.push(controls);
                }
                if let Some(expected) = &reference {
                    assert_eq!(&seen, expected);
                }
                reference = Some(seen);
            }
        }
    }
}

#[test]
fn selected_panel_padding_reaches_single_group_and_responsive_single_column() {
    let face = structured_source_tests::todo_with_items(20);
    for source in ["todo-component-flat", "todo-component-grouped"] {
        let structure = structured_source_tests::structure(
            conduit_face::TODO_LAYOUT_MASK_SOURCE,
            source,
            &face,
        );
        for width in [480, 1024] {
            // The flat recipe remains one group at wide extents; the grouped
            // recipe must become genuine full-width panels below its breakpoint.
            let mut reference = None;
            for panel in [0, 16, 64] {
                let look = authored_look_metrics_tests::metric_proposal_with_panel(
                    18, 16, 150, 4, 12, 10, panel,
                );
                let admitted = admit_visual_look(
                    &look,
                    VisualUserConstraints {
                        forced_colors: false,
                        reduced_motion: true,
                        minimum_text_scale_percent: 100,
                        maximum_supported_text_scale_percent: 200,
                    },
                )
                .unwrap();
                let mut scene = NativeFaceScene::prepare_configured_look(
                    face.clone(),
                    width,
                    600,
                    true,
                    structure.recipe(),
                    &admitted,
                )
                .unwrap();
                if width == 480 || source == "todo-component-flat" {
                    assert!(scene.primary.iter().all(|row| row.panel_bounds.is_none()));
                    assert!(
                        scene
                            .primary
                            .iter()
                            .filter(|row| row.region.is_some())
                            .all(|row| row.region_inset == panel && row.bounds.x >= panel as i16)
                    );
                }
                let mut controls = BTreeSet::new();
                let mut text = String::new();
                for details in [false, true] {
                    scene.show_details(details);
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
                            if command.kind == conduit_face::GraphicsCommandKind::Text {
                                text.push_str(command.payload());
                            }
                        }
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
                for id in 1..=20 {
                    assert!(text.contains(&alloc::format!("Élan task {id}")));
                }
                assert!(!controls.is_empty());
                if let Some(expected) = &reference {
                    assert_eq!(&controls, expected);
                }
                reference = Some(controls);
                assert_eq!(scene.face(), &face);
            }
        }
    }
}
