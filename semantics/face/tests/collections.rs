//! Portable canonical contract proof; acknowledgement below is a simulated Mask fixture.
#[cfg(feature = "plot-catalog")]
mod common;
#[path = "collections/fixtures.rs"]
mod fixtures;
#[cfg(feature = "plot-catalog")]
#[path = "collections/proposals.rs"]
mod proposals;
use conduit_face::*;
use fixtures::*;

#[test]
fn ordered_pages_preserve_all_three_profiles_without_changing_source() {
    for mode in [
        None,
        Some(ChoiceMultiplicity::Exclusive),
        Some(ChoiceMultiplicity::Independent),
    ] {
        let face = specimen(mode, 3);
        let before = face.clone();
        let prepared = prepare_face_collection(&face, "list", 64).unwrap();
        assert_eq!(prepared.source(), &face);
        assert_eq!(prepared.subject().identity, "list");
        assert_eq!(prepared.multiplicity(), mode);
        let first = prepared.page(0, 2).unwrap();
        assert_eq!(
            first
                .items
                .iter()
                .map(|item| item.identity.as_str())
                .collect::<Vec<_>>(),
            ["opaque/3", "opaque/2"]
        );
        assert_eq!(first.total, 3);
        assert_eq!(first.next, Some(2));
        assert_eq!(first.face_identity, face.identity.as_str());
        assert_eq!(first.revision, face.revision);
        assert_eq!(first.collection_identity, "list");
        let last = prepared.page(2, 64).unwrap();
        assert_eq!(last.items[0].identity, "opaque/1");
        assert_eq!(last.next, None);
        assert!(prepared.page(3, 1).unwrap().items.is_empty());
        assert_eq!(face, before);
    }
}

#[test]
fn empty_singleton_and_full_capacity_pages_are_explicit_and_bounded() {
    for mode in [None, Some(ChoiceMultiplicity::Independent)] {
        for count in [0, 1, 64] {
            let face = specimen(mode, count);
            let collection = prepare_face_collection(&face, "list", 64).unwrap();
            assert_eq!(collection.items().len(), count);
            assert_eq!(collection.page(0, 64).unwrap().total, count);
            assert!(collection.page(count, 64).unwrap().items.is_empty());
            for (first, maximum) in [(count + 1, 1), (usize::MAX, 1), (0, 0), (0, 65)] {
                assert!(matches!(
                    collection.page(first, maximum),
                    Err(FaceCollectionRefusal::Capacity)
                ));
            }
        }
    }
    let face = specimen(None, 2);
    for maximum in [0, 1, 65] {
        assert!(matches!(
            prepare_face_collection(&face, "list", maximum),
            Err(FaceCollectionRefusal::Capacity)
        ));
    }
}

#[test]
fn missing_members_ambiguous_facts_and_order_do_not_make_prepared_views() {
    for case in 0..5 {
        let mut face = specimen(None, 2);
        let expected = match case {
            0 => {
                face.relationships.pop();
                FaceCollectionRefusal::Membership
            }
            1 => {
                face.relationships.push(face.relationships[0].clone());
                FaceCollectionRefusal::Membership
            }
            2 => {
                change(&mut face, "opaque/1", "order", FacePropertyValue::Count(0));
                FaceCollectionRefusal::Order
            }
            3 => {
                face.properties
                    .push(property("list", "item-count", FacePropertyValue::Count(2)));
                FaceCollectionRefusal::AmbiguousFact
            }
            _ => {
                change(
                    &mut face,
                    "list",
                    "item-count",
                    FacePropertyValue::Text("2".into()),
                );
                FaceCollectionRefusal::Type
            }
        };
        let face = rebuild(face);
        assert_eq!(
            prepare_face_collection(&face, "list", 64).unwrap_err(),
            expected
        );
    }
    let mut face = specimen(None, 1);
    face.revision += 1; // Unrebound serialized source must not gain a valid token.
    assert!(matches!(
        prepare_face_collection(&face, "list", 64),
        Err(FaceCollectionRefusal::InvalidFace)
    ));
}

#[test]
fn exclusive_complete_domain_and_independent_reversibility_share_recipe_validation() {
    let mut exclusive = specimen(Some(ChoiceMultiplicity::Exclusive), 2);
    exclusive.relationships.retain(|r| r.target != "opaque/1");
    change(
        &mut exclusive,
        "list",
        "item-count",
        FacePropertyValue::Count(1),
    );
    assert!(matches!(
        prepare_face_collection(&rebuild(exclusive), "list", 64),
        Err(FaceCollectionRefusal::Membership)
    ));
    let mut independent = specimen(Some(ChoiceMultiplicity::Independent), 2);
    independent.actions[0].arguments[0].contract = conduit_core::CheckedValueContract::new(
        "value/bool".into(),
        1,
        vec![conduit_core::ValueConstraint::CanonicalMembership {
            members: vec![vec![1]],
            negated: false,
        }],
    )
    .unwrap();
    assert!(matches!(
        prepare_face_collection(&rebuild(independent), "list", 64),
        Err(FaceCollectionRefusal::Action)
    ));
    let mut independent = specimen(Some(ChoiceMultiplicity::Independent), 2);
    independent.properties.push(property(
        "list",
        "minimum-selected-count",
        FacePropertyValue::Count(0),
    ));
    assert!(matches!(
        prepare_face_collection(&rebuild(independent), "list", 64),
        Err(FaceCollectionRefusal::UnsupportedProfile)
    ));
}

#[test]
fn filtered_inventory_extremes_preserve_exact_count_bounds() {
    let mut face = specimen(Some(ChoiceMultiplicity::Independent), 2);
    face.properties.extend([
        property(
            "list",
            "inventory-count",
            FacePropertyValue::Count(u64::MAX),
        ),
        property(
            "list",
            "selected-count",
            FacePropertyValue::Count(u64::MAX - 1),
        ),
    ]);
    let face = rebuild(face);
    assert!(prepare_face_collection(&face, "list", 64).is_ok());
    let mut invalid = face;
    change(
        &mut invalid,
        "list",
        "selected-count",
        FacePropertyValue::Count(u64::MAX),
    );
    assert!(matches!(
        prepare_face_collection(&rebuild(invalid), "list", 64),
        Err(FaceCollectionRefusal::Selection)
    ));
}
