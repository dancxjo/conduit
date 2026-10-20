use super::*;
use conduit_face::{FaceDisclosureLevel, FacePropertyValue};

#[test]
fn source_discloses_field_details_and_keeps_filtered_selected_names_available() {
    let mut draft = draft();
    let face = draft.host_owned_face(&host_owned_basis()).unwrap();
    let primary: alloc::vec::Vec<_> = face
        .actions
        .iter()
        .filter(|action| action.disclosure == FaceDisclosureLevel::Primary)
        .collect();
    assert_eq!(primary.len(), 1);
    assert_eq!(primary[0].identity, "creche.birth");
    for action_id in ["creche.name", "creche.naming", "creche.search"] {
        let action = face
            .actions
            .iter()
            .find(|action| action.identity == action_id)
            .unwrap();
        let details = format!("{}/input-details", action.target);
        assert!(
            face.relationships
                .iter()
                .any(|relation| relation.source == action.target
                    && relation.target == details
                    && relation.kind == FaceRelationshipKind::Contains)
        );
        assert!(
            face.disclosures
                .iter()
                .any(|disclosure| disclosure.subject == details
                    && disclosure.level == FaceDisclosureLevel::SelectedDetail)
        );
        assert!(
            face.text
                .iter()
                .any(|text| text.subject == details && text.text.starts_with("Current value: "))
        );
    }
    let selected = face
        .subjects
        .iter()
        .find(|subject| subject.name == "Starting Plot 1")
        .unwrap();
    assert!(
        face.disclosures
            .iter()
            .any(|disclosure| disclosure.subject == selected.identity
                && disclosure.level == FaceDisclosureLevel::SelectedDetail)
    );
    draft.search_plots(draft.revision(), "no-match").unwrap();
    let filtered = draft.host_owned_face(&host_owned_basis()).unwrap();
    assert!(
        filtered
            .subjects
            .iter()
            .any(|subject| subject.identity == selected.identity)
    );
    assert!(
        filtered
            .text
            .iter()
            .any(|text| text.subject == selected.identity && text.text == "Clock")
    );
    assert!(
        !filtered
            .disclosures
            .iter()
            .any(|disclosure| disclosure.subject == selected.identity)
    );
}

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
fn choice_fallback_is_source_disclosed_detail_with_exact_selection_and_action_binding() {
    let draft = draft();
    let face = draft.host_owned_face(&host_owned_basis()).unwrap();
    let utterances = plan_face_utterances(&face).unwrap();
    for (index, choice) in draft.choices().iter().enumerate() {
        let action_id = format!("creche.plot.{index}");
        let action = face
            .actions
            .iter()
            .find(|action| action.identity == action_id)
            .unwrap();
        let subject = face
            .subjects
            .iter()
            .find(|subject| subject.identity == action.target)
            .unwrap();
        assert_eq!(subject.role, FaceRole::Plot);
        assert_eq!(subject.name, choice.title);
        let details_id = format!("{}/selection-details", subject.identity);
        let details = face
            .subjects
            .iter()
            .find(|subject| subject.identity == details_id)
            .unwrap();
        assert_eq!(details.role, FaceRole::Info);
        assert!(
            face.relationships
                .iter()
                .any(|relation| relation.source == subject.identity
                    && relation.target == details_id
                    && relation.kind == FaceRelationshipKind::Contains)
        );
        assert!(
            face.disclosures
                .iter()
                .any(|disclosure| disclosure.subject == details_id
                    && disclosure.level == FaceDisclosureLevel::SelectedDetail)
        );
        let wording = if choice.selected {
            "Included."
        } else {
            "Not included."
        };
        assert!(
            face.text
                .iter()
                .any(|text| text.subject == details_id && text.text == wording)
        );
        assert!(
            !face
                .text
                .iter()
                .any(|text| text.subject == subject.identity && text.text == wording)
        );
        assert!(
            utterances
                .clauses
                .iter()
                .any(|clause| clause.text.contains(wording))
        );
        for (name, expected) in [
            ("selected", FacePropertyValue::Flag(choice.selected)),
            (
                "boolean-change-action",
                FacePropertyValue::Identity(action_id),
            ),
        ] {
            let properties: alloc::vec::Vec<_> = face
                .properties
                .iter()
                .filter(|property| property.subject == subject.identity && property.name == name)
                .collect();
            assert_eq!(properties.len(), 1);
            assert_eq!(properties[0].value, expected);
        }
        assert_eq!(action.arguments.len(), 1);
        assert_eq!(action.arguments[0].name, "value");
        assert_eq!(
            action.arguments[0].contract.value_kind.as_str(),
            "value/bool"
        );
        action.arguments[0].contract.validate(&[0]).unwrap();
        action.arguments[0].contract.validate(&[1]).unwrap();
        assert_eq!(action.availability.is_available(), choice.refusal.is_none());
    }
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

#[test]
fn the_maximum_unicode_name_remains_an_exact_source_value() {
    let mut draft = draft();
    let name = "É".repeat(crate::names::MAX_FRIENDLY_NAME_BYTES / 2);
    apply(
        &mut draft,
        "creche.name",
        Some((UTF8_TEXT_VALUE_KIND, name.as_bytes())),
    )
    .unwrap();
    let face = draft.face(&basis()).unwrap();
    assert_eq!(current_value(&face, "creche.name"), name.as_bytes());
    assert_eq!(draft.friendly_name(), name);
}
