//! Current Todo semantic projection through the unchanged generic Mask.
//! Fixture Plans are explicitly not live environmental Body admission evidence.
#[path = "../../../plots/todo/face/tests/common/face.rs"]
mod fixture;
use conduit_face::{
    FaceRole,
    presentation_preferences::{PresentationPreferences, SplitLayout, Theme},
};
#[test]
fn canonical_todo_projection_uses_generic_mask_at_zero_one_three_and_twenty_remaining() {
    let output = std::env::var_os("CONDUIT_TODO_MASK_PREVIEW").map(std::path::PathBuf::from);
    if let Some(path) = &output {
        std::fs::create_dir_all(path).unwrap();
    }
    for remaining in [0, 1, 3, 20] {
        let face = fixture::face(remaining, remaining.max(3), false);
        face.validate().unwrap();
        let original = face.clone();
        assert!(
            crate::property_mask::frame(&face, 1024, 512, 0, None).is_none(),
            "Todo must use generic scene, not Info-property shortcut"
        );
        let names: Vec<_> = face
            .subjects
            .iter()
            .filter(|s| {
                s.role == FaceRole::Item
                    && face.disclosures.iter().any(|d| {
                        d.subject == s.identity
                            && d.level == conduit_face::FaceDisclosureLevel::Primary
                    })
            })
            .map(|s| s.name.clone())
            .collect();
        let mut shown = String::new();
        for theme in [
            Theme::Navy,
            Theme::Editorial,
            Theme::Engineering,
            Theme::FieldDay,
            Theme::FieldNight,
        ] {
            let look = theme.look().unwrap();
            let (_, _, pages, _, _) =
                crate::view::frame_with_look(face.clone(), 1024, 512, 0, None, false, Some(&look))
                    .unwrap();
            assert!(pages > 0 && pages <= 64);
            for page in 0..pages {
                let (scene, _, _, _, _) = crate::view::frame_with_look(
                    face.clone(),
                    1024,
                    512,
                    page,
                    None,
                    false,
                    Some(&look),
                )
                .unwrap();
                for c in scene.commands() {
                    shown.push_str(c.payload());
                    shown.push('\n');
                }
                let mut pixels = vec![0; 1024 * 600];
                crate::view::render_with_palette(
                    face.clone(),
                    1024,
                    512,
                    page,
                    &mut pixels[..1024 * 512],
                    None,
                    Some(&look),
                )
                .unwrap();
                let preferences = PresentationPreferences {
                    theme,
                    split: SplitLayout::CollapsedRight,
                    ..Default::default()
                };
                crate::mask_controls::chrome_in_context(
                    1024,
                    600,
                    &mut pixels,
                    preferences,
                    &look,
                    crate::mask_controls::Context::for_face(&face, false),
                )
                .unwrap();
                crate::display_clock::render(1024, 600, 0, &mut pixels).unwrap();
                if let Some(output) = &output {
                    let mut ppm = b"P6\n1024 600\n255\n".to_vec();
                    for pixel in pixels {
                        ppm.extend_from_slice(&[
                            (pixel >> 16) as u8,
                            (pixel >> 8) as u8,
                            pixel as u8,
                        ]);
                    }
                    std::fs::write(
                        output.join(format!("todo-{remaining}-{}-{page}.ppm", theme.label())),
                        ppm,
                    )
                    .unwrap();
                }
            }
        }
        assert!(!shown.contains("todo-return-route-unavailable"));
        assert!(!shown.contains("OWNER FACE"));
        assert!(!shown.contains("[ ]"));
        if remaining <= 3 {
            assert_eq!(
                crate::view::frame(face.clone(), 1024, 512, 0, None)
                    .unwrap()
                    .2,
                1
            );
        }
        for name in names {
            assert!(
                shown.contains(&name),
                "missing primary Todo item {name}; shown={shown}"
            );
        }
        assert_eq!(face, original);
        if let Some(output) = &output {
            std::fs::write(
                output.join(format!("todo-{remaining}-face.json")),
                serde_json::to_vec_pretty(&face).unwrap(),
            )
            .unwrap();
        }
        println!(
            "TODO_MASK_FIXTURE remaining={remaining} identity={} revision={} unchanged=true body_admission=false",
            face.identity.as_str(),
            face.revision
        );
    }
}

