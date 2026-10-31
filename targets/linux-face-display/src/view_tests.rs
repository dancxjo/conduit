//! Native presentation behavior and raster geometry proofs.
use super::*;
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn complete_info_property_pages_do_not_alternate_with_an_empty_snapshot() {
        let base = unavailable("unused");
        let make = |properties| {
            Face::new(
                9,
                base.basis.clone(),
                vec![FaceSubject {
                    identity: "instrument".into(),
                    role: FaceRole::Info,
                    name: "Room conditions".into(),
                }],
                vec![],
                properties,
                vec![],
            )
            .unwrap()
        };
        let property = conduit_face::FaceProperty {
            subject: "instrument".into(),
            name: "reading".into(),
            value: conduit_face::FacePropertyValue::Text("Real value".into()),
        };
        let face = make(vec![property.clone()]);
        for tick in 0..6 {
            let (scene, page, pages, _, _) = frame(face.clone(), 1024, 600, tick, None).unwrap();
            assert_eq!((page, pages), (0, 1));
            assert!(
                scene
                    .commands()
                    .iter()
                    .any(|c| c.payload() == "reading: Real value")
            );
        }
        let unsupported = conduit_face::FaceProperty {
            subject: "instrument".into(),
            name: "count".into(),
            value: conduit_face::FacePropertyValue::Count(12),
        };
        let mixed = make(vec![property, unsupported]);
        let (_, _, pages, _, _) = frame(mixed, 1024, 600, 0, None).unwrap();
        assert_eq!(pages, 1, "Count metadata adds no native primary content");
    }
    #[test]
    #[ignore = "requires explicitly selected private actual current Face"]
    fn actual_info_face_with_metadata_has_no_empty_page_in_any_cycle() {
        let path =
            std::env::var_os("CONDUIT_PRIVATE_CURRENT_FACE_TEST").expect("private actual Face");
        let document: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let face: Face = serde_json::from_value(document["face"].clone()).unwrap();
        face.validate().unwrap();
        assert!(
            face.properties
                .iter()
                .any(|p| matches!(p.value, conduit_face::FacePropertyValue::Count(_)))
        );
        for tick in 0..8 {
            let (scene, page, pages, _, _) = frame(
                face.clone(),
                1024,
                600,
                tick,
                Some(conduit_core::Unit::Fahrenheit),
            )
            .unwrap();
            assert_eq!((page, pages), (0, 1));
            assert!(scene.commands().iter().any(|c| c.payload().contains("°F")));
            assert!(scene.commands().iter().any(|c| c.payload().contains('%')));
        }
    }
    #[test]
    fn sensor_pages_do_not_hide_transcript_from_the_same_face() {
        let base = unavailable("Transcript: the greenhouse is quiet");
        let face = Face::new(
            9,
            base.basis,
            vec![
                base.subjects[0].clone(),
                FaceSubject {
                    identity: "sensor".into(),
                    role: FaceRole::Info,
                    name: "Greenhouse".into(),
                },
            ],
            vec![],
            vec![conduit_face::FaceProperty {
                subject: "sensor".into(),
                name: "reading".into(),
                value: conduit_face::FacePropertyValue::Text("Sensor marker".into()),
            }],
            base.text,
        )
        .unwrap();
        let original = face.clone();
        let (_, _, pages, _, _) = frame(face.clone(), 1024, 600, 0, None).unwrap();
        assert!(pages >= 2);
        let mut sensor = false;
        let mut transcript = false;
        for tick in 0..pages {
            let (scene, page, count, _, _) = frame(face.clone(), 1024, 600, tick, None).unwrap();
            assert_eq!((page, count), (tick, pages));
            sensor |= scene
                .commands()
                .iter()
                .any(|c| c.payload().contains("Sensor marker"));
            transcript |= scene
                .commands()
                .iter()
                .any(|c| c.payload().contains("greenhouse is quiet"));
        }
        assert!(
            sensor && transcript,
            "both contributions must remain perceptible"
        );
        assert_eq!(face, original);
    }
}

#[cfg(test)]
mod native_chart_pixel_tests {
    use super::*;
    use conduitos::display::{PixelTarget, RetainedPixelTarget};
    #[test]
    fn chart_pixel_is_written_once_without_stretching_and_preserves_status_strip() {
        let mut pixels = vec![0; 1024 * 600];
        let mut target = ChartViewport {
            width: 1024,
            height: 600,
            pixels: &mut pixels,
        };
        assert_eq!(target.format().width, 1024);
        assert_eq!(target.format().height, 564);
        target.write_pixel(511, 100, 0xabcdef).unwrap();
        assert_eq!(target.read_pixel(511, 100).unwrap(), 0xabcdef);
        assert!(target.write_pixel(1024, 100, 1).is_err());
        assert_eq!(pixels.iter().filter(|p| **p != 0).count(), 1);
        assert_eq!(pixels[(100 + 36) * 1024 + 511], 0xabcdef);
        assert!(pixels[..36 * 1024].iter().all(|p| *p == 0));
    }
}
