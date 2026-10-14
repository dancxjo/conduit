use conduit_face::{
    Face, FaceAction, FaceActionAvailability, FaceAspect, FaceBasis, FaceCursor, FaceDepth,
    FaceDisclosureLevel, FaceNavigation, FacePlace, FaceProjection, FaceProperty,
    FacePropertyValue, FaceRelationship, FaceRelationshipKind, FaceRole, FaceSubject, FaceText,
    NavigationAspect, NavigationFollow, NavigationPlace, ProjectionItem, ProjectionMembership,
    ProjectionRefusal, MAX_PROJECTION_MEMBERSHIPS,
};

fn subject(identity: &str, role: FaceRole) -> FaceSubject {
    FaceSubject {
        identity: identity.into(),
        role,
        name: format!("{identity} subject"),
    }
}

fn fixture() -> (Face, FaceNavigation) {
    let face = Face::new_with_semantics(
        9,
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
        vec![
            subject("entrance", FaceRole::Document),
            subject("plot/text-lab", FaceRole::Plot),
            subject("program", FaceRole::Plot),
            subject("gear/upper", FaceRole::Gear),
            subject("cord/text", FaceRole::Cord),
            subject("body", FaceRole::Body),
            subject("host/local", FaceRole::Host),
            subject("line/browser", FaceRole::Line),
        ],
        vec![
            FaceRelationship {
                source: "gear/upper".into(),
                target: "host/local".into(),
                kind: FaceRelationshipKind::Realizes,
            },
            FaceRelationship {
                source: "cord/text".into(),
                target: "line/browser".into(),
                kind: FaceRelationshipKind::Realizes,
            },
        ],
        vec![
            FaceProperty {
                subject: "gear/upper".into(),
                name: "implementation".into(),
                value: FacePropertyValue::Identity("impl/text-upper".into()),
            },
            FaceProperty {
                subject: "host/local".into(),
                name: "plan-participation".into(),
                value: FacePropertyValue::Flag(true),
            },
        ],
        vec![FaceText {
            subject: "plot/text-lab".into(),
            text: "Keyboard to uppercase text".into(),
        }],
        vec![FaceAction {
            identity: "action/birth".into(),
            intent: "conduit.intent/birth@1".into(),
            target: "program".into(),
            name: "Birth".into(),
            arguments: vec![],
            disclosure: FaceDisclosureLevel::CurrentAction,
            availability: FaceActionAvailability::Available,
        }],
        vec![],
    )
    .unwrap();

    let navigation = FaceNavigation::new(
        &face,
        vec![
            NavigationPlace {
                place: FacePlace::Entrance,
                root_subject: "entrance".into(),
                name: "Entrance".into(),
                aspects: vec![NavigationAspect {
                    aspect: FaceAspect::Structure,
                    focusable_subjects: vec!["entrance".into(), "plot/text-lab".into()],
                }],
            },
            NavigationPlace {
                place: FacePlace::Program,
                root_subject: "program".into(),
                name: "Program".into(),
                aspects: vec![
                    NavigationAspect {
                        aspect: FaceAspect::Structure,
                        focusable_subjects: vec![
                            "program".into(),
                            "gear/upper".into(),
                            "cord/text".into(),
                        ],
                    },
                    NavigationAspect {
                        aspect: FaceAspect::Plan,
                        focusable_subjects: vec!["gear/upper".into(), "cord/text".into()],
                    },
                ],
            },
            NavigationPlace {
                place: FacePlace::Body,
                root_subject: "body".into(),
                name: "Body".into(),
                aspects: vec![
                    NavigationAspect {
                        aspect: FaceAspect::Structure,
                        focusable_subjects: vec![
                            "body".into(),
                            "host/local".into(),
                            "line/browser".into(),
                        ],
                    },
                    NavigationAspect {
                        aspect: FaceAspect::Plan,
                        focusable_subjects: vec!["host/local".into(), "line/browser".into()],
                    },
                ],
            },
        ],
        vec![
            NavigationFollow {
                identity: "follow/gear-host".into(),
                source_subject: "gear/upper".into(),
                relationship: FaceRelationshipKind::Realizes,
                target_subject: "host/local".into(),
                target_place: FacePlace::Body,
                target_aspect: FaceAspect::Plan,
            },
            NavigationFollow {
                identity: "follow/cord-line".into(),
                source_subject: "cord/text".into(),
                relationship: FaceRelationshipKind::Realizes,
                target_subject: "line/browser".into(),
                target_place: FacePlace::Body,
                target_aspect: FaceAspect::Plan,
            },
        ],
    )
    .unwrap();
    (face, navigation)
}

fn membership(
    place: FacePlace,
    aspect: FaceAspect,
    item: ProjectionItem,
    depth: FaceDepth,
) -> ProjectionMembership {
    ProjectionMembership {
        place,
        aspect,
        item,
        depth,
    }
}

