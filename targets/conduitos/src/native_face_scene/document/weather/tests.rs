use super::summary;
use crate::native_face_scene::tests::face;
use alloc::vec;
use conduit_core::{CheckedValueContract, Quantity, TEMPERATURE_INFO_ID, Unit, kind_id};
use conduit_face::{Face, FaceProperty, FacePropertyValue, FaceRole, FaceSubject, FaceText};

fn reading(availability: &str) -> Face {
    let mut result = face(8);
    let subject = FaceSubject {
        identity: "weather".into(),
        role: FaceRole::Semantic(kind_id("weather/conditions")),
        name: "Conditions".into(),
    };
    let value = Quantity::from_decimal(6700, -3, Unit::Celsius)
        .unwrap()
        .encode();
    let contract =
        CheckedValueContract::new(kind_id(TEMPERATURE_INFO_ID), value.len() as u32, vec![])
            .unwrap();
    for (name, value) in [
        ("availability", FacePropertyValue::Text(availability.into())),
        ("location", FacePropertyValue::Text("Seattle".into())),
        ("conditions", FacePropertyValue::Text("Cloudy".into())),
        (
            "temperature",
            FacePropertyValue::TypedValue {
                contract,
                bytes: value.to_vec(),
            },
        ),
    ] {
        result.properties.push(FaceProperty {
            subject: subject.identity.clone(),
            name: name.into(),
            value,
        });
    }
    result.text.push(FaceText {
        subject: subject.identity.clone(),
        text: "Seattle: 6.7 °C, Cloudy.".into(),
    });
    result.subjects.push(subject);
    result
}

#[test]
fn renders_the_selected_temperature_unit_without_changing_canonical_face_content() {
    let face = reading("fresh");
    let original = face.clone();
    let subject = face.subjects.last().unwrap();
    assert_eq!(
        summary(&face, subject, Some(Unit::Fahrenheit)).as_deref(),
        Some("Seattle: 44.1 °F, Cloudy.")
    );
    assert_eq!(face, original);
}

#[test]
fn stale_weather_keeps_its_warning_in_the_selected_unit() {
    let face = reading("stale");
    let text = summary(&face, face.subjects.last().unwrap(), Some(Unit::Fahrenheit)).unwrap();
    assert!(text.contains("44.1 °F"), "{text}");
    assert!(
        text.contains("out of date") || text.contains("stale"),
        "{text}"
    );
}

#[test]
fn unavailable_unknown_and_untyped_weather_leave_the_authored_wording_in_charge() {
    for availability in ["unavailable", "unknown"] {
        let face = reading(availability);
        assert!(summary(&face, face.subjects.last().unwrap(), Some(Unit::Fahrenheit)).is_none());
    }
    let mut face = reading("fresh");
    face.properties.retain(|p| p.name != "temperature");
    face.properties.push(FaceProperty {
        subject: "weather".into(),
        name: "temperature-celsius-milli".into(),
        value: FacePropertyValue::Signed(6700),
    });
    assert!(summary(&face, face.subjects.last().unwrap(), Some(Unit::Fahrenheit)).is_none());
}
