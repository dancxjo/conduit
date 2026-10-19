use super::*;
use alloc::vec;
use conduit_face::{FaceActionAvailability, FaceProperty, FacePropertyValue, FaceRole};

fn seal(face: Face) -> Face {
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

fn field() -> Face {
    let mut face = tests::face(7);
    let action = &face.actions[0];
    face.properties.extend([
        FaceProperty {
            subject: action.target.clone(),
            name: "current-value".into(),
            value: FacePropertyValue::TypedValue {
                contract: action.arguments[0].contract.clone(),
                bytes: "Éléonore 雪".as_bytes().to_vec(),
            },
        },
        FaceProperty {
            subject: action.target.clone(),
            name: "value-change-action".into(),
            value: FacePropertyValue::Identity(action.identity.clone()),
        },
    ]);
    seal(face)
}

#[test]
fn verified_current_text_is_read_only_and_unavailable_state_does_not_erase_it() {
    let face = field();
    let before = face.clone();
    assert_eq!(text_field::current(&face, 0), Some("Éléonore 雪"));
    assert_eq!(face, before);
    let mut unavailable = face.clone();
    unavailable.actions[0].availability = FaceActionAvailability::Unavailable {
        reason_code: "source/temporarily-unavailable".into(),
        explanation: "Source unavailable".into(),
    };
    assert_eq!(
        text_field::current(&seal(unavailable), 0),
        Some("Éléonore 雪")
    );
    assert_eq!(text_field::current(&tests::face(7), 0), None);
    assert_eq!(text_field::current(&face, 1), None);
}

#[test]
fn a_foreign_action_contract_or_subject_cannot_become_the_field_binding() {
    let face = field();
    let mut foreign = face.clone();
    foreign
        .properties
        .iter_mut()
        .find(|p| p.name == "value-change-action")
        .unwrap()
        .value = FacePropertyValue::Identity("finish".into());
    assert_eq!(text_field::current(&seal(foreign), 0), None);
    let mut changed_contract = face.clone();
    changed_contract.actions[0].arguments[0] =
        FaceActionArgument::text("value".into(), "Name".into(), 0, 63).unwrap();
    assert_eq!(text_field::current(&seal(changed_contract), 0), None);
    let mut other_role = face.clone();
    other_role.subjects[1].role = FaceRole::Item;
    assert_eq!(text_field::current(&seal(other_role), 0), None);
    let mut multiple = face.clone();
    multiple.actions[0]
        .arguments
        .push(FaceActionArgument::text("other".into(), "Other".into(), 0, 64).unwrap());
    assert_eq!(text_field::current(&seal(multiple), 0), None);
}

#[test]
fn ambiguous_or_untyped_current_values_fall_back_without_reading_prose() {
    let face = field();
    for name in ["current-value", "value-change-action"] {
        let mut ambiguous = face.clone();
        let property = ambiguous
            .properties
            .iter()
            .find(|p| p.name == name)
            .unwrap()
            .clone();
        ambiguous.properties.push(property);
        assert_eq!(text_field::current(&seal(ambiguous), 0), None);
    }
    let mut untyped = face.clone();
    untyped
        .properties
        .iter_mut()
        .find(|p| p.name == "current-value")
        .unwrap()
        .value = FacePropertyValue::Text("Éléonore 雪".into());
    assert_eq!(text_field::current(&seal(untyped), 0), None);
    let mut empty = face;
    let FacePropertyValue::TypedValue { bytes, .. } = &mut empty
        .properties
        .iter_mut()
        .find(|p| p.name == "current-value")
        .unwrap()
        .value
    else {
        unreachable!()
    };
    *bytes = vec![];
    assert_eq!(text_field::current(&seal(empty), 0), Some(""));
}

#[test]
fn current_field_row_targets_the_existing_argument_and_details_retain_the_contract() {
    let face = field();
    let (primary, details) = document::prepare(&face, true).unwrap();
    let controls: alloc::vec::Vec<_> = primary
        .iter()
        .filter(|row| row.control.is_some_and(|control| control.action == 0))
        .collect();
    assert_eq!(controls.len(), 2);
    let field = controls
        .iter()
        .find(|row| row.control.unwrap().argument == Some(0))
        .unwrap();
    assert_eq!(field.text, "Friendly name: Éléonore 雪");
    assert_eq!(field.control.unwrap().argument, Some(0));
    assert!(details.iter().any(|row| row.control
        == Some(FaceControl {
            action: 0,
            argument: Some(0),
        })));
    let (view_only, _) = document::prepare(&face, false).unwrap();
    assert!(view_only.iter().all(|row| row.control.is_none()));
    assert!(
        view_only
            .iter()
            .any(|row| row.text == "Friendly name: Éléonore 雪")
    );
}
