//! Component consumption of real source collection facts and typed returns.
use super::*;
use conduit_face::{FaceCollectionRefusal as Refusal, prepare_face_collection};

#[test]
fn page_and_focus_views_preserve_source_and_informational_lists_cannot_select() {
    let draft = draft();
    let before = draft.selection(draft.revision()).unwrap();
    let face = draft.face(&basis()).unwrap();
    let choices = collection(&face, "/name-system/input-details");
    let prepared = prepare_face_collection(&face, &choices.identity, 32).unwrap();
    assert!(core::ptr::eq(prepared.source(), &face));
    let mut encountered = Vec::new();
    let mut first = 0;
    loop {
        let page = prepared.page(first, 4).unwrap();
        assert_eq!(page.face_identity, face.identity.as_str());
        assert_eq!(page.revision, face.revision);
        assert_eq!(page.total, draft.naming_systems().count());
        encountered.extend(page.items.iter().map(|s| s.identity.as_str()));
        let Some(next) = page.next else { break };
        first = next;
    }
    assert_eq!(encountered.len(), draft.naming_systems().count());
    assert_eq!(prepared.page(0, 0).err(), Some(Refusal::Capacity));
    let show = fixtures::acknowledged_fixture_show(&face);
    assert_eq!(
        prepared
            .interaction(&face, &show, "forged-option", true, 1)
            .err(),
        Some(Refusal::UnknownItem)
    );
    let review = collection(&face, "/reviewed-plots");
    let informational = prepare_face_collection(&face, &review.identity, 16).unwrap();
    assert_eq!(informational.multiplicity(), None);
    assert_eq!(
        informational
            .interaction(&face, &show, &informational.items()[0].identity, true, 1)
            .err(),
        Some(Refusal::NoSelectionAction)
    );
    assert_eq!(draft.selection(draft.revision()).unwrap(), before);
}

#[test]
fn exclusive_option_reference_commits_its_source_value_once_and_refuses_stale_membership() {
    let mut draft = draft();
    let face = draft.face(&basis()).unwrap();
    let choices = collection(&face, "/name-system/input-details");
    let prepared = prepare_face_collection(&face, &choices.identity, 32).unwrap();
    let item = *prepared.items().last().unwrap();
    let FacePropertyValue::TypedValue { bytes, .. } =
        property(&face, &item.identity, "choice-value")
    else {
        panic!("source option value")
    };
    let id = core::str::from_utf8(bytes).unwrap();
    let show = fixtures::acknowledged_fixture_show(&face);
    assert_eq!(
        prepared
            .interaction(&face, &show, &item.identity, false, 1)
            .err(),
        Some(Refusal::Selection)
    );
    let input = prepared
        .interaction(&face, &show, &item.identity, true, 1)
        .unwrap();
    assert_eq!(input.action_id, "creche.naming");
    assert_eq!(input.arguments[0].value, *bytes);
    assert_eq!(
        draft.apply_face_interaction(&basis(), &show, &input),
        Ok(BirthActionOutcome::Changed)
    );
    assert_eq!(draft.requested_system(), id);
    let revision = draft.revision();
    let current = draft.face(&basis()).unwrap();
    assert_eq!(
        prepared
            .interaction(&current, &show, &item.identity, true, 2)
            .err(),
        Some(Refusal::StaleFace)
    );
    assert_eq!(
        draft.apply_face_interaction(&basis(), &show, &input),
        Err(BirthFaceRefusal::Interaction(
            FaceInteractionRefusal::StaleFace
        ))
    );
    assert_eq!(draft.revision(), revision);
}

#[test]
fn independent_replacement_preserves_disabled_state_and_read_only_empty_review() {
    let mut draft = draft();
    let face = draft.face(&basis()).unwrap();
    let inventory = collection(&face, "/initial-plots");
    let prepared = prepare_face_collection(&face, &inventory.identity, 16).unwrap();
    let show = fixtures::acknowledged_fixture_show(&face);
    assert_eq!(
        prepared
            .interaction(&face, &show, &prepared.items()[1].identity, true, 1)
            .err(),
        Some(Refusal::Interaction(
            FaceInteractionRefusal::UnavailableAction
        ))
    );
    assert_eq!(draft.revision(), face.revision as u32);
    let input = prepared
        .interaction(&face, &show, &prepared.items()[0].identity, false, 1)
        .unwrap();
    assert_eq!(input.arguments[0].value, [0]);
    assert_eq!(
        draft.apply_face_interaction(&basis(), &show, &input),
        Ok(BirthActionOutcome::Changed)
    );
    assert!(draft.choices().iter().all(|choice| !choice.selected));
    let current = draft.face(&basis()).unwrap();
    let review = collection(&current, "/reviewed-plots");
    let empty = prepare_face_collection(&current, &review.identity, 16).unwrap();
    let page = empty.page(0, 1).unwrap();
    assert_eq!(page.total, 0);
    assert!(page.items.is_empty());
    assert_eq!(page.next, None);
}
