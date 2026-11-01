//! Placement is supplied by the shared Face, not inferred from a Host or network.
use conduit_face::{Face, FaceProperty, FacePropertyValue as V, FaceRole, FaceSubject};
fn face(placement: Option<&str>) -> Face {
    let mut f = crate::view::unavailable("unused");
    f.text.clear();
    let body = "a".repeat(64);
    f.basis.body_id = Some(serde_json::from_value(serde_json::json!(body)).unwrap());
    f.basis.wake_id = Some(serde_json::from_value(serde_json::json!("b".repeat(64))).unwrap());
    f.subjects = vec![
        FaceSubject {
            identity: format!("body/{body}"),
            role: FaceRole::Body,
            name: "My environmental Body".into(),
        },
        FaceSubject {
            identity: "environment".into(),
            role: FaceRole::Info,
            name: "Room conditions".into(),
        },
    ];
    let mut properties: Vec<_> = (0..9)
        .map(|i| FaceProperty {
            subject: "environment".into(),
            name: format!("status-{i}"),
            value: V::Text("available".into()),
        })
        .collect();
    if let Some(text) = placement {
        properties.push(FaceProperty {
            subject: "environment".into(),
            name: "placement".into(),
            value: V::Text(text.into()),
        });
    }
    Face::new(1, f.basis, f.subjects, vec![], properties, vec![]).unwrap()
}
#[test]
fn placement_wraps_between_body_and_fieldgroup_once_per_page_in_both_looks() {
    let place = "The upper room beside the west facing windows and the bookcase";
    let f = face(Some(place));
    let original = f.clone();
    for look in [
        conduit_face::field_instrument_day_look().unwrap(),
        conduit_face::field_instrument_night_look().unwrap(),
    ] {
        for width in [320, 500] {
            let (_, _, pages) =
                crate::property_mask::frame_with_look(&f, width, 476, 0, None, Some(&look))
                    .unwrap();
            assert!(pages > 1);
            for page in 0..pages {
                let (scene, _, _) =
                    crate::property_mask::frame_with_look(&f, width, 476, page, None, Some(&look))
                        .unwrap();
                let body = scene
                    .commands()
                    .iter()
                    .find(|c| c.payload() == "My environmental Body")
                    .unwrap();
                let placement = scene
                    .commands()
                    .iter()
                    .find(|c| c.payload() == place)
                    .unwrap();
                let group = scene
                    .commands()
                    .iter()
                    .find(|c| c.payload() == "Room conditions")
                    .unwrap();
                assert!(placement.bounds.y >= body.bounds.y + body.bounds.height as i16);
                assert!(group.bounds.y >= placement.bounds.y + placement.bounds.height as i16);
                assert_eq!(
                    scene
                        .commands()
                        .iter()
                        .filter(|c| c.payload() == place)
                        .count(),
                    1
                );
                assert!(
                    !scene
                        .commands()
                        .iter()
                        .any(|c| c.payload().starts_with("placement:"))
                );
                let typography = crate::raster_target::selected_typography(Some(&look)).unwrap();
                let measured = conduitos::display::typography::TextLayout::new_selected(
                    place,
                    typography.style(placement.text_role().into()),
                    placement.bounds.width,
                    100,
                )
                .unwrap()
                .finish_height();
                assert!(measured <= u32::from(placement.bounds.height));
            }
        }
    }
    assert_eq!(f, original);
}
#[test]
fn absent_placement_has_no_invented_room_and_invalid_metadata_keeps_full_fallback() {
    let f = face(None);
    let (scene, _, _) = crate::property_mask::frame(&f, 500, 476, 0, None).unwrap();
    assert!(
        !scene
            .commands()
            .iter()
            .any(|c| c.payload().contains(" room"))
    );
    for invalid in ["a\nb", &"x".repeat(97)] {
        let f = face(Some(invalid));
        let original = f.clone();
        assert!(crate::property_mask::frame(&f, 500, 476, 0, None).is_none());
        assert_eq!(f, original);
        assert!(
            crate::view::frame_with_context(f, 500, 476, 0, None, true, None, false, false).is_ok()
        );
    }
}
