use super::*;
use alloc::vec;
use conduit_core::{CheckedValueContract, kind_id};
use conduit_data::MeasurementPlotPoint;

fn series(points: usize) -> MeasurementPlotSeries {
    MeasurementPlotSeries::from_projected(
        (0..points)
            .map(|i| {
                let position = if points == 1 {
                    0
                } else {
                    i * 1_000_000 / (points - 1)
                };
                MeasurementPlotPoint::new(i as u64, position as i64, position as i64).unwrap()
            })
            .collect(),
        points,
        0,
    )
    .unwrap()
}
fn face(labels: &[&str], points: usize) -> Face {
    let mut face = crate::native_face_scene::tests::face(7);
    face.subjects[0].role = FaceRole::Semantic(kind_id(MEASUREMENT_PLOT_FACE_KIND));
    for name in labels {
        face.properties.push(FaceProperty {
            subject: face.subjects[0].identity.clone(),
            name: (*name).into(),
            value: FacePropertyValue::TypedValue {
                contract: CheckedValueContract::new(
                    kind_id(MEASUREMENT_PLOT_SERIES_INFO_ID),
                    1024,
                    vec![],
                )
                .unwrap(),
                bytes: conduit_data::encode_measurement_plot_series(&series(points)).unwrap(),
            },
        });
    }
    reseal(face)
}
fn reseal(face: Face) -> Face {
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
fn full_series_uses_continuous_chunks_and_preserves_ordinary_face() {
    for (width, height) in [(640, 480), (1024, 600)] {
        let face = face(&["Blue", "Green", "Yellow", "White"], 32);
        let mut view =
            NativeFaceScene::prepare_with_input(face.clone(), width, height, false).unwrap();
        let mut text = String::new();
        let mut chunks = Vec::new();
        for _ in 0..view.pages() {
            let frame = view.frame().unwrap();
            assert!(frame.scene.commands().len() <= conduit_face::MAX_GRAPHICS_COMMANDS);
            for command in frame.scene.commands() {
                text.push_str(command.payload());
                if let Some(line) = command.polyline_geometry() {
                    chunks.push(line);
                }
            }
            view.turn_page(true);
        }
        assert_eq!(chunks.len(), 12);
        for lines in chunks.chunks_exact(3) {
            assert_eq!(
                lines
                    .iter()
                    .map(|line| line.points().len() - 1)
                    .sum::<usize>(),
                31
            );
            assert_eq!(lines[0].points().last(), lines[1].points().first());
            assert_eq!(lines[1].points().last(), lines[2].points().first());
        }
        for label in [
            "Blue",
            "Green",
            "Yellow",
            "White",
            "Window sample index",
            "Normalized value",
            "Friendly name",
        ] {
            assert!(text.contains(label), "missing {label}");
        }
        assert_eq!(view.face(), &face);
    }
}

#[test]
fn exact_semantic_role_and_value_kind_are_both_required() {
    let mut face = face(&["Blue"], 3);
    face.subjects[0].role = FaceRole::Document;
    assert!(item(&face, 0).unwrap().is_none());
    face.subjects[0].role = FaceRole::Semantic(kind_id(MEASUREMENT_PLOT_FACE_KIND));
    let index = face.properties.len() - 1;
    let FacePropertyValue::TypedValue { contract, .. } = &mut face.properties[index].value else {
        panic!()
    };
    contract.value_kind = kind_id("unrelated/value");
    assert!(matches!(
        item(&face, 0),
        Err(FaceSceneError::UnsupportedStructure)
    ));
}

#[test]
fn malformed_series_and_ambiguous_names_refuse_before_drawing() {
    for labels in [&["same", "same"][..], &["a", "b", "c", "d", "e"][..]] {
        assert!(matches!(
            item(&face(labels, 3), 0),
            Err(FaceSceneError::UnsupportedStructure)
        ));
    }
    let mut face = face(&["Blue"], 3);
    let index = face.properties.len() - 1;
    let FacePropertyValue::TypedValue { bytes, .. } = &mut face.properties[index].value else {
        panic!()
    };
    *bytes = vec![0; 1025];
    assert!(matches!(item(&face, 0), Err(FaceSceneError::InvalidFace)));
    *match &mut face.properties[index].value {
        FacePropertyValue::TypedValue { bytes, .. } => bytes,
        _ => unreachable!(),
    } = vec![0];
    assert!(matches!(item(&face, 0), Err(FaceSceneError::InvalidFace)));
}

#[test]
fn one_point_has_visible_marker_and_small_extent_is_explicit_refusal() {
    let face = face(&["Blue"], 1);
    let mut view = NativeFaceScene::prepare_with_input(face.clone(), 1024, 600, false).unwrap();
    let mut found = false;
    for _ in 0..view.pages() {
        let frame = view.frame().unwrap();
        found |= frame
            .scene
            .commands()
            .iter()
            .any(|c| c.paint == GraphicsPaintRole::Accent && c.polyline_geometry().is_none());
        view.turn_page(true);
    }
    assert!(found);
    assert!(NativeFaceScene::prepare_with_input(face, 100, 100, false).is_err());
}
