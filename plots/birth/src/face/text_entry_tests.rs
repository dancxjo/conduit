//! Atomic text and suggestion composition, retaining real source ownership.
use super::*;
use conduit_face::{
    FaceTextEntryRefusal as Refusal, TextSuggestionPolicy, prepare_face_text_entry,
};

fn field(face: &Face, action: &str) -> String {
    face.actions
        .iter()
        .find(|a| a.identity == action)
        .unwrap()
        .target
        .clone()
}

#[test]
fn preparing_and_browsing_suggestions_never_changes_source_values_or_variation() {
    let draft = draft();
    let before = draft.selection(draft.revision()).unwrap();
    let suggestion = draft.suggestion().clone();
    let face = draft.face(&basis()).unwrap();
    for (action, policy) in [
        ("creche.name", TextSuggestionPolicy::Free),
        ("creche.naming", TextSuggestionPolicy::Fixed),
    ] {
        let prepared = prepare_face_text_entry(&face, &field(&face, action), 32).unwrap();
        assert_eq!(prepared.policy(), Some(policy));
        for item in prepared.suggestions().unwrap().items() {
            let _draft_bytes = prepared.suggestion(&item.identity).unwrap();
        }
        assert_eq!(
            prepared.suggestion("forged-option"),
            Err(Refusal::Suggestion)
        );
    }
    assert_eq!(draft.selection(draft.revision()).unwrap(), before);
    assert_eq!(draft.suggestion(), &suggestion);
}

#[test]
fn fixed_membership_accepts_exact_option_bytes_once_and_never_a_display_label() {
    let mut draft = draft();
    let face = draft.face(&basis()).unwrap();
    let prepared = prepare_face_text_entry(&face, &field(&face, "creche.naming"), 32).unwrap();
    let show = fixtures::acknowledged_fixture_show(&face);
    assert_eq!(
        prepared.submit(&face, &show, b"Surprise me", 1).err(),
        Some(Refusal::InvalidText)
    );
    let last = prepared.suggestions().unwrap().items().last().unwrap();
    let bytes = prepared.suggestion(&last.identity).unwrap();
    let proposal = prepared.submit(&face, &show, bytes, 1).unwrap();
    assert_eq!(
        draft.apply_face_interaction(&basis(), &show, &proposal),
        Ok(BirthActionOutcome::Changed)
    );
    assert_eq!(draft.requested_system().as_bytes(), bytes);
    let current = draft.face(&basis()).unwrap();
    assert_eq!(
        prepared.submit(&current, &show, bytes, 2).err(),
        Some(Refusal::StaleFace)
    );
    assert!(
        draft
            .apply_face_interaction(&basis(), &show, &proposal)
            .is_err()
    );
}

#[test]
fn free_text_uses_the_existing_checked_action_and_keeps_suggestion_generation_unchanged() {
    let mut draft = draft();
    let variation = draft.suggestion().clone();
    let face = draft.face(&basis()).unwrap();
    let prepared = prepare_face_text_entry(&face, &field(&face, "creche.name"), 4).unwrap();
    assert_eq!(prepared.current(), draft.friendly_name());
    let show = fixtures::acknowledged_fixture_show(&face);
    for invalid in [&[0xff][..], &[b'x'; 65][..]] {
        assert_eq!(
            prepared.submit(&face, &show, invalid, 1).err(),
            Some(Refusal::InvalidText)
        );
    }
    let candidate = prepared.suggestions().unwrap().items()[0];
    assert_eq!(
        prepared.suggestion(&candidate.identity).unwrap(),
        variation.name.as_bytes()
    );
    let proposal = prepared
        .submit(&face, &show, "Custom 東京 👋".as_bytes(), 1)
        .unwrap();
    assert_eq!(
        draft.apply_face_interaction(&basis(), &show, &proposal),
        Ok(BirthActionOutcome::Changed)
    );
    assert_eq!(draft.friendly_name(), "Custom 東京 👋");
    assert_eq!(draft.suggestion(), &variation);
    let revision = draft.revision();
    assert!(
        draft
            .apply_face_interaction(&basis(), &show, &proposal)
            .is_err()
    );
    assert_eq!(draft.revision(), revision);
}

