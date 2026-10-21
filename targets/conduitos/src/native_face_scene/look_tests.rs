//! Look geometry preserves Face authority and complete reachable controls.
use super::*;
use conduit_face::{
    FaceActionAvailability, FaceDisclosureLevel, VisualDensity, VisualExpression, VisualPalette,
    VisualUserConstraints, admit_visual_expression,
};

fn primary_face(available: bool) -> Face {
    let mut face = tests::face(7);
    face.actions[1].name = "An arbitrary source-owned primary action".into();
    face.actions[1].disclosure = FaceDisclosureLevel::Primary;
    if !available {
        face.actions[1].availability = FaceActionAvailability::Unavailable {
            reason_code: "test/unavailable".into(),
            explanation: "Needs owner approval".into(),
        };
    }
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
fn primary_fill_follows_disclosure_and_availability_without_changing_face_or_hits() {
    for available in [false, true] {
        let face = primary_face(available);
        for width in [320, 1280] {
            for scale in [100, 200] {
                for density in [VisualDensity::Compact, VisualDensity::Spacious] {
                    for palette in [VisualPalette::Phosphor, VisualPalette::Editorial] {
                        let mut requested = VisualExpression::phosphor();
                        requested.density = density;
                        requested.palette = palette;
                        requested.text_scale_percent = scale;
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
                        let mut scene = NativeFaceScene::prepare_visual_fallback(
                            face.clone(),
                            width,
                            800,
                            true,
                            &expression,
                        )
                        .unwrap();
                        assert_eq!(scene.face(), &face);
                        let mut found = false;
                        let mut primary_hits = 0;
                        for page in 0..scene.pages() {
                            scene.page = page;
                            let frame = scene.frame().unwrap();
                            assert!(
                                frame.scene.commands().len() <= conduit_face::MAX_GRAPHICS_COMMANDS
                            );
                            primary_hits += frame
                                .hits
                                .iter()
                                .filter(|hit| hit.control.action == 1)
                                .count();
                            for row in scene.rows().iter().filter(|row| {
                                row.page == page
                                    && row.control.is_some_and(|control| control.action == 1)
                            }) {
                                found = true;
                                assert_eq!(row.emphasized(), available);
                                let filled = frame.scene.commands().iter().any(|command| {
                                    command.bounds == row.bounds
                                        && command.style == GraphicsShapeStyle::RoundedFill
                                        && command.paint == GraphicsPaintRole::Accent
                                });
                                assert_eq!(filled, available);
                                assert!(frame.scene.commands().iter().any(|command| command.kind
                                    == conduit_face::GraphicsCommandKind::Text
                                    && command.bounds == row.text_bounds()
                                    && command.paint
                                        == if available {
                                            GraphicsPaintRole::Background
                                        } else {
                                            GraphicsPaintRole::Muted
                                        }));
                            }
                        }
                        assert!(found);
                        assert_eq!(primary_hits > 0, available);
                    }
                }
            }
        }
    }
}
