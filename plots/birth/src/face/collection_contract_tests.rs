//! Negative collection-model conformance over a source-derived Face.
//! Altered fixtures are never offered to the actual Birth owner for mutation.
use super::*;
use conduit_face::{FaceCollectionRefusal as Refusal, prepare_face_collection};

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
fn set(face: &mut Face, subject: &str, name: &str, value: FacePropertyValue) {
    face.properties
        .iter_mut()
        .find(|p| p.subject == subject && p.name == name)
        .unwrap()
        .value = value;
}

#[test]
fn malformed_membership_order_and_cardinality_refuse_without_a_proposal() {
    let draft = draft();
    let face = draft.face(&basis()).unwrap();
    let id = collection(&face, "/name-system/input-details")
        .identity
        .clone();
    assert_eq!(
        prepare_face_collection(&face, &id, 16).err(),
        Some(Refusal::Capacity)
    );
    let mut missing = face.clone();
    set(&mut missing, &id, "item-count", FacePropertyValue::Count(0));
    assert_eq!(
        prepare_face_collection(&rehash(missing), &id, 32).err(),
        Some(Refusal::Membership)
    );
    let items = children(&face, &id);
    let mut duplicate_order = face.clone();
    set(
        &mut duplicate_order,
        &items[1].identity,
        "order",
        FacePropertyValue::Count(0),
    );
    assert_eq!(
        prepare_face_collection(&rehash(duplicate_order), &id, 32).err(),
        Some(Refusal::Order)
    );
    let mut cardinality = face.clone();
    set(
        &mut cardinality,
        &items[1].identity,
        "selected",
        FacePropertyValue::Flag(true),
    );
    assert_eq!(
        prepare_face_collection(&rehash(cardinality), &id, 32).err(),
        Some(Refusal::Selection)
    );
    let mut forged = face.clone();
    let FacePropertyValue::TypedValue { contract, .. } =
        property(&face, &items[1].identity, "choice-value")
    else {
        panic!("typed value")
    };
    set(
        &mut forged,
        &items[1].identity,
        "choice-value",
        FacePropertyValue::TypedValue {
            contract: contract.clone(),
            bytes: b"forged-option".to_vec(),
        },
    );
    assert_eq!(
        prepare_face_collection(&rehash(forged), &id, 32).err(),
        Some(Refusal::Membership)
    );
}

#[test]
fn exact_source_association_prevents_action_retargeting_and_one_way_toggles() {
    let draft = draft();
    let face = draft.face(&basis()).unwrap();
    let id = collection(&face, "/name-system/input-details")
        .identity
        .clone();
    let mut forged = face.clone();
    set(
        &mut forged,
        &id,
        "selection-action",
        FacePropertyValue::Identity("creche.name".into()),
    );
    assert_eq!(
        prepare_face_collection(&rehash(forged), &id, 32).err(),
        Some(Refusal::Action)
    );
    let independent = collection(&face, "/initial-plots").identity.clone();
    let mut one_way = face.clone();
    one_way
        .actions
        .iter_mut()
        .find(|a| a.identity == "creche.plot.0")
        .unwrap()
        .arguments
        .clear();
    assert_eq!(
        prepare_face_collection(&rehash(one_way), &independent, 16).err(),
        Some(Refusal::Action)
    );
}

#[test]
fn duplicate_labels_and_reordered_subjects_preserve_ids_source_order_and_values() {
    let draft = draft();
    let original = draft.face(&basis()).unwrap();
    let id = collection(&original, "/name-system/input-details")
        .identity
        .clone();
    let original_items = children(&original, &id);
    let target = original_items.last().unwrap().identity.clone();
    let FacePropertyValue::TypedValue { bytes, .. } = property(&original, &target, "choice-value")
    else {
        panic!("typed option")
    };
    let expected = bytes.clone();
    let mut changed = original.clone();
    for item in changed.subjects.iter_mut().filter(|s| {
        original_items
            .iter()
            .any(|item| item.identity == s.identity)
    }) {
        item.name = "Same label · 東京".into();
    }
    changed.subjects.reverse();
    changed.relationships.reverse();
    let changed = rehash(changed);
    let prepared = prepare_face_collection(&changed, &id, 32).unwrap();
    assert_eq!(
        prepared
            .items()
            .iter()
            .map(|s| &s.identity)
            .collect::<Vec<_>>(),
        original_items
            .iter()
            .map(|s| &s.identity)
            .collect::<Vec<_>>()
    );
    let show = fixtures::acknowledged_fixture_show(&changed);
    let proposal = prepared
        .interaction(&changed, &show, &target, true, 1)
        .unwrap();
    assert_eq!(proposal.arguments[0].value, expected);
    assert_ne!(proposal.arguments[0].value, "Same label · 東京".as_bytes());
    assert_eq!(draft.revision(), original.revision as u32);
}

#[test]
fn independent_profile_refuses_cardinality_limits_and_impossible_inventory_counts() {
    let face = draft().face(&basis()).unwrap();
    let id = collection(&face, "/initial-plots").identity.clone();
    for name in ["minimum-selected-count", "maximum-selected-count"] {
        let mut limited = face.clone();
        limited.properties.push(conduit_face::FaceProperty {
            subject: id.clone(),
            name: name.into(),
            value: FacePropertyValue::Count(1),
        });
        assert_eq!(
            prepare_face_collection(&rehash(limited), &id, 16).err(),
            Some(Refusal::UnsupportedProfile)
        );
    }
    for total in [0, 3] {
        let mut impossible = face.clone();
        set(
            &mut impossible,
            &id,
            "selected-count",
            FacePropertyValue::Count(total),
        );
        assert_eq!(
            prepare_face_collection(&rehash(impossible), &id, 16).err(),
            Some(Refusal::Selection)
        );
    }
}

#[test]
fn exclusive_collection_cannot_hide_an_unselected_checked_alternative() {
    let original = draft().face(&basis()).unwrap();
    let id = collection(&original, "/name-system/input-details")
        .identity
        .clone();
    let omitted = children(&original, &id)
        .into_iter()
        .find(|item| {
            property(&original, &item.identity, "selected") == &FacePropertyValue::Flag(false)
        })
        .unwrap()
        .identity
        .clone();
    let mut incomplete = original.clone();
    incomplete.relationships.retain(|relationship| {
        !(relationship.source == id
            && relationship.kind == FaceRelationshipKind::Contains
            && relationship.target == omitted)
    });
    let count = children(&incomplete, &id).len() as u64;
    set(
        &mut incomplete,
        &id,
        "item-count",
        FacePropertyValue::Count(count),
    );
    // The Face and declared count remain consistent; only checked-domain
    // completeness can detect the silently omitted reachable alternative.
    let incomplete = rehash(incomplete);
    assert_eq!(
        prepare_face_collection(&incomplete, &id, 32).err(),
        Some(Refusal::Membership)
    );
    assert!(prepare_face_collection(&original, &id, 32).is_ok());
}
