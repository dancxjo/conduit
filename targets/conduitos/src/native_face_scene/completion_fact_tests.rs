//! A completion fact is wording, independently of typed Boolean input.
use super::*;
use alloc::collections::BTreeSet;
use conduit_face::{
    FaceActionAvailability, FaceProperty, FacePropertyValue, FaceRole, FaceSubject,
};

#[test]
fn completion_facts_are_text_without_manufacturing_checkbox_controls() {
    let name = "Keep [ ] literal wording 雪";
    for (value, caption) in [
        (
            Some(FacePropertyValue::Flag(false)),
            format!("Incomplete · {name}"),
        ),
        (
            Some(FacePropertyValue::Flag(true)),
            format!("Complete · {name}"),
        ),
        (Some(FacePropertyValue::Text("true".into())), name.into()),
        (None, name.into()),
    ] {
        let properties = value
            .map(|value| FaceProperty {
                subject: "item/completion".into(),
                name: "complete".into(),
                value,
            })
            .into_iter()
            .collect();
        let face = Face::new_with_semantics(
            1,
            tests::face(1).basis,
            vec![FaceSubject {
                identity: "item/completion".into(),
                role: FaceRole::Item,
                name: name.into(),
            }],
            vec![],
            properties,
            vec![],
            vec![],
            vec![],
        )
        .unwrap();
        let original = face.clone();
        for input in [false, true] {
            let (primary, _) = document::prepare(&face, input).unwrap();
            let row = primary
                .iter()
                .find(|row| row.text == caption)
                .expect("exact state and authored wording");
            assert!(row.control.is_none());
            assert!(row.checkbox.is_none());
            assert_eq!(face, original);
        }
    }
}

#[test]
fn actual_todo_caption_retains_source_actions_and_observer_limits() {
    let face = structured_source_tests::todo();
    let original = face.clone();
    let item = face
        .subjects
        .iter()
        .find(|subject| {
            subject.role == FaceRole::Item
                && face.properties.iter().any(|property| {
                    property.subject == subject.identity
                        && property.name == "complete"
                        && property.value == FacePropertyValue::Flag(false)
                })
        })
        .unwrap();
    let expected: BTreeSet<_> = face
        .actions
        .iter()
        .filter(|action| {
            action.target == item.identity
                && action.availability == FaceActionAvailability::Available
        })
        .map(|action| action.identity.as_str())
        .collect();
    assert!(!expected.is_empty());
    for input in [false, true] {
        let (primary, details) = document::prepare(&face, input).unwrap();
        let row = primary
            .iter()
            .find(|row| row.text == format!("Incomplete · {}", item.name))
            .unwrap();
        assert!(row.control.is_none() && row.checkbox.is_none());
        let controls: BTreeSet<_> = primary
            .iter()
            .filter_map(|row| row.control)
            .map(|control| &face.actions[control.action])
            .filter(|action| action.target == item.identity)
            .map(|action| action.identity.as_str())
            .collect();
        assert_eq!(
            controls,
            if input {
                expected.clone()
            } else {
                BTreeSet::new()
            }
        );
        assert!(details.iter().any(|row| row.text.contains(&item.name)));
        assert_eq!(face, original);
    }
}
