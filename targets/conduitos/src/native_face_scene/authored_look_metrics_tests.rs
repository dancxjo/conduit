//! Shared authored metric preparation; preserves exact Face, focus and actions.
use super::*;
// Ordinary typed author proposals exercise arbitrary sizes independently of
// Field resource registration; these are test values, never physical proof.
fn metric_proposal(body: u16, small: u16, leading: u16) -> conduit_face::LookInfo {
    use conduit_face::*;
    let base = engineering_look().unwrap();
    let LookVisualFacetOption::Some(visual) = base.visual() else {
        panic!("visual")
    };
    let t = visual.tokens();
    let tokens = LookVisualTokensInfo::new(
        *t.background(),
        *t.bootstrap_surface(),
        *t.emphasis(),
        *t.failure(),
        *t.focus(),
        *t.focus_width_px(),
        leading,
        *t.muted(),
        *t.radius_control_px(),
        *t.radius_panel_px(),
        *t.reading_paper(),
        *t.space_control_block_px(),
        *t.space_control_inline_px(),
        *t.space_panel_px(),
        *t.space_unit_px(),
        *t.structure_primary(),
        *t.structure_secondary(),
        *t.success(),
        *t.surface(),
        *t.text_primary(),
        *t.text_secondary(),
        body,
        small,
        *t.warning(),
        *t.workbench_canvas(),
    )
    .unwrap();
    let id = alloc::format!("test/native-metric-{body}-{small}-{leading}");
    let expression = VisualExpressionInfo::new(
        VisualDensityPreference::Compact,
        id.clone(),
        VisualMotionPreference::None,
        VisualPalettePreference::Editorial,
        100,
        VisualTypographyPreference::Engineering,
        2,
    )
    .unwrap();
    LookInfo::new(
        base.generative().clone(),
        id,
        base.spoken().clone(),
        2,
        LookVisualFacetOption::some(expression, tokens).unwrap(),
    )
    .unwrap()
}
#[test]
fn authored_look_metrics_keep_exact_face_and_measure_all_graphical_text() {
    let face = tests::face(2);
    let focus = FaceFocusRequest {
        action_id: "edit".into(),
        argument_name: Some("value".into()),
    };
    for look in [
        conduit_face::engineering_look().unwrap(),
        conduit_face::field_instrument_day_look().unwrap(),
        conduit_face::field_instrument_night_look().unwrap(),
        metric_proposal(18, 16, 150),
        metric_proposal(20, 18, 160),
    ] {
        for scale in [100, 125, 200] {
            let admitted = conduit_face::admit_visual_look(
                &look,
                conduit_face::VisualUserConstraints {
                    forced_colors: false,
                    reduced_motion: true,
                    minimum_text_scale_percent: scale,
                    maximum_supported_text_scale_percent: 200,
                },
            )
            .unwrap();
            let mut scene =
                NativeFaceScene::prepare_visual_look(face.clone(), 800, 600, true, &admitted)
                    .unwrap();
            scene.focus_named(&focus).unwrap();
            assert_eq!(scene.focused_request(), Some(focus.clone()));
            assert_eq!(scene.face(), &face);
            for _ in 0..scene.pages() {
                for command in scene.frame().unwrap().scene.commands() {
                    if command.kind == conduit_face::GraphicsCommandKind::Text {
                        let measured = TextLayout::new_selected(
                            command.payload(),
                            scene.typography.style(command.text_role().into()),
                            command.bounds.width,
                            scale,
                        )
                        .unwrap()
                        .finish_height();
                        assert!(
                            measured <= u32::from(command.bounds.height),
                            "{}",
                            command.payload()
                        );
                    }
                }
                scene.turn_page(true);
            }
            assert_eq!(scene.face(), &face);
        }
    }
}
#[test]
fn authored_look_metrics_do_not_change_control_identity_or_reading_order() {
    let face = tests::face(2);
    let mut reference = None;
    for look in [
        conduit_face::engineering_look().unwrap(),
        conduit_face::field_instrument_day_look().unwrap(),
        conduit_face::field_instrument_night_look().unwrap(),
        metric_proposal(18, 16, 150),
        metric_proposal(20, 18, 160),
    ] {
        let admitted = conduit_face::admit_visual_look(
            &look,
            conduit_face::VisualUserConstraints {
                forced_colors: false,
                reduced_motion: true,
                minimum_text_scale_percent: 100,
                maximum_supported_text_scale_percent: 200,
            },
        )
        .unwrap();
        let mut scene =
            NativeFaceScene::prepare_visual_look(face.clone(), 800, 600, true, &admitted).unwrap();
        let mut controls = alloc::vec![];
        for _ in 0..scene.pages() {
            for hit in scene.frame().unwrap().hits {
                controls.push(hit.control);
            }
            scene.turn_page(true);
        }
        assert!(!controls.is_empty());
        if let Some(old) = &reference {
            assert_eq!(&controls, old);
        }
        reference = Some(controls);
        assert_eq!(scene.face(), &face);
    }
}

