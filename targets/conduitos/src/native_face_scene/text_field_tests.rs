use super::*;
use alloc::vec;
use conduit_core::{CheckedValueContract, ValueConstraint, kind_id};
use conduit_face::{FaceActionAvailability, FaceProperty, FacePropertyValue, FaceRole};
use conduit_human::{KeyEvent, KeyModifiers, KeyTransition};

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

fn finite_field() -> Face {
    let mut face = field();
    let contract = CheckedValueContract::new(
        kind_id(conduit_face::UTF8_TEXT_VALUE_KIND),
        64,
        vec![ValueConstraint::CanonicalMembership {
            members: vec![b"auto".to_vec(), b"roman".to_vec()],
            negated: false,
        }],
    )
    .unwrap();
    face.actions[0].arguments[0].contract = contract.clone();
    face.properties
        .iter_mut()
        .find(|property| property.name == "current-value")
        .unwrap()
        .value = FacePropertyValue::TypedValue {
        contract,
        bytes: b"auto".to_vec(),
    };
    seal(face)
}

fn press(usage: u8) -> KeyEvent {
    KeyEvent::new(usage, KeyTransition::Pressed, KeyModifiers::NONE).unwrap()
}

#[test]
fn finite_choices_wrap_only_inside_the_bound_source_contract() {
    let face = finite_field();
    assert_eq!(text_field::adjacent(&face, 0, None, true), Some("roman"));
    assert_eq!(
        text_field::adjacent(&face, 0, Some(b"roman"), true),
        Some("auto")
    );
    assert_eq!(text_field::adjacent(&face, 0, None, false), Some("roman"));
    assert_eq!(
        text_field::adjacent(&face, 0, Some(b"unfinished"), true),
        Some("auto")
    );
    assert_eq!(text_field::adjacent(&field(), 0, None, true), None);
    assert_eq!(text_field::adjacent(&tests::face(7), 0, None, true), None);
}

#[test]
fn a_finite_choice_cannot_bypass_other_constraints_or_expand_its_admitted_view() {
    let mut face = finite_field();
    let contract = CheckedValueContract::new(
        kind_id(conduit_face::UTF8_TEXT_VALUE_KIND),
        64,
        vec![
            ValueConstraint::ByteLength {
                minimum: 4,
                maximum: 4,
            },
            ValueConstraint::CanonicalMembership {
                members: vec![b"auto".to_vec(), b"roman".to_vec()],
                negated: false,
            },
        ],
    )
    .unwrap();
    face.actions[0].arguments[0].contract = contract.clone();
    face.properties
        .iter_mut()
        .find(|property| property.name == "current-value")
        .unwrap()
        .value = FacePropertyValue::TypedValue {
        contract,
        bytes: b"auto".to_vec(),
    };
    let face = seal(face);
    assert_eq!(text_field::choices(&face, 0).unwrap().as_slice(), &["auto"]);
    assert_eq!(text_field::adjacent(&face, 0, None, true), Some("auto"));
    let mut large = finite_field();
    let members = (0..33)
        .map(|index| alloc::format!("value-{index:02}").into_bytes())
        .collect();
    let contract = CheckedValueContract::new(
        kind_id(conduit_face::UTF8_TEXT_VALUE_KIND),
        64,
        vec![ValueConstraint::CanonicalMembership {
            members,
            negated: false,
        }],
    )
    .unwrap();
    large.actions[0].arguments[0].contract = contract.clone();
    large
        .properties
        .iter_mut()
        .find(|property| property.name == "current-value")
        .unwrap()
        .value = FacePropertyValue::TypedValue {
        contract,
        bytes: b"value-00".to_vec(),
    };
    let large = seal(large);
    assert!(text_field::choices(&large, 0).is_none());
    assert_eq!(text_field::current(&large, 0), Some("value-00"));
}

