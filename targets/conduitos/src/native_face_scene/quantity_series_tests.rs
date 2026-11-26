use super::*;
#[path = "../../../../semantics/face/tests/common/quantity_series.rs"]
mod common;

#[test]
fn ordinary_quantity_series_prepares_owned_points_and_complete_reader() {
    let face = common::fixture();
    let scene = NativeFaceScene::prepare(face.clone(), 640, 480).unwrap();
    assert_eq!(scene.face, face);
    assert_eq!(scene.quantity_series.len(), 1);
    let series = &scene.quantity_series[0];
    assert_eq!(series.points.len(), 2);
    assert_eq!(series.points[0].time_millionths, 0);
    assert_eq!(series.points[1].time_millionths, 1_000_000);
    assert_eq!(series.unavailable_samples, 1);
    assert_eq!(series.gap_runs, 1);
    assert_eq!(
        scene
            .primary
            .iter()
            .filter(|row| row.plot.is_some())
            .count(),
        1
    );
    assert!(scene.primary.iter().any(|row| row.text.contains("10 V")));
    assert!(scene.primary.iter().any(|row| row.text.contains("12 V")));
    assert!(
        scene
            .details
            .iter()
            .any(|row| row.text.contains("meter-unavailable"))
    );
    assert!(scene.details.iter().all(|row| row.plot.is_none()));
    for page in 0..scene.pages() {
        let mut reader = NativeFaceScene::prepare(face.clone(), 640, 480).unwrap();
        reader.page = page;
        let frame = reader.frame().unwrap();
        assert_eq!(frame.face_id, &face.identity);
        assert_eq!(frame.revision, face.revision);
        assert!(frame.hits.is_empty());
        assert!(frame.scene.commands().len() <= conduit_face::MAX_GRAPHICS_COMMANDS);
    }
}

#[test]
fn malformed_series_keeps_original_reading_without_a_plot() {
    let mut face = common::fixture();
    face.properties
        .iter_mut()
        .find(|p| p.name == "clock")
        .unwrap()
        .value = conduit_face::FacePropertyValue::Text("unrecognized-clock".into());
    face.reseal_prepared().unwrap();
    let scene = NativeFaceScene::prepare(face.clone(), 320, 240).unwrap();
    assert!(scene.quantity_series.is_empty());
    assert!(scene.primary.iter().all(|row| row.plot.is_none()));
    assert_eq!(scene.face, face);
    assert!(
        scene
            .primary
            .iter()
            .any(|row| row.text.contains("Observation"))
    );
}

#[test]
fn point_plot_commands_are_bounded_nonconnecting_and_inside_the_admitted_row() {
    let face = common::fixture();
    let series = conduit_face::PreparedQuantitySeries::prepare(&face, "voltage", 64)
        .unwrap()
        .project(None, 16)
        .unwrap();
    let bounds = LayoutRect {
        x: 20,
        y: 30,
        width: 100,
        height: 48,
    };
    let mut graphics = GraphicsScene::empty();
    append(&mut graphics, &series, bounds, bounds).unwrap();
    assert_eq!(graphics.commands().len(), 4);
    for command in graphics.commands() {
        assert!(command.bounds.x >= bounds.x && command.bounds.y >= bounds.y);
        assert!(i32::from(command.bounds.x) + i32::from(command.bounds.width) <= 120);
        assert!(i32::from(command.bounds.y) + i32::from(command.bounds.height) <= 78);
    }
}

#[test]
fn selected_visual_metrics_keep_plot_and_reading_on_small_high_scale_pages() {
    use conduit_face::{VisualExpression, VisualUserConstraints, admit_visual_expression};
    let face = common::fixture();
    for (width, height, scale) in [(320, 240, 100), (320, 240, 200), (640, 480, 200)] {
        let mut requested = VisualExpression::phosphor();
        requested.text_scale_percent = scale;
        let selected = admit_visual_expression(
            Ok(requested),
            VisualUserConstraints {
                forced_colors: false,
                reduced_motion: true,
                minimum_text_scale_percent: 100,
                maximum_supported_text_scale_percent: 200,
            },
        )
        .unwrap();
        let mut scene =
            NativeFaceScene::prepare_visual_fallback(face.clone(), width, height, false, &selected)
                .unwrap();
        assert_eq!(scene.text_scale_percent(), scale);
        assert_eq!(
            scene
                .primary
                .iter()
                .filter(|row| row.plot.is_some())
                .count(),
            1
        );
        for page in 0..scene.pages() {
            scene.page = page;
            let frame = scene.frame().unwrap();
            assert_eq!(frame.face_id, &face.identity);
            assert!(frame.scene.commands().len() <= conduit_face::MAX_GRAPHICS_COMMANDS);
        }
        scene.showing_details = true;
        scene.page = 0;
        for page in 0..scene.pages() {
            scene.page = page;
            assert!(scene.frame().is_ok());
        }
    }
}