#[test]
fn selected_metrics_apply_to_checked_recipe_and_mechanical_layout() {
    let face = structured_source_tests::todo();
    let structure = structured_source_tests::structure(
        conduit_face::TODO_LAYOUT_MASK_SOURCE,
        "todo-component-grouped",
        &face,
    );
    let mut reference = None;
    for look in [
        conduit_face::field_instrument_day_look().unwrap(),
        conduit_face::field_instrument_night_look().unwrap(),
        metric_proposal(18, 16, 150),
        metric_proposal(20, 18, 160),
    ] {
        for scale in [100, 200] {
            let admitted = conduit_face::admit_visual_look(
                &look,
                conduit_face::VisualUserConstraints {
                    forced_colors: false,
                    reduced_motion: true,
                    minimum_text_scale_percent: scale,
                    maximum_supported_text_scale_percent: 200,
                },
            )
            .unwrap();
            let conduit_face::LookVisualFacetOption::Some(visual) = admitted.selected.visual()
            else {
                panic!("selected visual Look");
            };
            let expected = Typography::with_look_metrics(
                admitted.visual.expression.typography,
                visual.tokens(),
                200,
            )
            .unwrap();
            let configured = NativeFaceScene::prepare_configured_look(
                face.clone(),
                800,
                600,
                true,
                structure.recipe(),
                &admitted,
            )
            .unwrap();
            let structured =
                NativeFaceScene::prepare_structured_look(&structure, 800, 600, true, &admitted)
                    .unwrap();
            for mut scene in [configured, structured] {
                assert_eq!(scene.face(), &face);
                assert_eq!(scene.typography, expected);
                assert_eq!(scene.text_scale_percent, scale);
                let mut controls = alloc::vec![];
                for _ in 0..scene.pages() {
                    let frame = scene.frame().unwrap();
                    controls.extend(frame.hits.into_iter().map(|hit| hit.control));
                    for command in frame.scene.commands() {
                        if command.kind == conduit_face::GraphicsCommandKind::Text {
                            let height = TextLayout::new_selected(
                                command.payload(),
                                expected.style(command.text_role().into()),
                                command.bounds.width,
                                scale,
                            )
                            .unwrap()
                            .finish_height();
                            assert!(
                                height <= u32::from(command.bounds.height),
                                "{}",
                                command.payload()
                            );
                        }
                    }
                    scene.turn_page(true);
                }
                assert!(!controls.is_empty());
                if let Some(previous) = &reference {
                    assert_eq!(&controls, previous);
                }
                reference = Some(controls);
            }
        }
    }
}

#[test]
fn compact_read_only_headings_use_the_admitted_chrome_vocabulary() {
    let face = tests::face(2);
    for look in [
        conduit_face::field_instrument_day_look().unwrap(),
        conduit_face::field_instrument_night_look().unwrap(),
    ] {
        for scale in [100, 125, 200] {
            let admitted = conduit_face::admit_visual_look(
                &look,
                conduit_face::VisualUserConstraints {
                    forced_colors: false,
                    reduced_motion: true,
                    minimum_text_scale_percent: scale,
                    maximum_supported_text_scale_percent: 200,
                },
            )
            .unwrap();
            let mut scene =
                NativeFaceScene::prepare_visual_look(face.clone(), 320, 600, false, &admitted)
                    .unwrap();
            scene.use_read_only_overview().unwrap();
            for details in [false, true] {
                scene.show_details(details);
                let frame = scene.frame().unwrap();
                let heading = frame
                    .scene
                    .commands()
                    .iter()
                    .find(|command| command.payload() == if details { "DETAILS" } else { "VIEW" })
                    .expect("compact heading");
                let height = TextLayout::new_selected(
                    heading.payload(),
                    scene.typography.style(heading.text_role().into()),
                    heading.bounds.width,
                    scale,
                )
                .unwrap()
                .finish_height();
                assert!(height <= u32::from(heading.bounds.height));
                assert_eq!(scene.face(), &face);
                assert!(frame.hits.is_empty());
            }
        }
    }
}
