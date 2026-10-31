#[cfg(test)]
mod property_contract {
    use crate::property_mask::*;
    #[test]
    fn body_heading_wraps_without_overlapping_readings_or_changing_face() {
        let mut face = crate::view::unavailable("Awaiting source");
        face.subjects[0].role = FaceRole::Info;
        let id = "a".repeat(64);
        face.basis.body_id = Some(serde_json::from_value(serde_json::json!(id)).unwrap());
        face.subjects.push(conduit_face::FaceSubject {
            identity: format!("body/{id}"),
            role: FaceRole::Body,
            name: "The greenhouse of Malo Lefèvre Laurent and friends".into(),
        });
        face.text.clear();
        face.properties.push(conduit_face::FaceProperty {
            subject: face.subjects[0].identity.clone(),
            name: "sensor-status".into(),
            value: FacePropertyValue::Text("ready".into()),
        });
        let original = face.clone();
        let (scene, _, _) = frame(&face, 368, 600, 0, None).unwrap();
        let title = scene
            .commands()
            .iter()
            .find(|c| c.payload() == "The greenhouse of Malo Lefèvre Laurent and friends")
            .unwrap();
        assert!(title.bounds.height > 40);
        let row = scene
            .commands()
            .iter()
            .find(|c| c.payload() == "sensor status: ready")
            .unwrap();
        assert!(row.bounds.y >= title.bounds.y + title.bounds.height as i16);
        assert!(has_no_additional_native_primary_content(&face));
        assert_eq!(face, original);
        face.subjects[1].identity = "body/some-other-body".into();
        assert_eq!(body_heading(&face), None);
        assert!(!has_no_additional_native_primary_content(&face));
    }

    #[test]
    fn preferred_temperature_unit_keeps_exact_face_and_other_values() {
        use conduit_core::{
            CheckedValueContract, QUANTITY_ENCODED_LEN, QUANTITY_INFO_ID, Quantity, Unit, kind_id,
        };
        let mut face = crate::view::unavailable("Awaiting source");
        face.subjects[0].role = FaceRole::Info;
        face.properties.push(conduit_face::FaceProperty {
            subject: face.subjects[0].identity.clone(),
            name: "value".into(),
            value: FacePropertyValue::TypedValue {
                contract: CheckedValueContract::new(
                    kind_id(QUANTITY_INFO_ID),
                    QUANTITY_ENCODED_LEN as u32,
                    vec![],
                )
                .unwrap(),
                bytes: Quantity::from_decimal(225, -1, Unit::Celsius)
                    .unwrap()
                    .encode()
                    .to_vec(),
            },
        });
        face.properties.push(conduit_face::FaceProperty {
            subject: face.subjects[0].identity.clone(),
            name: "ready".into(),
            value: FacePropertyValue::Flag(true),
        });
        face = Face::new(
            7,
            face.basis,
            face.subjects,
            face.relationships,
            face.properties,
            face.text,
        )
        .unwrap();
        let original = face.clone();
        let canonical = rows(&face, None).unwrap();
        assert_eq!(canonical[0].value, "22.5 °C");
        let preferred = rows(&face, Some(Unit::Fahrenheit)).unwrap();
        assert_eq!(preferred[0].value, "72.5 °F");
        assert_eq!(preferred[1].value, "true");
        assert_eq!(face, original);
    }
    #[test]
    fn property_look_keeps_face_and_explicit_unavailable_status() {
        let mut face = crate::view::unavailable("Awaiting source");
        face.subjects[0].role = FaceRole::Info;
        face.properties.push(conduit_face::FaceProperty {
            subject: face.subjects[0].identity.clone(),
            name: "sensor-status".into(),
            value: FacePropertyValue::Text("unavailable".into()),
        });
        face.properties.push(conduit_face::FaceProperty {
            subject: face.subjects[0].identity.clone(),
            name: "fresh".into(),
            value: FacePropertyValue::Flag(false),
        });
        // Reconstruct only this test fixture's content identity after editing it.
        face = Face::new(
            7,
            face.basis,
            face.subjects,
            face.relationships,
            face.properties,
            face.text,
        )
        .unwrap();
        let original = face.clone();
        let (scene, _, _) = frame(&face, 1024, 600, 0, None).unwrap();
        assert_eq!(face, original);
        assert!(
            scene
                .commands()
                .iter()
                .any(|c| c.payload() == "sensor status: unavailable")
        );
        assert!(
            scene
                .commands()
                .iter()
                .any(|c| c.payload() == "fresh: false")
        );
    }
}

#[cfg(test)]
mod status_layout_tests {
    use crate::property_mask::*;
    #[test]
    fn all_property_pages_reserve_notice_strip_without_moving_footer_or_losing_rows() {
        let mut face = crate::view::unavailable("Awaiting source");
        face.subjects[0].role = FaceRole::Info;
        for i in 0..36 {
            face.properties.push(conduit_face::FaceProperty {
                subject: face.subjects[0].identity.clone(),
                name: format!("reading-{i}"),
                value: FacePropertyValue::Text(format!("value-{i}")),
            });
        }
        let mut values = Vec::new();
        let (_, _, total_pages) = frame(&face, 1024, 600, 0, None).unwrap();
        assert!(total_pages > 1);
        for page in 0..total_pages {
            let (scene, actual, pages) = frame(&face, 1024, 600, page, None).unwrap();
            assert_eq!((actual, pages), (page, total_pages));
            assert!(scene.commands().len() <= 48);
            for command in scene.commands() {
                if command.payload().is_empty() {
                    continue;
                }
                assert!(command.bounds.y >= crate::view::STATUS_STRIP_HEIGHT as i16);
                if command.payload().starts_with("Page ") {
                    assert_eq!(command.bounds.y, 560);
                    assert_eq!(command.bounds.height, 32);
                } else {
                    assert!(i32::from(command.bounds.y) + i32::from(command.bounds.height) <= 560);
                }
                if let Some((_, value)) = command.payload().split_once(": ")
                    && value.starts_with("value-")
                {
                    values.push(value.to_owned());
                }
            }
        }
        assert_eq!(
            values,
            (0..36).map(|i| format!("value-{i}")).collect::<Vec<_>>()
        );
    }
}
