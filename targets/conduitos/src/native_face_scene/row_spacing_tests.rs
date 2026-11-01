//! The selected finite Look rhythm drives measurement, placement and exact hits.
use super::*;
use alloc::collections::BTreeSet;
use conduit_face::{VisualUserConstraints, admit_visual_look};

#[test]
fn selected_row_gap_is_exact_across_generic_configured_and_checked_layouts() {
    let face = structured_source_tests::todo();
    let structure = structured_source_tests::structure(
        conduit_face::TODO_LAYOUT_MASK_SOURCE,
        "todo-component-grouped",
        &face,
    );
    for scale in [100, 200] {
        let mut reference = None;
        for gap in [0, 4, 32] {
            let look = authored_look_metrics_tests::metric_proposal_with_gap(18, 16, 150, gap);
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
            let mut seen = Vec::new();
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
                let mut exact_pairs = 0;
                for pair in scene.primary.windows(2) {
                    let (a, b) = (&pair[0], &pair[1]);
                    if a.page == b.page
                        && a.subject_index.is_some()
                        && a.subject_index == b.subject_index
                        && a.bounds.x == b.bounds.x
                        && a.bounds.width == b.bounds.width
                    {
                        assert_eq!(b.bounds.y - a.bounds.y - a.bounds.height as i16, gap as i16);
                        exact_pairs += 1;
                    }
                }
                assert!(
                    exact_pairs > 0,
                    "must exercise within-component row placement"
                );
                let mut controls = BTreeSet::new();
                for _ in 0..scene.pages() {
                    let frame = scene.frame().unwrap();
                    assert!(frame.scene.commands().len() <= conduit_face::MAX_GRAPHICS_COMMANDS);
                    for command in frame.scene.commands() {
                        assert_eq!(
                            command.clip_class(),
                            conduit_face::GraphicsClipClass::FullyVisible
                        );
                    }
                    for hit in &frame.hits {
                        let control = frame
                            .hit_test(
                                hit.bounds.x + (hit.bounds.width / 2) as i16,
                                hit.bounds.y + (hit.bounds.height / 2) as i16,
                            )
                            .unwrap()
                            .control;
                        assert_eq!(control, hit.control);
                        controls.insert((control.action, control.argument));
                    }
                    scene.turn_page(true);
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