fn memberships() -> Vec<ProjectionMembership> {
    use FaceAspect::{Plan, Structure};
    use FaceDepth::{Context, Detail, Exact, Primary};
    use FacePlace::{Body, Entrance, Program};
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
            ProjectionItem::Subject("plot/text-lab".into()),
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
    face: &Face,
    navigation: &FaceNavigation,
    place: FacePlace,
    aspect: FaceAspect,
    depth: FaceDepth,
) -> FaceCursor {
    FaceCursor {
        face: face.identity.clone(),
        navigation: navigation.identity.clone(),
        revision: face.revision,
        place,
        aspect,
        focus: None,
        depth,
    }
}

#[test]
fn place_and_aspect_project_different_subject_sets_without_graph_fusion() {
    let (face, navigation) = fixture();
    let before = face.clone();
    let projection = FaceProjection::new(&face, &navigation, memberships()).unwrap();

    let program = projection
        .project(
            &face,
            &navigation,
            &cursor(
                &face,
                &navigation,
                FacePlace::Program,
                FaceAspect::Structure,
                FaceDepth::Primary,
            ),
        )
        .unwrap();
    let body = projection
        .project(
            &face,
            &navigation,
            &cursor(
                &face,
                &navigation,
                FacePlace::Body,
                FaceAspect::Structure,
                FaceDepth::Primary,
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
    assert_eq!(face, before);
    assert_eq!(face.identity, before.identity);
}

#[test]
fn depth_reveals_exact_content_without_deleting_canonical_truth() {
    let (face, navigation) = fixture();
    let projection = FaceProjection::new(&face, &navigation, memberships()).unwrap();
    let primary = projection
        .project(
            &face,
            &navigation,
            &cursor(
                &face,
                &navigation,
                FacePlace::Body,
                FaceAspect::Plan,
                FaceDepth::Primary,
            ),
        )
        .unwrap();
    let exact = projection
        .project(
            &face,
            &navigation,
            &cursor(
                &face,
                &navigation,
                FacePlace::Body,
                FaceAspect::Plan,
                FaceDepth::Exact,
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
    assert_eq!(face.relationships[1].source, "cord/text");
    assert_eq!(face.relationships[1].target, "line/browser");
}

#[test]
fn action_and_plan_facts_are_scoped_without_changing_their_exact_truth() {
    let (face, navigation) = fixture();
    let projection = FaceProjection::new(&face, &navigation, memberships()).unwrap();
    let entrance = projection
        .project(
            &face,
            &navigation,
            &cursor(
                &face,
                &navigation,
                FacePlace::Entrance,
                FaceAspect::Structure,
                FaceDepth::Exact,
            ),
        )
        .unwrap();
    let program = projection
        .project(
            &face,
            &navigation,
            &cursor(
                &face,
                &navigation,
                FacePlace::Program,
                FaceAspect::Structure,
                FaceDepth::Primary,
            ),
        )
        .unwrap();
    let body_plan = projection
        .project(
            &face,
            &navigation,
            &cursor(
                &face,
                &navigation,
                FacePlace::Body,
                FaceAspect::Plan,
                FaceDepth::Detail,
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
        face.actions[0].availability,
        FaceActionAvailability::Available
    );
}

#[test]
fn identical_truth_is_deterministic_and_changes_are_identity_bearing() {
    let (face, navigation) = fixture();
    let first = FaceProjection::new(&face, &navigation, memberships()).unwrap();
    let repeat = FaceProjection::new(&face, &navigation, memberships()).unwrap();
    assert_eq!(first.identity, repeat.identity);

    let mut changed = memberships();
    changed[0].depth = FaceDepth::Exact;
    let changed = FaceProjection::new(&face, &navigation, changed).unwrap();
    assert_ne!(first.identity, changed.identity);
}

#[test]
fn stale_unknown_duplicate_and_missing_follow_truth_fail_closed() {
    let (face, navigation) = fixture();
    let projection = FaceProjection::new(&face, &navigation, memberships()).unwrap();
    let mut stale_cursor = cursor(
        &face,
        &navigation,
        FacePlace::Program,
        FaceAspect::Structure,
        FaceDepth::Primary,
    );
    stale_cursor.revision += 1;
    assert_eq!(
        projection.project(&face, &navigation, &stale_cursor),
        Err(ProjectionRefusal::StaleFace)
    );

    let mut duplicate = memberships();
    duplicate.push(duplicate[0].clone());
    assert_eq!(
        FaceProjection::new(&face, &navigation, duplicate),
        Err(ProjectionRefusal::DuplicateMembership)
    );

    let mut unknown = memberships();
    unknown.push(membership(
        FacePlace::Entrance,
        FaceAspect::Structure,
        ProjectionItem::Text(99),
        FaceDepth::Exact,
    ));
    assert_eq!(
        FaceProjection::new(&face, &navigation, unknown),
        Err(ProjectionRefusal::UnknownItem)
    );

    let without_follow = memberships()
        .into_iter()
        .filter(|entry| !matches!(entry.item, ProjectionItem::Relationship(_)))
        .collect();
    assert_eq!(
        FaceProjection::new(&face, &navigation, without_follow),
        Err(ProjectionRefusal::MissingFollowRelationship)
    );

    let excessive = vec![memberships()[0].clone(); MAX_PROJECTION_MEMBERSHIPS + 1];
    assert_eq!(
        FaceProjection::new(&face, &navigation, excessive),
        Err(ProjectionRefusal::TooManyMemberships)
    );
}
