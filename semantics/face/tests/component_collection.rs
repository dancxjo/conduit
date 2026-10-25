//! Checked structural selection never infers collection meaning from a role.
use conduit_core::{kind_id, CheckedValueContract, StructuredInfoValue, ValueConstraint};
use conduit_face::*;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};

fn recipe(mode: &str, maximum: u16) -> StructuredInfoValue {
    ComponentRecipeInfo::new(
        24,
        "test/collection@1".into(),
        1,
        280,
        BoundedSequence::try_from_iter([ComponentRegionInfo::new(
            1,
            "selection".into(),
            1,
            ComponentSubjectSelector::collection(maximum, mode.into()).unwrap(),
        )
        .unwrap()])
        .unwrap(),
        1,
    )
    .unwrap()
    .into_structured()
    .unwrap()
}
fn property(subject: &str, name: &str, value: FacePropertyValue) -> FaceProperty {
    FaceProperty {
        subject: subject.into(),
        name: name.into(),
        value,
    }
}
fn rebuild(face: Face) -> Face {
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
fn specimen() -> Face {
    let subjects = [
        ("review", FaceRole::Collection),
        ("choices", FaceRole::Collection),
        ("names", FaceRole::Collection),
        ("one", FaceRole::Item),
        ("two", FaceRole::Item),
    ]
    .into_iter()
    .map(|(id, role)| FaceSubject {
        identity: id.into(),
        role,
        name: id.into(),
    })
    .collect();
    let relationships = ["one", "two"]
        .into_iter()
        .map(|id| FaceRelationship {
            source: "choices".into(),
            target: id.into(),
            kind: FaceRelationshipKind::Contains,
        })
        .collect();
    let mut facts = vec![
        property("review", "item-count", FacePropertyValue::Count(0)),
        property(
            "names",
            "choice-multiplicity",
            FacePropertyValue::Text("exclusive".into()),
        ),
        property(
            "choices",
            "choice-multiplicity",
            FacePropertyValue::Text("independent".into()),
        ),
        property("choices", "item-count", FacePropertyValue::Count(2)),
        property("choices", "inventory-count", FacePropertyValue::Count(3)),
        property("choices", "selected-count", FacePropertyValue::Count(2)),
    ];
    let mut actions = vec![];
    for (index, id) in ["one", "two"].into_iter().enumerate() {
        facts.extend([
            property(id, "order", FacePropertyValue::Count((10 - index) as u64)),
            property(id, "selected", FacePropertyValue::Flag(index == 0)),
            property(
                id,
                "boolean-change-action",
                FacePropertyValue::Identity(format!("toggle/{id}")),
            ),
        ]);
        actions.push(FaceAction {
            identity: format!("toggle/{id}"),
            intent: "test/replace".into(),
            target: id.into(),
            name: format!("Change {id}"),
            arguments: vec![FaceActionArgument {
                name: "value".into(),
                value_name: "Selected".into(),
                contract: CheckedValueContract::new(
                    kind_id("value/bool"),
                    1,
                    vec![ValueConstraint::CanonicalMembership {
                        members: vec![vec![0], vec![1]],
                        negated: false,
                    }],
                )
                .unwrap(),
            }],
            disclosure: FaceDisclosureLevel::CurrentAction,
            availability: FaceActionAvailability::Available,
        });
    }
    Face::new_with_semantics(
        1,
        FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: None,
            checked_plot_id: None,
            expanded_plot_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: vec![],
        },
        subjects,
        relationships,
        facts,
        vec![],
        actions,
        vec![],
    )
    .unwrap()
}
fn change(face: &mut Face, subject: &str, name: &str, value: FacePropertyValue) {
    face.properties
        .iter_mut()
        .find(|p| p.subject == subject && p.name == name)
        .unwrap()
        .value = value;
}
fn refusal(face: Face, expected: CollectionFactsRefusal) {
    assert_eq!(
        prepare_component_recipe(recipe("independent", 2), &rebuild(face))
            .err()
            .unwrap(),
        ComponentRecipeRefusal::Collection(expected)
    );
}
#[test]
fn typed_selector_selects_only_matching_collection_and_preserves_full_reading() {
    let face = specimen();
    let before = face.clone();
    let prepared = prepare_component_recipe(recipe("independent", 2), &face).unwrap();
    assert_eq!(
        prepared.subject_regions,
        vec![None, Some(0), None, None, None]
    );
    assert_eq!(prepared.reading, plan_face_utterances(&face).unwrap());
    assert_eq!(face, before);
    let information = prepare_component_recipe(recipe("information", 2), &face).unwrap();
    assert_eq!(
        information.subject_regions,
        vec![Some(0), None, None, None, None]
    );
}
#[test]
fn legacy_role_without_typed_mode_remains_complete_generic_reading() {
    let mut face = specimen();
    face.properties.retain(|p| p.name != "choice-multiplicity");
    let face = rebuild(face);
    let prepared = prepare_component_recipe(recipe("independent", 2), &face).unwrap();
    assert!(prepared.subject_regions.iter().all(Option::is_none));
    assert_eq!(prepared.reading, plan_face_utterances(&face).unwrap());
}
#[test]
fn declared_capacity_and_exact_membership_are_checked() {
    assert_eq!(
        prepare_component_recipe(recipe("independent", 1), &specimen())
            .err()
            .unwrap(),
        ComponentRecipeRefusal::Collection(CollectionFactsRefusal::Capacity)
    );
    let mut face = specimen();
    change(
        &mut face,
        "choices",
        "item-count",
        FacePropertyValue::Count(1),
    );
    refusal(face, CollectionFactsRefusal::Membership);
    let mut face = specimen();
    face.relationships.pop();
    refusal(face, CollectionFactsRefusal::Membership);
}
#[test]
fn duplicate_or_wrong_typed_facts_and_order_are_refused() {
    let mut face = specimen();
    face.properties.push(property(
        "choices",
        "item-count",
        FacePropertyValue::Count(2),
    ));
    refusal(face, CollectionFactsRefusal::AmbiguousFact);
    let mut face = specimen();
    change(&mut face, "one", "order", FacePropertyValue::Count(9));
    refusal(face, CollectionFactsRefusal::Order);
    let mut face = specimen();
    change(
        &mut face,
        "one",
        "order",
        FacePropertyValue::Text("first".into()),
    );
    refusal(face, CollectionFactsRefusal::Type);
    let mut face = specimen();
    face.properties
        .retain(|p| !(p.subject == "one" && p.name == "order"));
    refusal(face, CollectionFactsRefusal::MissingFact);
}
#[test]
fn independent_inventory_selected_bounds_are_exact_and_overflow_safe() {
    for total in [0, 3] {
        let mut face = specimen();
        change(
            &mut face,
            "choices",
            "selected-count",
            FacePropertyValue::Count(total),
        );
        refusal(face, CollectionFactsRefusal::Selection);
    }
    let mut face = specimen();
    change(
        &mut face,
        "choices",
        "inventory-count",
        FacePropertyValue::Count(1),
    );
    refusal(face, CollectionFactsRefusal::Membership);
    let mut face = specimen();
    change(
        &mut face,
        "choices",
        "inventory-count",
        FacePropertyValue::Count(u64::MAX),
    );
    change(
        &mut face,
        "choices",
        "selected-count",
        FacePropertyValue::Count(u64::MAX - 1),
    );
    prepare_component_recipe(recipe("independent", 2), &rebuild(face)).unwrap();
    let mut face = specimen();
    face.properties.push(property(
        "choices",
        "maximum-selected-count",
        FacePropertyValue::Count(1),
    ));
    refusal(face, CollectionFactsRefusal::UnsupportedProfile);
}
#[test]
fn independent_action_requires_exact_source_association_and_both_boolean_replacements() {
    let mut face = specimen();
    change(
        &mut face,
        "one",
        "boolean-change-action",
        FacePropertyValue::Identity("toggle/two".into()),
    );
    refusal(face, CollectionFactsRefusal::Action);
    let mut face = specimen();
    face.actions[0].arguments[0].contract = CheckedValueContract::new(
        kind_id("value/bool"),
        1,
        vec![ValueConstraint::CanonicalMembership {
            members: vec![vec![1]],
            negated: false,
        }],
    )
    .unwrap();
    refusal(face, CollectionFactsRefusal::Action);
}
#[test]
fn selector_type_carries_finite_modes_and_item_bound() {
    assert!(ComponentSubjectSelector::collection(2, "role-guess".into()).is_err());
    for n in [0, 65] {
        assert!(ComponentSubjectSelector::collection(n, "independent".into()).is_err());
    }
    for n in [1, 64] {
        let selector = ComponentSubjectSelector::collection(n, "independent".into()).unwrap();
        let encoded = selector.clone().into_structured().unwrap();
        assert_eq!(
            ComponentSubjectSelector::from_structured(encoded).unwrap(),
            selector
        );
    }
}

