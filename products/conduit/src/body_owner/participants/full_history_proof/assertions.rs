//! Complete readings and existing quantity projection share the genuine Face.
use conduit_face::{
    Face, FacePropertyValue, FaceRole, PreparedQuantitySeries, QUANTITY_SERIES_ROLE,
};
use serde_json::Value;

pub(super) fn complete_view(face: &Face, view: &Value) {
    assert_eq!(view["face_id"], face.identity.as_str());
    assert_eq!(view["face_revision"], face.revision.to_string());
    let subjects = view["subjects"].as_array().unwrap();
    assert_eq!(subjects.len(), face.subjects.len());
    assert_eq!(
        view["relationships"].as_array().unwrap().len(),
        face.relationships.len()
    );
    assert_eq!(
        view["actions"].as_array().unwrap().len(),
        face.actions.len()
    );
    for (reading, source) in view["relationships"]
        .as_array()
        .unwrap()
        .iter()
        .zip(&face.relationships)
    {
        assert_eq!(reading["source"], source.source);
        assert_eq!(reading["target"], source.target);
        assert_eq!(reading["kind"], format!("{:?}", source.kind));
    }
    for (reading, source) in view["actions"]
        .as_array()
        .unwrap()
        .iter()
        .zip(&face.actions)
    {
        assert_eq!(reading["identity"], source.identity);
        assert_eq!(reading["name"], source.name);
        assert_eq!(reading["intent"], source.intent);
    }
    let mut series = 0;
    for (reading, subject) in subjects.iter().zip(&face.subjects) {
        assert_eq!(reading["identity"], subject.identity);
        assert_eq!(reading["name"], subject.name);
        let expected = face
            .properties
            .iter()
            .filter(|p| p.subject == subject.identity)
            .collect::<Vec<_>>();
        let properties = reading["properties"].as_array().unwrap();
        assert_eq!(
            properties.len(),
            expected.len(),
            "lost facts for {}",
            subject.identity
        );
        for (value, property) in properties.iter().zip(expected) {
            let value = value.as_str().unwrap();
            assert!(value.starts_with(&format!("{}: ", property.name)));
            match &property.value {
                FacePropertyValue::Text(text) | FacePropertyValue::Identity(text) => {
                    assert_eq!(value, format!("{}: {text}", property.name));
                }
                FacePropertyValue::Count(number) => {
                    assert_eq!(value, format!("{}: {number}", property.name))
                }
                FacePropertyValue::Signed(number) => {
                    assert_eq!(value, format!("{}: {number}", property.name))
                }
                FacePropertyValue::Flag(flag) => {
                    assert_eq!(value, format!("{}: {flag}", property.name))
                }
                FacePropertyValue::BaseImplementationId(id) => {
                    assert_eq!(value, format!("{}: {}", property.name, id.as_str()))
                }
                FacePropertyValue::TypedValue { contract, bytes } => {
                    let display = conduit_face::display_typed_value(contract, bytes);
                    assert_eq!(value, format!("{}: {display}", property.name));
                    let typed = reading["values"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|row| row["name"] == property.name)
                        .unwrap();
                    assert_eq!(typed["kind"], contract.value_kind.as_str());
                    assert_eq!(typed["text"], display);
                }
                FacePropertyValue::Content(bytes) => assert_eq!(
                    value,
                    format!("{}: {} content bytes", property.name, bytes.len())
                ),
                FacePropertyValue::ValueContract(contract) => {
                    assert_eq!(value, format!("{}: {contract:?}", property.name))
                }
            }
        }
        let text = face
            .text
            .iter()
            .filter(|t| t.subject == subject.identity)
            .map(|t| Value::String(t.text.clone()))
            .collect::<Vec<_>>();
        assert_eq!(reading["text"].as_array().unwrap(), &text);
        if matches!(&subject.role, FaceRole::Semantic(role) if role.as_str() == QUANTITY_SERIES_ROLE)
        {
            series += 1;
            let prepared = PreparedQuantitySeries::prepare(face, &subject.identity, 64).unwrap();
            assert_eq!(prepared.samples().len(), 64);
            let expected = prepared.project(None, 16).unwrap();
            assert_eq!(expected.points.len(), 16);
            assert_eq!(expected.omitted_points, 16);
            assert_eq!(expected.unavailable_samples, 32);
            assert_eq!(
                reading["quantity_series"]["plot"],
                serde_json::to_value(expected).unwrap()
            );
            assert!(reading["quantity_series"]["refusal"].is_null());
        }
    }
    assert_eq!(series, 2);
}
