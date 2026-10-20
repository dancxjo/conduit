//! Three source-owned collection meanings over the same actual Crèche Face.
use super::*;
use alloc::{borrow::ToOwned, vec::Vec};
use conduit_face::{FacePropertyValue, render_linear_face};

fn property<'a>(face: &'a Face, subject: &str, name: &str) -> &'a FacePropertyValue {
    let mut values = face
        .properties
        .iter()
        .filter(|p| p.subject == subject && p.name == name);
    let value = &values.next().expect("source fact").value;
    assert!(values.next().is_none());
    value
}
fn children<'a>(face: &'a Face, collection: &str) -> Vec<&'a conduit_face::FaceSubject> {
    face.relationships
        .iter()
        .filter(|r| r.source == collection && r.kind == FaceRelationshipKind::Contains)
        .map(|r| {
            face.subjects
                .iter()
                .find(|s| s.identity == r.target)
                .unwrap()
        })
        .collect()
}
fn collection<'a>(face: &'a Face, suffix: &str) -> &'a conduit_face::FaceSubject {
    face.subjects
        .iter()
        .find(|s| s.identity.ends_with(suffix) && s.role == FaceRole::Collection)
        .unwrap()
}

#[test]
fn actual_creche_has_informational_exclusive_and_independent_collections() {
    let draft = draft();
    let before = draft.selection(draft.revision()).unwrap();
    let face = draft.face(&basis()).unwrap();
    let review = collection(&face, "/reviewed-plots");
    assert_eq!(
        property(&face, &review.identity, "item-count"),
        &FacePropertyValue::Count(1)
    );
    assert!(face.actions.iter().all(|a| a.target != review.identity));
    let reviewed = children(&face, &review.identity);
    assert_eq!(reviewed.len(), 1);
    assert_eq!(reviewed[0].name, "Clock");
    assert!(
        face.actions
            .iter()
            .all(|a| a.target != reviewed[0].identity)
    );

    let action = face
        .actions
        .iter()
        .find(|a| a.identity == "creche.naming")
        .unwrap();
    let choices = collection(&face, "/name-system/input-details");
    assert_eq!(
        property(&face, &choices.identity, "selection-action"),
        &FacePropertyValue::Identity(action.identity.clone())
    );
    assert_eq!(
        property(&face, &choices.identity, "value-source"),
        &FacePropertyValue::Identity(action.target.clone())
    );
    let named = children(&face, &choices.identity);
    assert_eq!(named.len(), draft.naming_systems().count());
    assert_eq!(
        property(&face, &choices.identity, "minimum-selected-count"),
        &FacePropertyValue::Count(1)
    );
    assert_eq!(
        property(&face, &choices.identity, "maximum-selected-count"),
        &FacePropertyValue::Count(1)
    );
    assert_eq!(
        named
            .iter()
            .filter(|s| property(&face, &s.identity, "selected") == &FacePropertyValue::Flag(true))
            .count(),
        1
    );
    for ((id, label), item) in draft.naming_systems().zip(named) {
        assert_eq!(item.role, FaceRole::Item);
        assert_eq!(item.name, label);
        let FacePropertyValue::TypedValue { contract, bytes } =
            property(&face, &item.identity, "choice-value")
        else {
            panic!("typed option")
        };
        assert_eq!(contract.value_kind.as_str(), UTF8_TEXT_VALUE_KIND);
        assert_eq!(bytes, id.as_bytes());
        assert!(action.arguments[0].contract.validate(bytes).is_ok());
        assert!(face.actions.iter().all(|a| a.target != item.identity));
    }
    let independent = collection(&face, "/initial-plots");
    assert_eq!(
        property(&face, &independent.identity, "choice-multiplicity"),
        &FacePropertyValue::Text("independent".into())
    );
    assert_eq!(
        property(&face, &independent.identity, "item-count"),
        &FacePropertyValue::Count(2)
    );
    for item in children(&face, &independent.identity) {
        let FacePropertyValue::Identity(id) =
            property(&face, &item.identity, "boolean-change-action")
        else {
            panic!("source association")
        };
        let action = face
            .actions
            .iter()
            .find(|a| a.identity == *id && a.target == item.identity)
            .unwrap();
        assert!(action.arguments[0].contract.validate(&[0]).is_ok());
        assert!(action.arguments[0].contract.validate(&[1]).is_ok());
    }
    assert_eq!(draft.selection(draft.revision()).unwrap(), before);
}

