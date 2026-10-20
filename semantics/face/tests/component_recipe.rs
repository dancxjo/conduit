//! Structural preparation proof only; actual renderer consumption is separate.
use conduit_face::*;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};

fn face(items: usize) -> Face {
    face_with_action(items, "test/do", "list")
}

fn face_with_action(items: usize, action_identity: &str, action_target: &str) -> Face {
    let mut subjects = vec![FaceSubject {
        identity: "list".into(),
        role: FaceRole::Collection,
        name: "Things".into(),
    }];
    subjects.extend((0..items).map(|index| FaceSubject {
        identity: format!("item/{index}"),
        role: FaceRole::Item,
        name: format!("Thing {index}"),
    }));
    subjects.push(FaceSubject {
        identity: "unknown".into(),
        role: FaceRole::Semantic(conduit_core::kind_id("biology/cell@1")),
        name: "An open semantic subject".into(),
    });
    Face::new_with_semantics(
        7,
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
        vec![],
        vec![],
        vec![],
        vec![FaceAction {
            identity: action_identity.into(),
            intent: "test/do@1".into(),
            target: action_target.into(),
            name: "Do this".into(),
            arguments: vec![],
            disclosure: FaceDisclosureLevel::CurrentAction,
            availability: FaceActionAvailability::Available,
        }],
        vec![FaceDisclosure {
            subject: "list".into(),
            level: FaceDisclosureLevel::Primary,
        }],
    )
    .unwrap()
}
fn region(
    name: &str,
    selector: ComponentSubjectSelector,
    capacity: u16,
    group: u8,
) -> ComponentRegionInfo {
    ComponentRegionInfo::new(group, name.into(), capacity, selector).unwrap()
}
fn recipe(regions: Vec<ComponentRegionInfo>, columns: u16) -> conduit_core::StructuredInfoValue {
    ComponentRecipeInfo::new(
        24,
        "test/recipe@1".into(),
        columns,
        280,
        BoundedSequence::try_from_iter(regions).unwrap(),
        1,
    )
    .unwrap()
    .into_structured()
    .unwrap()
}
fn regions(group: u8) -> Vec<ComponentRegionInfo> {
    vec![
        region(
            "list",
            ComponentSubjectSelector::role("Collection".into()).unwrap(),
            1,
            0,
        ),
        region(
            "items",
            ComponentSubjectSelector::role("Item".into()).unwrap(),
            20,
            group,
        ),
    ]
}

#[test]
fn action_target_selects_original_subject_without_labels_or_role_assumptions() {
    let face = face_with_action(2, "source/選ぶ", "item/1");
    let before = face.clone();
    let prepared = prepare_component_recipe(
        recipe(
            vec![region(
                "input",
                ComponentSubjectSelector::action_target("source/選ぶ".into()).unwrap(),
                1,
                0,
            )],
            2,
        ),
        &face,
    )
    .unwrap();
    assert_eq!(prepared.subject_regions, vec![None, None, Some(0), None]);
    assert_eq!(prepared.reading, plan_face_utterances(&face).unwrap());
    assert_eq!(face, before);
}

#[test]
fn absent_optional_action_target_preserves_complete_generic_reading() {
    let face = face(2);
    for missing in ["absent/action", "Do this", "list"] {
        let prepared = prepare_component_recipe(
            recipe(
                vec![region(
                    "optional-input",
                    ComponentSubjectSelector::action_target(missing.into()).unwrap(),
                    1,
                    0,
                )],
                2,
            ),
            &face,
        )
        .unwrap();
        assert!(prepared.subject_regions.iter().all(Option::is_none));
        assert_eq!(prepared.reading, plan_face_utterances(&face).unwrap());
    }
}
#[test]
fn different_structure_preserves_complete_original_reading_and_unmatched_meaning() {
    let face = face(20);
    let before = face.clone();
    let plain = prepare_component_recipe(recipe(regions(0), 1), &face).unwrap();
    let grouped = prepare_component_recipe(recipe(regions(1), 2), &face).unwrap();
    assert_eq!(plain.subject_regions, grouped.subject_regions);
    assert_eq!(plain.subject_regions.last(), Some(&None));
    assert_eq!(plain.reading, plan_face_utterances(&face).unwrap());
    assert_eq!(grouped.reading, plain.reading);
    assert_eq!(face, before);
    assert_ne!(plain.recipe, grouped.recipe);
    assert_ne!(
        plain.geometry_for_width(600).unwrap().columns,
        grouped.geometry_for_width(600).unwrap().columns
    );
}
#[test]
fn slots_refuse_overlap_duplicate_regions_and_subject_capacity() {
    let face = face(20);
    let mut overlap = regions(0);
    overlap.push(region(
        "again",
        ComponentSubjectSelector::subject("item/0".into()).unwrap(),
        1,
        1,
    ));
    assert!(matches!(
        prepare_component_recipe(recipe(overlap, 1), &face),
        Err(ComponentRecipeRefusal::OverlappingSlots)
    ));
    let mut duplicate = regions(0);
    duplicate[1] = region(
        "list",
        ComponentSubjectSelector::role("Item".into()).unwrap(),
        20,
        0,
    );
    assert!(matches!(
        prepare_component_recipe(recipe(duplicate, 1), &face),
        Err(ComponentRecipeRefusal::DuplicateRegion)
    ));
    assert!(matches!(
        prepare_component_recipe(recipe(regions(0), 1), &self::face(21)),
        Err(ComponentRecipeRefusal::SlotCapacity)
    ));
}
#[test]
fn geometry_checks_outer_and_inter_column_gutters_before_subtraction() {
    let prepared = prepare_component_recipe(recipe(regions(0), 2), &face(1)).unwrap();
    assert_eq!(
        prepared.geometry_for_width(600).unwrap(),
        ComponentGeometry {
            columns: 2,
            content_width: 552,
            column_width: 264,
            gutter: 24
        }
    );
    assert_eq!(
        prepared.geometry_for_width(320).unwrap(),
        ComponentGeometry {
            columns: 1,
            content_width: 272,
            column_width: 272,
            gutter: 24
        }
    );
    for width in [0, 47, 48] {
        assert_eq!(
            prepared.geometry_for_width(width),
            Err(ComponentRecipeRefusal::Geometry)
        );
    }
    assert!(ComponentRecipeInfo::new(25, "bad".into(), 2, 280, BoundedSequence::new(), 1).is_err());
    assert!(ComponentSubjectSelector::role("UniversalButton".into()).is_err());
}

