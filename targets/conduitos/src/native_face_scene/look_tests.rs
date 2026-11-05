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

#[test]
fn refused_and_host_incompatible_looks_cannot_remove_todo_controls() {
    use conduit_face::{
        LookInfo, LookVisualFacetOption, VisualDensityPreference, VisualExpressionInfo,
        VisualMotionPreference, VisualPalettePreference, VisualTypographyPreference,
        admit_visual_look, phosphor_look, render_linear_face,
    };
    let face = structured_source_tests::todo();
    let original = face.clone();
    let structure = structured_source_tests::structure(
        conduit_face::TODO_LAYOUT_MASK_SOURCE,
        "todo-component-grouped",
        &face,
    );
    let default = phosphor_look().unwrap();
    let LookVisualFacetOption::Some(facet) = default.visual() else {
        panic!("visual")
    };
    // A foreign resource identity cannot relabel the default's visual facet.
    assert!(
        LookInfo::new(
            default.generative().clone(),
            "fixture/forged@1".into(),
            default.spoken().clone(),
            2,
            default.visual().clone()
        )
        .is_err()
    );
    let identity = "fixture/large-look@1";
    let large = LookInfo::new(
        default.generative().clone(),
        identity.into(),
        default.spoken().clone(),
        2,
        LookVisualFacetOption::some(
            VisualExpressionInfo::new(
                VisualDensityPreference::Spacious,
                identity.into(),
                VisualMotionPreference::Restrained,
                VisualPalettePreference::Editorial,
                200,
                VisualTypographyPreference::Editorial,
                2,
            )
            .unwrap(),
            facet.tokens().clone(),
        )
        .unwrap(),
    )
    .unwrap();
    let user = VisualUserConstraints {
        forced_colors: false,
        reduced_motion: true,
        minimum_text_scale_percent: 100,
        maximum_supported_text_scale_percent: 125,
    };
    let fallback = admit_visual_look(&large, user).unwrap();
    assert_eq!(fallback.requested, large);
    assert_eq!(fallback.selected, default);
    assert!(matches!(
        fallback.visual.outcome,
        conduit_face::VisualExpressionOutcome::PhosphorFallback {
            refusal: conduit_face::VisualExpressionRefusal::HostTextCapacity
        }
    ));
    assert!(
        admit_visual_look(
            &large,
            VisualUserConstraints {
                minimum_text_scale_percent: 150,
                ..user
            }
        )
        .is_err()
    );
    let mut baseline = None;
    for look in [admit_visual_look(&default, user).unwrap(), fallback] {
        let mut scene =
            NativeFaceScene::prepare_structured_look(&structure, 1024, 600, true, &look).unwrap();
        assert_eq!(scene.face(), &original);
        assert_eq!(
            render_linear_face(scene.face()).unwrap().lines,
            render_linear_face(&original).unwrap().lines
        );
        let mut orders = Vec::new();
        for details in [false, true] {
            scene.showing_details = details;
            scene.focus = None;
            scene.page = 0;
            let mut expected = Vec::new();
            for row in if details {
                &scene.details
            } else {
                &scene.primary
            } {
                if let Some(control) = row.control
                    && original.actions[control.action].availability.is_available()
                    && !expected.contains(&control)
                {
                    expected.push(control);
                }
            }
            assert!(!expected.is_empty());
            let mut reached = Vec::new();
            for _ in 0..expected.len() {
                reached.push(scene.focus_next(true).unwrap());
                scene.frame().unwrap();
            }
            assert_eq!(reached, expected);
            assert_eq!(scene.focus_next(true), expected.first().copied());
            orders.push(reached);
        }
        if let Some(prior) = &baseline {
            assert_eq!(&orders, prior);
        } else {
            baseline = Some(orders);
        }
    }
    assert_eq!(face, original);
}
