use conduit_presentation::{
    NavigationAspect, NavigationFollow, NavigationPlace, Presentation, PresentationAction,
    PresentationActionAvailability, PresentationAspect, PresentationBasis, PresentationCursor,
    PresentationDepth, PresentationDisclosureLevel, PresentationNavigation, PresentationPlace,
    PresentationProjection, PresentationProperty, PresentationPropertyValue,
    PresentationRelationship, PresentationRelationshipKind, PresentationRole, PresentationSubject,
    PresentationText, ProjectionItem, ProjectionMembership, ProjectionRefusal,
    MAX_PROJECTION_MEMBERSHIPS,
};

fn subject(identity: &str, role: PresentationRole) -> PresentationSubject {
    PresentationSubject {
        identity: identity.into(),
        role,
        label: identity.into(),
        accessibility_name: format!("{identity} subject"),
    }
}

fn fixture() -> (Presentation, PresentationNavigation) {
    let presentation = Presentation::new_with_semantics(
        9,
        PresentationBasis {
            seed_id: None,
            body_id: None,
            wake_id: None,
            source_document_id: None,
            checked_form_id: None,
            expanded_form_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: vec![],
        },
        vec![
            subject("entrance", PresentationRole::Document),
            subject("seed/text-lab", PresentationRole::Seed),
            subject("program", PresentationRole::Form),
            subject("gear/upper", PresentationRole::Gear),
            subject("cord/text", PresentationRole::Cord),
            subject("body", PresentationRole::Body),
            subject("host/local", PresentationRole::Host),
            subject("line/browser", PresentationRole::Line),
        ],
        vec![
            PresentationRelationship {
                source: "gear/upper".into(),
                target: "host/local".into(),
                kind: PresentationRelationshipKind::Realizes,
            },
            PresentationRelationship {
                source: "cord/text".into(),
                target: "line/browser".into(),
                kind: PresentationRelationshipKind::Realizes,
            },
        ],
        vec![
            PresentationProperty {
                subject: "gear/upper".into(),
                name: "implementation".into(),
                value: PresentationPropertyValue::Identity("impl/text-upper".into()),
            },
            PresentationProperty {
                subject: "host/local".into(),
                name: "plan-participation".into(),
                value: PresentationPropertyValue::Flag(true),
            },
        ],
        vec![PresentationText {
            subject: "seed/text-lab".into(),
            text: "Keyboard to uppercase text".into(),
        }],
        vec![PresentationAction {
            identity: "action/birth".into(),
            intent: "conduit.intent/birth@1".into(),
            target: "program".into(),
            label: "Birth".into(),
            disclosure: PresentationDisclosureLevel::CurrentAction,
            availability: PresentationActionAvailability::Available,
        }],
        vec![],
    )
    .unwrap();

    let navigation = PresentationNavigation::new(
        &presentation,
        vec![
            NavigationPlace {
                place: PresentationPlace::Entrance,
                root_subject: "entrance".into(),
                label: "Entrance".into(),
                aspects: vec![NavigationAspect {
                    aspect: PresentationAspect::Structure,
                    focusable_subjects: vec!["entrance".into(), "seed/text-lab".into()],
                }],
            },
            NavigationPlace {
                place: PresentationPlace::Program,
                root_subject: "program".into(),
                label: "Program".into(),
                aspects: vec![
                    NavigationAspect {
                        aspect: PresentationAspect::Structure,
                        focusable_subjects: vec![
                            "program".into(),
                            "gear/upper".into(),
                            "cord/text".into(),
                        ],
                    },
                    NavigationAspect {
                        aspect: PresentationAspect::Plan,
                        focusable_subjects: vec!["gear/upper".into(), "cord/text".into()],
                    },
                ],
            },
            NavigationPlace {
                place: PresentationPlace::Body,
                root_subject: "body".into(),
                label: "Body".into(),
                aspects: vec![
                    NavigationAspect {
                        aspect: PresentationAspect::Structure,
                        focusable_subjects: vec![
                            "body".into(),
                            "host/local".into(),
                            "line/browser".into(),
                        ],
                    },
                    NavigationAspect {
                        aspect: PresentationAspect::Plan,
                        focusable_subjects: vec!["host/local".into(), "line/browser".into()],
                    },
                ],
            },
        ],
        vec![
            NavigationFollow {
                identity: "follow/gear-host".into(),
                source_subject: "gear/upper".into(),
                relationship: PresentationRelationshipKind::Realizes,
                target_subject: "host/local".into(),
                target_place: PresentationPlace::Body,
                target_aspect: PresentationAspect::Plan,
            },
            NavigationFollow {
                identity: "follow/cord-line".into(),
                source_subject: "cord/text".into(),
                relationship: PresentationRelationshipKind::Realizes,
                target_subject: "line/browser".into(),
                target_place: PresentationPlace::Body,
                target_aspect: PresentationAspect::Plan,
            },
        ],
    )
    .unwrap();
    (presentation, navigation)
}

