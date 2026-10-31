//! Presentation/gesture contracts; never fixtures for Body actions or source identity.
use conduit_face::{
    FaceProperty, FacePropertyValue, FaceRole,
    presentation_preferences::{PresentationPreferences, SplitLayout, Theme},
};
#[test]
fn one_subject_is_one_fieldgroup_at_actual_report_widths() {
    let mut face = crate::view::unavailable("unused");
    face.subjects[0].role = FaceRole::Info;
    face.subjects[0].name = "Room conditions".into();
    face.text.clear();
    for name in ["temperature", "humidity"] {
        face.properties.push(FaceProperty {
            subject: face.subjects[0].identity.clone(),
            name: name.into(),
            value: FacePropertyValue::Text("20".into()),
        });
    }
    let original = face.clone();
    for width in [240, 368, 692] {
        let (scene, _, pages) =
            crate::property_mask::frame(&face, width, 512, 0, None).expect("fieldgroup fits");
        assert_eq!(pages, 1);
        assert_eq!(
            scene
                .commands()
                .iter()
                .filter(|c| c.payload() == "Room conditions")
                .count(),
            1
        );
        for label in ["temperature", "humidity"] {
            assert!(
                scene
                    .commands()
                    .iter()
                    .any(|c| c.payload().starts_with(&format!("{label}:")))
            );
        }
        assert!(
            !scene
                .commands()
                .iter()
                .any(|c| c.payload().contains("Room conditions /"))
        );
    }
    assert_eq!(face, original);
}
#[test]
fn resized_columns_and_all_theme_controls_fit_actual_hdmi_extent() {
    for requested in [320, 368, 692] {
        let columns = crate::columns::Columns::at_divider(1024, 512, requested).unwrap();
        assert_eq!(columns.report_width, requested);
        assert_eq!(columns.history_width, 1024 - requested - 12);
        for theme in [
            Theme::Navy,
            Theme::Editorial,
            Theme::Engineering,
            Theme::FieldDay,
            Theme::FieldNight,
        ] {
            let p = PresentationPreferences {
                theme,
                temperature: conduit_face::presentation_preferences::TemperatureUnit::Fahrenheit,
                split: SplitLayout::Split {
                    left_per_mille: (u32::from(requested) * 1000 / 1024) as u16,
                },
                ..Default::default()
            };
            let look = theme.look().unwrap();
            let mut pixels = vec![0x123456; 1024 * 600];
            crate::mask_controls::chrome(1024, 600, &mut pixels, p, &look).unwrap();
            assert!(
                pixels[512 * 1024..568 * 1024]
                    .iter()
                    .any(|pixel| *pixel != 0x123456)
            );
            assert!(
                pixels[568 * 1024..].iter().all(|pixel| *pixel == 0x123456),
                "controls never overwrite clock"
            );
        }
    }
}
#[test]
#[ignore = "requires explicit private same-Body retained Face/history, writes local preview images"]
fn actual_preferences_preview_preserves_face() {
    let folder = std::path::PathBuf::from(
        std::env::var_os("CONDUIT_PRIVATE_COLUMNS_PREVIEW").expect("private input/output folder"),
    );
    let document: serde_json::Value =
        serde_json::from_slice(&std::fs::read(folder.join("current-face.json")).unwrap()).unwrap();
    let face: conduit_face::Face = serde_json::from_value(document["face"].clone()).unwrap();
    face.validate().unwrap();
    let original = face.clone();
    for requested in [320, 368, 692] {
        for theme in [
            Theme::Navy,
            Theme::Editorial,
            Theme::Engineering,
            Theme::FieldDay,
            Theme::FieldNight,
        ] {
            let mut columns = crate::columns::Columns::at_divider(1024, 512, requested).unwrap();
            let look = theme.look().unwrap();
            let mut history = crate::history_input::HistoryInput::new_at_size(
                Some(conduit_core::Unit::Fahrenheit),
                columns.history_width,
                476,
            );
            history
                .refresh(&folder.join("retained-history.json"))
                .unwrap();
            let (_, _, pages) = crate::property_mask::frame_with_look(
                &face,
                requested,
                512,
                0,
                Some(conduit_core::Unit::Fahrenheit),
                Some(&look),
            )
            .expect("actual report fits");
            assert_eq!(pages, 1, "actual report stays visible");
            let mut pixels = vec![0; 1024 * 600];
            columns
                .render_with_palette(
                    face.clone(),
                    history.scene(),
                    0,
                    crate::columns::Presentation {
                        details: false,
                        retained_snapshot: false,
                        history_in_primary: true,
                        unit: Some(conduit_core::Unit::Fahrenheit),
                        look: Some(&look),
                    },
                    crate::columns::Notices {
                        report: None,
                        history: history.label().unwrap(),
                    },
                    &mut pixels[..1024 * 512],
                )
                .unwrap();
            let p = PresentationPreferences {
                theme,
                temperature: conduit_face::presentation_preferences::TemperatureUnit::Fahrenheit,
                split: SplitLayout::Split {
                    left_per_mille: (u32::from(requested) * 1000 / 1024) as u16,
                },
                ..Default::default()
            };
            crate::mask_controls::chrome(1024, 600, &mut pixels, p, &look).unwrap();
            crate::display_clock::render(1024, 600, 0, &mut pixels).unwrap();
            let mut image = b"P6\n1024 600\n255\n".to_vec();
            for pixel in pixels {
                image.extend_from_slice(&[(pixel >> 16) as u8, (pixel >> 8) as u8, pixel as u8]);
            }
            std::fs::write(
                folder.join(format!("interactive-{}-{requested}.ppm", theme.label())),
                image,
            )
            .unwrap();
        }
    }
    assert_eq!(face, original);
}
