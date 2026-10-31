//! Shared authored metric preparation; preserves exact Face, focus and actions.
use super::*;
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