fn membership(
    place: PresentationPlace,
    aspect: PresentationAspect,
    item: ProjectionItem,
    depth: PresentationDepth,
) -> ProjectionMembership {
    ProjectionMembership {
        place,
        aspect,
        item,
        depth,
    }
}

fn memberships() -> Vec<ProjectionMembership> {
    use PresentationAspect::{Plan, Structure};
    use PresentationDepth::{Context, Detail, Exact, Primary};
    use PresentationPlace::{Body, Entrance, Program};
    vec![
        membership(
            Entrance,
            Structure,
            ProjectionItem::Subject("entrance".into()),
            Context,
        ),
        membership(
            Entrance,
            Structure,
            ProjectionItem::Subject("seed/text-lab".into()),
            Primary,
        ),
        membership(Entrance, Structure, ProjectionItem::Text(0), Detail),
        membership(
            Program,
            Structure,
            ProjectionItem::Subject("program".into()),
            Primary,
        ),
        membership(
            Program,
            Structure,
            ProjectionItem::Subject("gear/upper".into()),
            Primary,
        ),
        membership(
            Program,
            Structure,
            ProjectionItem::Subject("cord/text".into()),
            Primary,
        ),
        membership(
            Program,
            Structure,
            ProjectionItem::Action("action/birth".into()),
            Primary,
        ),
        membership(
            Program,
            Plan,
            ProjectionItem::Subject("gear/upper".into()),
            Primary,
        ),
        membership(
            Program,
            Plan,
            ProjectionItem::Subject("cord/text".into()),
            Primary,
        ),
        membership(Program, Plan, ProjectionItem::Property(0), Detail),
        membership(Program, Plan, ProjectionItem::Relationship(0), Detail),
        membership(Program, Plan, ProjectionItem::Relationship(1), Detail),
        membership(
            Body,
            Structure,
            ProjectionItem::Subject("body".into()),
            Primary,
        ),
        membership(
            Body,
            Structure,
            ProjectionItem::Subject("host/local".into()),
            Primary,
        ),
        membership(
            Body,
            Structure,
            ProjectionItem::Subject("line/browser".into()),
            Primary,
        ),
        membership(
            Body,
            Plan,
            ProjectionItem::Subject("host/local".into()),
            Primary,
        ),
        membership(
            Body,
            Plan,
            ProjectionItem::Subject("line/browser".into()),
            Primary,
        ),
        membership(Body, Plan, ProjectionItem::Property(1), Detail),
        membership(Body, Plan, ProjectionItem::Relationship(0), Exact),
        membership(Body, Plan, ProjectionItem::Relationship(1), Exact),
    ]
}

fn cursor(
    presentation: &Presentation,
    navigation: &PresentationNavigation,
    place: PresentationPlace,
    aspect: PresentationAspect,
    depth: PresentationDepth,
) -> PresentationCursor {
    PresentationCursor {
        presentation: presentation.identity.clone(),
        navigation: navigation.identity.clone(),
        revision: presentation.revision,
        place,
        aspect,
        focus: None,
        depth,
    }
}

#[test]
fn place_and_aspect_project_different_subject_sets_without_graph_fusion() {
    let (presentation, navigation) = fixture();
    let before = presentation.clone();
    let projection =
        PresentationProjection::new(&presentation, &navigation, memberships()).unwrap();

    let program = projection
        .project(
            &presentation,
            &navigation,
            &cursor(
                &presentation,
                &navigation,
                PresentationPlace::Program,
                PresentationAspect::Structure,
                PresentationDepth::Primary,
            ),
        )
        .unwrap();
    let body = projection
        .project(
            &presentation,
            &navigation,
            &cursor(
                &presentation,
                &navigation,
                PresentationPlace::Body,
                PresentationAspect::Structure,
                PresentationDepth::Primary,
            ),
        )
        .unwrap();

    assert!(program
        .items
        .iter()
        .any(|item| item.item == ProjectionItem::Subject("gear/upper".into())));
    assert!(!program
        .items
        .iter()
        .any(|item| item.item == ProjectionItem::Subject("host/local".into())));
    assert!(body
        .items
        .iter()
        .any(|item| item.item == ProjectionItem::Subject("host/local".into())));
    assert!(!body
        .items
        .iter()
        .any(|item| item.item == ProjectionItem::Subject("gear/upper".into())));
    assert_eq!(presentation, before);
    assert_eq!(presentation.identity, before.identity);
}