#[test]
fn interleaved_regions_keep_typed_actions_facts_disclosure_and_reading_order() {
    let base = face(2);
    let mut subjects = base.subjects;
    subjects.insert(
        2,
        FaceSubject {
            identity: "status".into(),
            role: FaceRole::Status,
            name: "Progress".into(),
        },
    );
    let actions = vec![
        FaceAction {
            identity: "add".into(),
            intent: "test/add@1".into(),
            target: "list".into(),
            name: "Add".into(),
            arguments: vec![
                FaceActionArgument::text("text".into(), "Item text".into(), 1, 72).unwrap(),
            ],
            disclosure: FaceDisclosureLevel::CurrentAction,
            availability: FaceActionAvailability::Available,
        },
        FaceAction {
            identity: "toggle".into(),
            intent: "test/toggle@1".into(),
            target: "item/0".into(),
            name: "Complete".into(),
            arguments: vec![],
            disclosure: FaceDisclosureLevel::CurrentAction,
            availability: FaceActionAvailability::Available,
        },
        FaceAction {
            identity: "remove".into(),
            intent: "test/remove@1".into(),
            target: "item/1".into(),
            name: "Remove".into(),
            arguments: vec![],
            disclosure: FaceDisclosureLevel::SelectedDetail,
            availability: FaceActionAvailability::Unavailable {
                reason_code: "no-return".into(),
                explanation: "Return is unavailable".into(),
            },
        },
    ];
    let face = Face::new_with_semantics(
        7,
        base.basis,
        subjects,
        vec![
            FaceRelationship {
                source: "list".into(),
                target: "item/0".into(),
                kind: FaceRelationshipKind::Contains,
            },
            FaceRelationship {
                source: "item/0".into(),
                target: "item/1".into(),
                kind: FaceRelationshipKind::Semantic(conduit_core::kind_id("test/precedes@1")),
            },
        ],
        vec![
            FaceProperty {
                subject: "item/0".into(),
                name: "order".into(),
                value: FacePropertyValue::Count(0),
            },
            FaceProperty {
                subject: "item/1".into(),
                name: "complete".into(),
                value: FacePropertyValue::Flag(true),
            },
        ],
        vec![FaceText {
            subject: "status".into(),
            text: "One thing left".into(),
        }],
        actions,
        vec![
            FaceDisclosure {
                subject: "list".into(),
                level: FaceDisclosureLevel::Primary,
            },
            FaceDisclosure {
                subject: "item/0".into(),
                level: FaceDisclosureLevel::Primary,
            },
            FaceDisclosure {
                subject: "item/1".into(),
                level: FaceDisclosureLevel::SelectedDetail,
            },
        ],
    )
    .unwrap();
    let before = face.clone();
    let mut selected = regions(1);
    selected.push(region(
        "status",
        ComponentSubjectSelector::role("Status".into()).unwrap(),
        1,
        2,
    ));
    let prepared = prepare_component_recipe(recipe(selected, 2), &face).unwrap();
    // Original subjects interleave groups: no sorting/concatenation by region.
    assert_eq!(
        prepared.subject_regions,
        [Some(0), Some(1), Some(2), Some(1), None]
    );
    assert_eq!(prepared.reading, plan_face_utterances(&before).unwrap());
    assert_eq!(face, before);
    assert_eq!(
        face.actions
            .iter()
            .map(|action| action.identity.as_str())
            .collect::<Vec<_>>(),
        ["add", "toggle", "remove"]
    );
    assert_eq!(face.actions[0].arguments[0], before.actions[0].arguments[0]);
    assert!(!face.actions[2].availability.is_available());
}

#[test]
fn valid_face_reading_capacity_is_distinct_from_invalid_semantics() {
    let base = face(1);
    let face = Face::new_with_semantics(
        base.revision,
        base.basis,
        base.subjects,
        base.relationships,
        vec![FaceProperty {
            subject: "list".into(),
            name: "escaped text".into(),
            value: FacePropertyValue::Text("\u{1}".repeat(1024)),
        }],
        base.text,
        base.actions,
        base.disclosures,
    )
    .unwrap();
    face.validate().unwrap();
    assert!(matches!(
        prepare_component_recipe(recipe(regions(0), 1), &face),
        Err(ComponentRecipeRefusal::ReadingCapacity(
            FaceUtterancePlanError::ClauseTooLong
        ))
    ));
}
