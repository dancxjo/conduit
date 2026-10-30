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
        for theme in [Theme::Navy, Theme::Editorial, Theme::Engineering] {
            let look = theme.look().unwrap();
            let (_, _, pages, _, _) = crate::view::frame(face.clone(), 1024, 512, 0, None).unwrap();
            assert!(pages > 0 && pages <= 64);
            for page in 0..pages {
                let (scene, _, _, _, _) =
                    crate::view::frame(face.clone(), 1024, 512, page, None).unwrap();
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
                crate::mask_controls::chrome(1024, 600, &mut pixels, preferences, &look).unwrap();
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