#[test]
fn depth_reveals_exact_content_without_deleting_canonical_truth() {
    let (presentation, navigation) = fixture();
    let projection =
        PresentationProjection::new(&presentation, &navigation, memberships()).unwrap();
    let primary = projection
        .project(
            &presentation,
            &navigation,
            &cursor(
                &presentation,
                &navigation,
                PresentationPlace::Body,
                PresentationAspect::Plan,
                PresentationDepth::Primary,
            ),
        )
        .unwrap();
    let exact = projection
        .project(
            &presentation,
            &navigation,
            &cursor(
                &presentation,
                &navigation,
                PresentationPlace::Body,
                PresentationAspect::Plan,
                PresentationDepth::Exact,
            ),
        )
        .unwrap();

    assert!(!primary
        .items
        .iter()
        .any(|item| item.item == ProjectionItem::Relationship(1)));
    assert!(exact
        .items
        .iter()
        .any(|item| item.item == ProjectionItem::Relationship(1)));
    assert_eq!(presentation.relationships[1].source, "cord/text");
    assert_eq!(presentation.relationships[1].target, "line/browser");
}

#[test]
fn action_and_plan_facts_are_scoped_without_changing_their_exact_truth() {
    let (presentation, navigation) = fixture();
    let projection =
        PresentationProjection::new(&presentation, &navigation, memberships()).unwrap();
    let entrance = projection
        .project(
            &presentation,
            &navigation,
            &cursor(
                &presentation,
                &navigation,
                PresentationPlace::Entrance,
                PresentationAspect::Structure,
                PresentationDepth::Exact,
            ),
        )
        .unwrap();
    let program = projection
        .project(
            &presentation,
            &navigation,
            &cursor(
                &presentation,
                &navigation,
                PresentationPlace::Program,
                PresentationAspect::Structure,
                PresentationDepth::Primary,
            ),
        )
        .unwrap();
    let body_plan = projection
        .project(
            &presentation,
            &navigation,
            &cursor(
                &presentation,
                &navigation,
                PresentationPlace::Body,
                PresentationAspect::Plan,
                PresentationDepth::Detail,
            ),
        )
        .unwrap();

    assert!(!entrance
        .items
        .iter()
        .any(|item| matches!(item.item, ProjectionItem::Action(_))));
    assert!(program
        .items
        .iter()
        .any(|item| item.item == ProjectionItem::Action("action/birth".into())));
    assert!(body_plan
        .items
        .iter()
        .any(|item| item.item == ProjectionItem::Property(1)));
    assert_eq!(
        presentation.actions[0].availability,
        PresentationActionAvailability::Available
    );
}

#[test]
fn identical_truth_is_deterministic_and_changes_are_identity_bearing() {
    let (presentation, navigation) = fixture();
    let first = PresentationProjection::new(&presentation, &navigation, memberships()).unwrap();
    let repeat = PresentationProjection::new(&presentation, &navigation, memberships()).unwrap();
    assert_eq!(first.identity, repeat.identity);

    let mut changed = memberships();
    changed[0].depth = PresentationDepth::Exact;
    let changed = PresentationProjection::new(&presentation, &navigation, changed).unwrap();
    assert_ne!(first.identity, changed.identity);
}

#[test]
fn stale_unknown_duplicate_and_missing_follow_truth_fail_closed() {
    let (presentation, navigation) = fixture();
    let projection =
        PresentationProjection::new(&presentation, &navigation, memberships()).unwrap();
    let mut stale_cursor = cursor(
        &presentation,
        &navigation,
        PresentationPlace::Program,
        PresentationAspect::Structure,
        PresentationDepth::Primary,
    );
    stale_cursor.revision += 1;
    assert_eq!(
        projection.project(&presentation, &navigation, &stale_cursor),
        Err(ProjectionRefusal::StalePresentation)
    );

    let mut duplicate = memberships();
    duplicate.push(duplicate[0].clone());
    assert_eq!(
        PresentationProjection::new(&presentation, &navigation, duplicate),
        Err(ProjectionRefusal::DuplicateMembership)
    );

    let mut unknown = memberships();
    unknown.push(membership(
        PresentationPlace::Entrance,
        PresentationAspect::Structure,
        ProjectionItem::Text(99),
        PresentationDepth::Exact,
    ));
    assert_eq!(
        PresentationProjection::new(&presentation, &navigation, unknown),
        Err(ProjectionRefusal::UnknownItem)
    );

    let without_follow = memberships()
        .into_iter()
        .filter(|entry| !matches!(entry.item, ProjectionItem::Relationship(_)))
        .collect();
    assert_eq!(
        PresentationProjection::new(&presentation, &navigation, without_follow),
        Err(ProjectionRefusal::MissingFollowRelationship)
    );

    let excessive = vec![memberships()[0].clone(); MAX_PROJECTION_MEMBERSHIPS + 1];
    assert_eq!(
        PresentationProjection::new(&presentation, &navigation, excessive),
        Err(ProjectionRefusal::TooManyMemberships)
    );
}