#[test]
fn multiple_series_survive_configured_and_structured_selected_look_layout() {
    let mut face = common::fixture();
    let subject = face
        .subjects
        .iter()
        .find(|s| s.identity == "voltage")
        .unwrap()
        .clone();
    face.subjects.push(conduit_face::FaceSubject {
        identity: "voltage-second".into(),
        ..subject
    });
    let extra = face
        .properties
        .iter()
        .filter(|p| p.subject == "voltage")
        .cloned()
        .map(|mut p| {
            p.subject = "voltage-second".into();
            p
        })
        .collect::<Vec<_>>();
    face.properties.extend(extra);
    let extra = face
        .relationships
        .iter()
        .filter(|r| r.source == "voltage")
        .cloned()
        .map(|mut r| {
            r.source = "voltage-second".into();
            r
        })
        .collect::<Vec<_>>();
    face.relationships.extend(extra);
    face.reseal_prepared().unwrap();
    let structure = crate::native_face_scene::structured_source_tests::structure(
        conduit_face::TODO_LAYOUT_MASK_SOURCE,
        "todo-component-flat",
        &face,
    );
    let look = conduit_face::admit_visual_look(
        &conduit_face::engineering_look().unwrap(),
        conduit_face::VisualUserConstraints {
            forced_colors: false,
            reduced_motion: true,
            minimum_text_scale_percent: 200,
            maximum_supported_text_scale_percent: 200,
        },
    )
    .unwrap();
    for mut scene in [
        NativeFaceScene::prepare_configured_look(
            face.clone(),
            320,
            480,
            false,
            structure.recipe(),
            &look,
        )
        .unwrap(),
        NativeFaceScene::prepare_structured_look(&structure, 320, 480, false, &look).unwrap(),
    ] {
        assert_eq!(scene.quantity_series.len(), 2);
        assert_eq!(
            scene
                .primary
                .iter()
                .filter(|row| row.plot.is_some())
                .count(),
            2
        );
        assert_eq!(scene.face(), &face);
        for page in 0..scene.pages() {
            scene.page = page;
            assert!(
                scene.frame().unwrap().scene.commands().len()
                    <= conduit_face::MAX_GRAPHICS_COMMANDS
            );
        }
    }
}

#[test]
fn viewer_temperature_preference_converts_temperature_and_preserves_voltage() {
    let selected = conduit_face::admit_visual_look(
        &conduit_face::engineering_look().unwrap(),
        conduit_face::VisualUserConstraints {
            forced_colors: false,
            reduced_motion: true,
            minimum_text_scale_percent: 100,
            maximum_supported_text_scale_percent: 200,
        },
    )
    .unwrap();
    let volts = common::fixture();
    let mut temperature = volts.clone();
    for property in &mut temperature.properties {
        if let conduit_face::FacePropertyValue::TypedValue { contract, bytes } = &mut property.value
        {
            *contract = conduit_core::CheckedValueContract::new(
                conduit_core::TEMPERATURE_INFO_ID.into(),
                conduit_core::QUANTITY_ENCODED_LEN as u32,
                vec![],
            )
            .unwrap();
            *bytes = conduit_core::Quantity::parse_plot_literal("10°C")
                .unwrap()
                .encode()
                .to_vec();
        }
    }
    temperature.reseal_prepared().unwrap();
    for (face, expected) in [(volts, "V"), (temperature, "°F")] {
        let scene = NativeFaceScene::prepare_visual_fallback_with_temperature(
            face.clone(),
            640,
            480,
            false,
            &selected.visual,
            Some(conduit_core::Unit::resolve("°F").unwrap()),
        )
        .unwrap();
        assert_eq!(scene.quantity_series[0].unit.as_deref(), Some(expected));
        assert_eq!(scene.face(), &face);
    }
}

#[test]
fn irregular_interior_time_uses_shared_time_axis_instead_of_sample_index() {
    let mut face = common::fixture();
    face.properties
        .retain(|p| p.name != "series-unavailability");
    face.properties
        .iter_mut()
        .find(|p| p.subject == "sample/2" && p.name == "series-available")
        .unwrap()
        .value = conduit_face::FacePropertyValue::Flag(true);
    let mut value = face
        .properties
        .iter()
        .find(|p| p.subject == "sample/1" && p.name == "voltage")
        .unwrap()
        .clone();
    value.subject = "sample/2".into();
    face.properties.push(value);
    face.reseal_prepared().unwrap();
    let series = conduit_face::PreparedQuantitySeries::prepare(&face, "voltage", 64)
        .unwrap()
        .project(None, 16)
        .unwrap();
    let bounds = LayoutRect {
        x: 20,
        y: 30,
        width: 100,
        height: 48,
    };
    let mut graphics = GraphicsScene::empty();
    append(&mut graphics, &series, bounds, bounds).unwrap();
    assert_eq!(series.points[1].time_millionths, 100_000);
    assert_eq!(graphics.commands()[3].bounds.x, 29);
    assert_eq!(graphics.commands()[4].bounds.x, 117);
}