#[test]
fn unknown_source_mode_is_not_silently_treated_as_a_mode_mismatch() {
    let mut face = specimen();
    change(
        &mut face,
        "choices",
        "choice-multiplicity",
        FacePropertyValue::Text("unknown".into()),
    );
    refusal(face, CollectionFactsRefusal::UnsupportedProfile);
}

#[test]
fn information_selector_requires_canonical_facts_even_when_mode_is_absent() {
    let mut face = specimen();
    face.properties.retain(|p| p.subject != "review");
    assert_eq!(
        prepare_component_recipe(recipe("information", 2), &rebuild(face))
            .err()
            .unwrap(),
        ComponentRecipeRefusal::Collection(CollectionFactsRefusal::MissingFact)
    );
}

fn exclusive_specimen() -> Face {
    let mut face = specimen();
    face.properties
        .retain(|p| !(p.subject == "names" && p.name == "choice-multiplicity"));
    change(
        &mut face,
        "choices",
        "choice-multiplicity",
        FacePropertyValue::Text("exclusive".into()),
    );
    let contract = face.actions[0].arguments[0].contract.clone();
    face.subjects.push(FaceSubject {
        identity: "source".into(),
        role: FaceRole::TextEntry,
        name: "Choice".into(),
    });
    let mut action = face.actions[0].clone();
    action.identity = "replace/choice".into();
    action.target = "source".into();
    face.actions.push(action);
    face.properties.extend([
        property(
            "choices",
            "minimum-selected-count",
            FacePropertyValue::Count(1),
        ),
        property(
            "choices",
            "maximum-selected-count",
            FacePropertyValue::Count(1),
        ),
        property(
            "choices",
            "selection-action",
            FacePropertyValue::Identity("replace/choice".into()),
        ),
        property(
            "choices",
            "value-source",
            FacePropertyValue::Identity("source".into()),
        ),
        property(
            "source",
            "value-change-action",
            FacePropertyValue::Identity("replace/choice".into()),
        ),
        property(
            "source",
            "current-value",
            FacePropertyValue::TypedValue {
                contract: contract.clone(),
                bytes: vec![0],
            },
        ),
        property(
            "one",
            "choice-value",
            FacePropertyValue::TypedValue {
                contract: contract.clone(),
                bytes: vec![0],
            },
        ),
        property(
            "two",
            "choice-value",
            FacePropertyValue::TypedValue {
                contract,
                bytes: vec![1],
            },
        ),
    ]);
    rebuild(face)
}
#[test]
fn exclusive_selection_requires_complete_domain_and_current_value_agreement() {
    let face = exclusive_specimen();
    let prepared = prepare_component_recipe(recipe("exclusive", 2), &face).unwrap();
    assert_eq!(
        prepared.subject_regions,
        vec![None, Some(0), None, None, None, None]
    );
    for (subject, name, value, expected) in [
        (
            "two",
            "selected",
            FacePropertyValue::Flag(true),
            CollectionFactsRefusal::Selection,
        ),
        (
            "choices",
            "minimum-selected-count",
            FacePropertyValue::Count(0),
            CollectionFactsRefusal::UnsupportedProfile,
        ),
        (
            "source",
            "value-change-action",
            FacePropertyValue::Identity("toggle/one".into()),
            CollectionFactsRefusal::Action,
        ),
    ] {
        let mut invalid = face.clone();
        change(&mut invalid, subject, name, value);
        assert_eq!(
            prepare_component_recipe(recipe("exclusive", 2), &rebuild(invalid))
                .err()
                .unwrap(),
            ComponentRecipeRefusal::Collection(expected)
        );
    }
    let mut incomplete = face.clone();
    incomplete.relationships.pop();
    change(
        &mut incomplete,
        "choices",
        "item-count",
        FacePropertyValue::Count(1),
    );
    assert_eq!(
        prepare_component_recipe(recipe("exclusive", 2), &rebuild(incomplete))
            .err()
            .unwrap(),
        ComponentRecipeRefusal::Collection(CollectionFactsRefusal::Membership)
    );
}