/// Explicitly selected captured Face: compatibility proof, not a live MCU receipt.
#[test]
#[ignore = "requires an explicitly selected private captured environmental Face"]
fn exact_environmental_face_is_consumed_by_hdmi_and_thermometer_masks() {
    use conduit_seven_segment_mask::*;
    let path = std::env::var_os("CONDUIT_SHARED_FACE_INPUT").expect("captured Face input");
    let document: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let face: conduit_face::Face = serde_json::from_value(
        document
            .get("face")
            .expect("captured display envelope")
            .clone(),
    )
    .unwrap();
    face.validate().unwrap();
    let original = face.clone();
    let selected = PreparedEnvironmentFace::prepare(&face, "environment").unwrap();
    let profile = Tm1637Profile {
        digits: core::array::from_fn(|n| DigitWiring {
            address: n as u8,
            segment_bits: [0, 1, 2, 3, 4, 5, 6],
        }),
        decimal_points: core::array::from_fn(|n| {
            Some(WireBit {
                address: n as u8,
                bit: 7,
            })
        }),
        colon: Some(WireBit { address: 1, bit: 7 }),
    };
    let mask = TimedSevenSegmentMask::prepare(
        SevenSegmentLook::fahrenheit().with_colon_fraction(),
        3000,
        6000,
    )
    .unwrap();
    let prepared = mask.prepare_show(&selected, &profile).unwrap();
    assert_eq!(prepared.face_content_id(), face.identity.as_str());
    assert_eq!(prepared.face_revision(), face.revision);
    for tick in [0, 3000, 5999] {
        let show = prepared.at(tick);
        assert_eq!(show.face_content_id, face.identity.as_str());
        assert_eq!(show.face_revision, face.revision);
    }
    let mut pixels = vec![0; 1024 * 512];
    crate::view::render_with_palette(face.clone(), 1024, 512, 0, &mut pixels, None, None).unwrap();
    assert_eq!(face, original);
    println!(
        "SHARED_FACE_COMPATIBILITY identity={} revision={} hdmi=true tm1637=true deployed_mcu_receipt=false",
        face.identity.as_str(),
        face.revision
    );
}

#[test]
fn details_retains_completed_items_and_exact_unavailable_reasons() {
    let face = fixture::face(1, 3, false);
    let mut text = String::new();
    let pages = crate::view::frame_with_details(face.clone(), 1024, 512, 0, None, true)
        .unwrap()
        .2;
    for page in 0..pages {
        let (scene, ..) =
            crate::view::frame_with_details(face.clone(), 1024, 512, page, None, true).unwrap();
        for command in scene.commands() {
            text.push_str(command.payload());
            text.push('\n');
        }
    }
    assert!(text.contains("Task 2"));
    assert!(text.contains("Task 3"));
    assert!(text.contains("todo-return-route-unavailable"));
    assert!(text.contains("complete"));
}
#[test]
fn todo_boolean_contract_exposes_no_return_route_in_observer() {
    let face = fixture::face(1, 3, false);
    let original = face.clone();
    let mut scene =
        conduitos::native_face_scene::NativeFaceScene::prepare_read_only(face, 1024, 512).unwrap();
    assert!(scene.focus_next(true).is_none());
    assert!(scene.frame().unwrap().hits.is_empty());
    scene.show_details(true);
    assert_eq!(scene.face(), &original);
}

#[test]
fn admitted_todo_preserves_the_current_sources_actual_completion_contract() {
    let face = fixture::face(1, 3, true);
    let expected = face
        .actions
        .iter()
        .find(|a| a.identity == "todo.complete.task-1")
        .expect("current dev Todo completion action");
    assert!(expected.arguments.is_empty());
    // A visual Look must not manufacture the Boolean return contract from a
    // newer, unmerged Todo producer. Preserve current Source and Face exactly.
    for look in [
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
        let mut scene = conduitos::native_face_scene::NativeFaceScene::prepare_visual_fallback(
            face.clone(),
            1024,
            512,
            true,
            &admitted.visual,
        )
        .unwrap();
        let mut found = false;
        for _ in 0..scene.pages() {
            let frame = scene.frame().unwrap();
            for hit in &frame.hits {
                if hit.action.identity == expected.identity {
                    assert_eq!(hit.action, expected);
                    found = true;
                }
            }
            assert_eq!(frame.face_id, &face.identity);
            assert_eq!(frame.revision, face.revision);
            scene.turn_page(true);
        }
        assert!(found, "actual completion contract remains reachable");
        assert_eq!(scene.face(), &face);
    }
}