fn rehash(face: Face) -> Face {
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
fn ambiguous_or_mismatched_suggestion_associations_refuse_before_proposals() {
    let face = draft().face(&basis()).unwrap();
    let name = field(&face, "creche.name");
    let naming = field(&face, "creche.naming");
    let mut ambiguous = face.clone();
    ambiguous.properties.push(
        face.properties
            .iter()
            .find(|p| p.subject == name && p.name == "suggestion-source")
            .unwrap()
            .clone(),
    );
    assert_eq!(
        prepare_face_text_entry(&rehash(ambiguous), &name, 32).err(),
        Some(Refusal::Binding)
    );
    let mut mismatched = face.clone();
    let source = mismatched
        .properties
        .iter_mut()
        .find(|p| p.subject == naming && p.name == "suggestion-source")
        .unwrap();
    source.value = conduit_face::FacePropertyValue::Identity(format!(
        "{}/name-suggestions",
        name.trim_end_matches("/body-name")
    ));
    assert_eq!(
        prepare_face_text_entry(&rehash(mismatched), &naming, 32).err(),
        Some(Refusal::UnsupportedPolicy)
    );
    let mut unsupported = face.clone();
    unsupported
        .properties
        .iter_mut()
        .find(|p| p.subject == name && p.name == "suggestion-policy")
        .unwrap()
        .value = conduit_face::FacePropertyValue::Text("remote-unbounded".into());
    assert_eq!(
        prepare_face_text_entry(&rehash(unsupported), &name, 32).err(),
        Some(Refusal::UnsupportedPolicy)
    );
}

#[test]
fn disabled_or_retired_show_and_intervening_revision_never_commit_text() {
    let mut draft = draft();
    let face = draft.face(&basis()).unwrap();
    let name = field(&face, "creche.name");
    let prepared = prepare_face_text_entry(&face, &name, 4).unwrap();
    let show = fixtures::acknowledged_fixture_show(&face);
    let mut retired = show.clone();
    retired.show.lifecycle = ManifestationLifecycle::Replaced;
    assert!(matches!(
        prepared.submit(&face, &retired, b"name", 1),
        Err(Refusal::Interaction(_))
    ));
    draft
        .edit_name(draft.revision(), "External revision".into())
        .unwrap();
    let current = draft.face(&basis()).unwrap();
    assert_eq!(
        prepared.submit(&current, &show, b"old draft", 1).err(),
        Some(Refusal::StaleFace)
    );
    let mut disabled = current;
    disabled
        .actions
        .iter_mut()
        .find(|a| a.identity == "creche.name")
        .unwrap()
        .availability = conduit_face::FaceActionAvailability::Unavailable {
        reason_code: "readonly".into(),
        explanation: "Source editing is unavailable".into(),
    };
    let disabled = rehash(disabled);
    let disabled_show = fixtures::acknowledged_fixture_show(&disabled);
    let prepared = prepare_face_text_entry(&disabled, &name, 4).unwrap();
    assert_eq!(prepared.current(), "External revision");
    assert_eq!(
        prepared
            .submit(&disabled, &disabled_show, b"forged", 2)
            .err(),
        Some(Refusal::Interaction(
            FaceInteractionRefusal::UnavailableAction
        ))
    );
    assert_eq!(draft.friendly_name(), "External revision");
}

#[test]
fn option_reorder_and_duplicate_labels_keep_exact_suggestion_identity_and_bytes() {
    let face = draft().face(&basis()).unwrap();
    let naming = field(&face, "creche.naming");
    let prepared = prepare_face_text_entry(&face, &naming, 32).unwrap();
    let item = prepared.suggestions().unwrap().items().last().unwrap();
    let identity = item.identity.clone();
    let expected = prepared.suggestion(&identity).unwrap().to_vec();
    let identities: alloc::vec::Vec<_> = prepared
        .suggestions()
        .unwrap()
        .items()
        .iter()
        .map(|s| s.identity.clone())
        .collect();
    let mut reordered = face.clone();
    for subject in &mut reordered.subjects {
        if identities.contains(&subject.identity) {
            subject.name = "Duplicate 東京".into();
        }
    }
    reordered.subjects.reverse();
    reordered.relationships.reverse();
    let reordered = rehash(reordered);
    let prepared = prepare_face_text_entry(&reordered, &naming, 32).unwrap();
    assert_eq!(prepared.suggestion(&identity).unwrap(), expected);
    let show = fixtures::acknowledged_fixture_show(&reordered);
    assert_eq!(
        prepared
            .submit(&reordered, &show, &expected, 1)
            .unwrap()
            .arguments[0]
            .value,
        expected
    );
}