#[test]
fn empty_source_query_result_keeps_inventory_selection_and_informational_review() {
    let mut draft = draft();
    let before = draft.selection(draft.revision()).unwrap().workset;
    draft.search_plots(draft.revision(), "no-match").unwrap();
    let face = draft.face(&basis()).unwrap();
    let inventory = collection(&face, "/initial-plots");
    assert!(children(&face, &inventory.identity).is_empty());
    assert_eq!(
        property(&face, &inventory.identity, "item-count"),
        &FacePropertyValue::Count(0)
    );
    assert_eq!(
        property(&face, &inventory.identity, "inventory-count"),
        &FacePropertyValue::Count(2)
    );
    assert_eq!(
        property(&face, &inventory.identity, "selected-count"),
        &FacePropertyValue::Count(1)
    );
    let review = collection(&face, "/reviewed-plots");
    assert_eq!(children(&face, &review.identity)[0].name, "Clock");
    assert!(
        render_linear_face(&face)
            .unwrap()
            .lines
            .join("\n")
            .contains("Clock")
    );
    assert_eq!(draft.selection(draft.revision()).unwrap().workset, before);
    assert_eq!(
        apply(
            &mut draft,
            "creche.search",
            Some((UTF8_TEXT_VALUE_KIND, b""))
        ),
        Ok(BirthActionOutcome::Changed)
    );
    assert_eq!(
        apply(&mut draft, "creche.plot.0", Some(("value/bool", &[0]))),
        Ok(BirthActionOutcome::Changed)
    );
    let empty = draft.face(&basis()).unwrap();
    let review = collection(&empty, "/reviewed-plots");
    assert!(children(&empty, &review.identity).is_empty());
    assert_eq!(
        property(&empty, &review.identity, "item-count"),
        &FacePropertyValue::Count(0)
    );
}

#[test]
fn naming_selection_uses_ids_and_rejects_invalid_or_replayed_values_without_effects() {
    let mut draft = draft();
    let revision = draft.revision();
    let name = draft.friendly_name().to_owned();
    let system = draft.requested_system().to_owned();
    for (kind, bytes, expected) in [
        (
            UTF8_TEXT_VALUE_KIND,
            b"Surprise me".as_slice(),
            FaceInteractionRefusal::ViolatedConstraint,
        ),
        (
            UTF8_TEXT_VALUE_KIND,
            b"unknown".as_slice(),
            FaceInteractionRefusal::ViolatedConstraint,
        ),
        (
            UTF8_TEXT_VALUE_KIND,
            &[255],
            FaceInteractionRefusal::MalformedEncoding,
        ),
        ("value/bool", &[1], FaceInteractionRefusal::WrongValueKind),
    ] {
        assert_eq!(
            input(&draft, "creche.naming", Some((kind, bytes))).err(),
            Some(expected)
        );
        assert_eq!(draft.revision(), revision);
        assert_eq!(draft.friendly_name(), name);
        assert_eq!(draft.requested_system(), system);
    }
    assert_eq!(
        input(&draft, "creche.naming", None).err(),
        Some(FaceInteractionRefusal::MissingArgument)
    );
    let id = draft
        .naming_systems()
        .find(|(id, _)| *id != "surprise")
        .unwrap()
        .0
        .to_owned();
    let (show, choice) = input(
        &draft,
        "creche.naming",
        Some((UTF8_TEXT_VALUE_KIND, id.as_bytes())),
    )
    .unwrap();
    assert_eq!(
        draft.apply_face_interaction(&basis(), &show, &choice),
        Ok(BirthActionOutcome::Changed)
    );
    assert_eq!(draft.requested_system(), id);
    let revision = draft.revision();
    let committed = draft.friendly_name().to_owned();
    assert_eq!(
        draft.apply_face_interaction(&basis(), &show, &choice),
        Err(BirthFaceRefusal::Interaction(
            FaceInteractionRefusal::StaleFace
        ))
    );
    assert_eq!(draft.revision(), revision);
    assert_eq!(draft.friendly_name(), committed);
}

#[path = "collection_contract_tests.rs"]
mod contract;
#[path = "collection_interaction_tests.rs"]
mod interactions;
#[path = "collection_profile_tests.rs"]
mod profile;

#[path = "collection_reconstruction_tests.rs"]
mod reconstruction;

#[path = "collection_order_tests.rs"]
mod reading_order;
