//! Authored focus markers share the admitted control gutter and exact action hits.
use super::*;
use conduit_face::{VisualUserConstraints, admit_visual_look};

#[test]
fn every_authored_focus_width_fits_without_moving_text_or_action_hits() {
    let face = structured_source_tests::todo();
    let structure = structured_source_tests::structure(
        conduit_face::TODO_LAYOUT_MASK_SOURCE,
        "todo-component-grouped",
        &face,
    );
    for scale in [100, 200] {
        let mut reference = None;
        for width in 1..=8 {
            let look = authored_look_metrics_tests::metric_proposal_with_focus(
                18,
                16,
                150,
                4,
                (0, 0, 12),
                width,
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
            let mut geometries = Vec::new();
            for mut scene in [
                NativeFaceScene::prepare_visual_look(face.clone(), 1024, 600, true, &admitted)
                    .unwrap(),
                NativeFaceScene::prepare_configured_look(
                    face.clone(),
                    1024,
                    600,
                    true,
                    structure.recipe(),
                    &admitted,
                )
                .unwrap(),
                NativeFaceScene::prepare_structured_look(&structure, 1024, 600, true, &admitted)
                    .unwrap(),
            ] {
                let control = scene.focus_next(true).unwrap();
                let row = scene
                    .primary
                    .iter()
                    .find(|row| row.control == Some(control))
                    .unwrap();
                let marker = LayoutRect {
                    x: row.bounds.x + ((8 - width) / 2) as i16,
                    y: row.bounds.y + 3,
                    width,
                    height: row.bounds.height - 6,
                };
                assert!(marker.x >= row.bounds.x);
                assert!(i32::from(marker.x) + i32::from(width) <= i32::from(row.text_bounds().x));
                let frame = scene.frame().unwrap();
                assert!(frame.scene.commands().iter().any(|command| command.kind
                    == conduit_face::GraphicsCommandKind::Rect
                    && command.bounds == marker
                    && command.style == GraphicsShapeStyle::Fill));
                assert!(frame.scene.commands().len() <= conduit_face::MAX_GRAPHICS_COMMANDS);
                assert_eq!(
                    frame.hit_test(row.bounds.x, row.bounds.y).unwrap().control,
                    control
                );
                geometries.push((
                    row.bounds,
                    row.text_bounds(),
                    frame.scene.commands().len(),
                    control,
                ));
                assert_eq!(scene.face(), &face);
            }
            if let Some(previous) = &reference {
                assert_eq!(previous, &geometries);
            }
            reference = Some(geometries);
        }
    }
}

#[test]
fn legacy_expression_keeps_its_existing_focus_marker() {
    let face = structured_source_tests::todo();
    let scene = NativeFaceScene::prepare_with_input(face, 1024, 600, true).unwrap();
    assert_eq!(scene.focus_marker_width, 2);
}
