use super::*;
use conduit_face::FacePropertyValue;

fn current_value(face: &Face, action_id: &str) -> alloc::vec::Vec<u8> {
    let action = face
        .actions
        .iter()
        .find(|a| a.identity == action_id)
        .unwrap();
    assert_eq!(action.arguments.len(), 1);
    let bindings: alloc::vec::Vec<_> = face
        .properties
        .iter()
        .filter(|p| p.subject == action.target && p.name == "value-change-action")
        .collect();
    assert_eq!(bindings.len(), 1);
    assert_eq!(
        bindings[0].value,
        FacePropertyValue::Identity(action_id.into())
    );
    let values: alloc::vec::Vec<_> = face
        .properties
        .iter()
        .filter(|p| p.subject == action.target && p.name == "current-value")
        .collect();
    assert_eq!(values.len(), 1);
    let FacePropertyValue::TypedValue { contract, bytes } = &values[0].value else {
        panic!("current field value must retain its portable contract");
    };
    assert_eq!(contract, &action.arguments[0].contract);
    contract.validate(bytes).unwrap();
    bytes.clone()
}

#[test]
fn typed_field_values_follow_source_commits_without_changing_old_faces() {
    let mut draft = draft();
    let before = draft.face(&basis()).unwrap();
    let old_name = draft.friendly_name().as_bytes().to_vec();
    assert_eq!(current_value(&before, "creche.name"), old_name);
    assert_eq!(current_value(&before, "creche.naming"), b"surprise");
    assert_eq!(current_value(&before, "creche.search"), b"");

    let name = "Éléonore 雪";
    let (show, edit) = input(
        &draft,
        "creche.name",
        Some((UTF8_TEXT_VALUE_KIND, name.as_bytes())),
    )
    .unwrap();
    assert_eq!(
        draft.apply_face_interaction(&basis(), &show, &edit),
        Ok(BirthActionOutcome::Changed)
    );
    let named = draft.face(&basis()).unwrap();
    assert_eq!(current_value(&named, "creche.name"), name.as_bytes());
    assert_eq!(current_value(&before, "creche.name"), old_name);
    assert_ne!(named.identity, before.identity);
    assert!(
        draft
            .apply_face_interaction(&basis(), &show, &edit)
            .is_err()
    );

    apply(
        &mut draft,
        "creche.naming",
        Some((UTF8_TEXT_VALUE_KIND, b"roman")),
    )
    .unwrap();
    let tradition = draft.face(&basis()).unwrap();
    assert_eq!(current_value(&tradition, "creche.naming"), b"roman");
    assert_eq!(
        current_value(&tradition, "creche.name"),
        draft.friendly_name().as_bytes()
    );
    assert_eq!(current_value(&named, "creche.name"), name.as_bytes());
    apply(&mut draft, "creche.suggest", None).unwrap();
    let suggested = draft.face(&basis()).unwrap();
    assert_eq!(
        current_value(&suggested, "creche.name"),
        draft.friendly_name().as_bytes()
    );
    assert_ne!(draft.friendly_name(), name);
    apply(
        &mut draft,
        "creche.search",
        Some((UTF8_TEXT_VALUE_KIND, b"sound")),
    )
    .unwrap();
    assert_eq!(
        current_value(&draft.face(&basis()).unwrap(), "creche.search"),
        b"sound"
    );
}

#[test]
fn empty_edit_and_refused_values_preserve_source_availability_and_contract() {
    let mut draft = draft();
    apply(&mut draft, "creche.name", Some((UTF8_TEXT_VALUE_KIND, b""))).unwrap();
    let empty = draft.face(&basis()).unwrap();
    assert_eq!(current_value(&empty, "creche.name"), b"");
    assert!(
        !empty
            .actions
            .iter()
            .find(|a| a.identity == "creche.birth")
            .unwrap()
            .availability
            .is_available()
    );
    assert!(readout(&draft).contains("InvalidName"));
    let revision = draft.revision();
    assert_eq!(
        input(
            &draft,
            "creche.naming",
            Some((UTF8_TEXT_VALUE_KIND, b"not-a-tradition"))
        ),
        Err(FaceInteractionRefusal::ViolatedConstraint)
    );
    let too_long = "É".repeat(crate::names::MAX_FRIENDLY_NAME_BYTES / 2 + 1);
    assert!(
        input(
            &draft,
            "creche.name",
            Some((UTF8_TEXT_VALUE_KIND, too_long.as_bytes()))
        )
        .is_err()
    );
    assert_eq!(draft.revision(), revision);
    let unchanged = draft.face(&basis()).unwrap();
    assert_eq!(unchanged, empty);
    assert_eq!(current_value(&unchanged, "creche.naming"), b"surprise");
}