#[test]
fn arrow_choice_is_a_cancellable_draft_until_the_checked_enter_submission() {
    let face = finite_field();
    let show = tests::fixture::show(&face);
    let mut scene = NativeFaceScene::prepare(face.clone(), 640, 480).unwrap();
    scene
        .focus_named(&FaceFocusRequest {
            action_id: "edit".into(),
            argument_name: Some("value".into()),
        })
        .unwrap();
    assert!(matches!(
        scene.key(press(79), &show, 1).unwrap(),
        FaceSceneInput::Changed
    ));
    assert!(
        scene
            .input
            .preview(
                scene.focused(),
                &chrome::Chrome::prepare(scene.screen, scene.text_scale_percent).unwrap(),
                scene.text_scale_percent
            )
            .unwrap()
            .unwrap()
            .contains("Draft: roman")
    );
    assert_eq!(scene.face(), &face);
    assert!(matches!(
        scene.key(press(41), &show, 2).unwrap(),
        FaceSceneInput::Changed
    ));
    assert_eq!(
        scene
            .input
            .preview(
                scene.focused(),
                &chrome::Chrome::prepare(scene.screen, scene.text_scale_percent).unwrap(),
                scene.text_scale_percent
            )
            .unwrap(),
        None
    );
    assert_eq!(scene.face(), &face);
    assert!(matches!(
        scene.key(press(79), &show, 3).unwrap(),
        FaceSceneInput::Changed
    ));
    let FaceSceneInput::Submitted { interaction, .. } = scene.key(press(40), &show, 4).unwrap()
    else {
        panic!("Enter must submit the checked replacement")
    };
    assert_eq!(interaction.arguments.len(), 1);
    assert_eq!(interaction.arguments[0].value, b"roman");
    assert_eq!(scene.face(), &face);
    assert!(matches!(
        scene.key(press(80), &show, 5),
        Err(FaceSceneError::StaleFace)
    ));
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
    let unavailable = seal(unavailable);
    assert_eq!(text_field::current(&unavailable, 0), Some("Éléonore 雪"));
    let (primary, _) = document::prepare(&unavailable, true).unwrap();
    assert!(
        primary
            .iter()
            .any(|row| row.text == "Friendly name: Éléonore 雪" && row.control.is_none())
    );
    assert!(
        primary
            .iter()
            .any(|row| row.text.contains("Source unavailable"))
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
    assert_eq!(field.text, "Friendly name: Éléonore 雪 · Type to replace");
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

#[test]
fn finite_action_argument_chooses_a_draft_without_a_committed_value_binding() {
    let mut face = finite_field();
    face.properties.retain(|property| {
        !matches!(
            property.name.as_str(),
            "current-value" | "value-change-action"
        )
    });
    face.subjects
        .iter_mut()
        .find(|subject| subject.identity == face.actions[0].target)
        .unwrap()
        .role = FaceRole::Plot;
    let face = seal(face);
    assert_eq!(text_field::current(&face, 0), None);
    assert_eq!(text_field::adjacent(&face, 0, None, true), Some("auto"));
    assert_eq!(text_field::adjacent(&face, 0, None, false), Some("roman"));
    assert_eq!(
        text_field::adjacent(&face, 0, Some(b"auto"), true),
        Some("roman")
    );
    let mut scene = NativeFaceScene::prepare(face.clone(), 640, 480).unwrap();
    scene
        .focus_named(&FaceFocusRequest {
            action_id: face.actions[0].identity.clone(),
            argument_name: Some(face.actions[0].arguments[0].name.clone()),
        })
        .unwrap();
    let show = tests::fixture::show(&face);
    assert!(matches!(
        scene.key(press(79), &show, 1).unwrap(),
        FaceSceneInput::Changed
    ));
    assert_eq!(scene.face(), &face);
    assert!(matches!(
        scene.key(press(43), &show, 2).unwrap(),
        FaceSceneInput::Changed
    ));
    assert_eq!(scene.face(), &face);
    scene
        .focus_named(&FaceFocusRequest {
            action_id: face.actions[0].identity.clone(),
            argument_name: Some(face.actions[0].arguments[0].name.clone()),
        })
        .unwrap();
    scene.key(press(79), &show, 3).unwrap();
    let FaceSceneInput::Submitted { interaction, .. } = scene.key(press(40), &show, 4).unwrap()
    else {
        panic!("Enter must commit the exact contract member");
    };
    assert_eq!(interaction.arguments[0].value, b"auto");
    assert_eq!(scene.face(), &face);
}

#[test]
fn reading_and_background_pointer_cancel_command_drafts_without_submission() {
    let mut face = finite_field();
    face.properties.retain(|property| {
        !matches!(
            property.name.as_str(),
            "current-value" | "value-change-action"
        )
    });
    face.subjects
        .iter_mut()
        .find(|subject| subject.identity == face.actions[0].target)
        .unwrap()
        .role = FaceRole::Plot;
    let face = seal(face);
    let show = tests::fixture::show(&face);
    for usage in [59, 60, 78, 0] {
        let mut scene = NativeFaceScene::prepare(face.clone(), 640, 480).unwrap();
        scene
            .focus_named(&FaceFocusRequest {
                action_id: face.actions[0].identity.clone(),
                argument_name: Some(face.actions[0].arguments[0].name.clone()),
            })
            .unwrap();
        scene.key(press(79), &show, 1).unwrap();
        let result = if usage == 0 {
            scene.pointer(639, 479, &show, 2)
        } else {
            scene.key(press(usage), &show, 2)
        };
        assert!(matches!(result.unwrap(), FaceSceneInput::Changed));
        assert_eq!(scene.face(), &face);
    }
}
