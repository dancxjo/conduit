use conduit_face::{
    observe_navigation, render_linear_navigation, Face, FaceAction, FaceActionAvailability,
    FaceAspect, FaceBasis, FaceCursor, FaceDepth, FaceDisclosureLevel, FaceNavigation, FacePlace,
    FaceProjection, FaceProperty, FacePropertyValue, FaceRelationship, FaceRelationshipKind,
    FaceRole, FaceSubject, LinearFaceError, NavigationAspect, NavigationFollow,
    NavigationOperation, NavigationPlace, NavigationState, ProjectionItem, ProjectionMembership,
    ProjectionRefusal,
};

fn fixture() -> (Face, FaceNavigation, FaceProjection, FaceCursor) {
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
            sign_ids: Vec::new(),
        },
        vec![
            subject("program", FaceRole::Plot),
            subject("gear/upper", FaceRole::Gear),
            subject("body", FaceRole::Body),
            subject("host/local", FaceRole::Host),
        ],
        vec![FaceRelationship {
            source: "gear/upper".into(),
            target: "host/local".into(),
            kind: FaceRelationshipKind::Realizes,
        }],
        vec![
            FaceProperty {
                subject: "gear/upper".into(),
                name: "kind-id".into(),
                value: FacePropertyValue::Identity("text/upper".into()),
            },
            FaceProperty {
                subject: "host/local".into(),
                name: "boot-id".into(),
                value: FacePropertyValue::Identity("boot/local".into()),
            },
        ],
        Vec::new(),
        vec![FaceAction {
            identity: "action/inspect-upper".into(),
            intent: "conduit.intent/inspect@1".into(),
            target: "gear/upper".into(),
            name: "Inspect Uppercase".into(),
            arguments: vec![],
            disclosure: FaceDisclosureLevel::SelectedDetail,
            availability: FaceActionAvailability::Unavailable {
                reason_code: "authority/not-admitted".into(),
                explanation: "Inspection authority is not admitted".into(),
            },
        }],
        Vec::new(),
    )
    .unwrap();
    let navigation = FaceNavigation::new(
        &face,
        vec![
            NavigationPlace {
                place: FacePlace::Program,
                root_subject: "program".into(),
                name: "Program".into(),
                aspects: vec![NavigationAspect {
                    aspect: FaceAspect::Plan,
                    focusable_subjects: vec!["program".into(), "gear/upper".into()],
                }],
            },
            NavigationPlace {
                place: FacePlace::Body,
                root_subject: "body".into(),
                name: "Body".into(),
                aspects: vec![NavigationAspect {
                    aspect: FaceAspect::Plan,
                    focusable_subjects: vec!["body".into(), "host/local".into()],
                }],
            },
        ],
        vec![NavigationFollow {
            identity: "follow/gear/upper/host/local".into(),
            source_subject: "gear/upper".into(),
            relationship: FaceRelationshipKind::Realizes,
            target_subject: "host/local".into(),
            target_place: FacePlace::Body,
            target_aspect: FaceAspect::Plan,
        }],
    )
    .unwrap();
    let projection = FaceProjection::new(
        &face,
        &navigation,
        vec![
            membership(
                FacePlace::Program,
                ProjectionItem::Subject("program".into()),
                FaceDepth::Primary,
            ),
            membership(
                FacePlace::Program,
                ProjectionItem::Subject("gear/upper".into()),
                FaceDepth::Primary,
            ),
            membership(
                FacePlace::Program,
                ProjectionItem::Relationship(0),
                FaceDepth::Primary,
            ),
            membership(
                FacePlace::Program,
                ProjectionItem::Property(0),
                FaceDepth::Detail,
            ),
            membership(
                FacePlace::Program,
                ProjectionItem::Action("action/inspect-upper".into()),
                FaceDepth::Detail,
            ),
            membership(
                FacePlace::Body,
                ProjectionItem::Subject("body".into()),
                FaceDepth::Primary,
            ),
            membership(
                FacePlace::Body,
                ProjectionItem::Subject("host/local".into()),
                FaceDepth::Primary,
            ),
            membership(
                FacePlace::Body,
                ProjectionItem::Property(1),
                FaceDepth::Exact,
            ),
        ],
    )
    .unwrap();
    let cursor = FaceCursor {
        face: face.identity.clone(),
        navigation: navigation.identity.clone(),
        revision: face.revision,
        place: FacePlace::Program,
        aspect: FaceAspect::Plan,
        focus: Some("gear/upper".into()),
        depth: FaceDepth::Detail,
    };
    (face, navigation, projection, cursor)
}

#[test]
fn linear_manifestation_is_scoped_and_exposes_portable_navigation() {
    let (face, navigation, projection, cursor) = fixture();
    let observation = observe_navigation(&face, &navigation, &projection, &cursor).unwrap();
    let output = render_linear_navigation(&face, &navigation, &projection, &cursor)
        .unwrap()
        .lines
        .join("\n");

    assert!(output.contains("CURSOR place=Program aspect=Plan focus=gear/upper depth=Detail"));
    assert!(output.contains("AVAILABLE PLACE Program"));
    assert!(output.contains("AVAILABLE PLACE Body"));
    assert!(output.contains("AVAILABLE ASPECT Plan"));
    assert!(output.contains("AVAILABLE FOLLOW"));
    assert!(output.contains("id=\"gear/upper\""));
    assert!(output.contains("name=\"kind-id\""));
    assert!(!output.contains("id=\"host/local\""));
    assert!(!output.contains("name=\"boot-id\""));
    assert_eq!(observation.cursor, cursor);
    assert_eq!(observation.available_places, navigation.places);
    assert_eq!(observation.available_aspects, navigation.places[0].aspects);
    assert_eq!(
        observation
            .projected_subjects
            .iter()
            .map(|subject| subject.identity.as_str())
            .collect::<Vec<_>>(),
        vec!["gear/upper", "program"]
    );
    assert_eq!(observation.current_follows.len(), 1);
    assert_eq!(observation.projected_actions.len(), 1);
    assert!(matches!(
        observation.projected_actions[0].availability,
        FaceActionAvailability::Unavailable { .. }
    ));
    assert!(output.contains("action/inspect-upper"));
    assert!(output.contains("authority/not-admitted"));
    assert!(observation
        .projected_subjects
        .iter()
        .all(|subject| output.contains(&format!("id={:?}", subject.identity))));
}

#[test]
fn follow_changes_only_cursor_and_linear_domain() {
    let (face, navigation, projection, cursor) = fixture();
    let before_identity = face.identity.clone();
    let before_basis = face.basis.clone();
    let mut state = NavigationState::new(&navigation, cursor, 4).unwrap();
    let followed = state
        .navigate(
            &face,
            &navigation,
            face.revision,
            NavigationOperation::Follow("follow/gear/upper/host/local".into()),
        )
        .unwrap();
    let output = render_linear_navigation(&face, &navigation, &projection, followed)
        .unwrap()
        .lines
        .join("\n");

    assert!(output.contains("CURSOR place=Body aspect=Plan focus=host/local depth=Detail"));
    assert!(output.contains("id=\"host/local\""));
    assert!(!output.contains("id=\"gear/upper\""));
    assert_eq!(face.identity, before_identity);
    assert_eq!(face.basis, before_basis);
}

#[test]
fn stale_cursor_refuses_before_any_linear_output() {
    let (face, navigation, projection, mut cursor) = fixture();
    cursor.revision += 1;
    assert_eq!(
        render_linear_navigation(&face, &navigation, &projection, &cursor),
        Err(LinearFaceError::InvalidProjection(
            ProjectionRefusal::StaleFace
        ))
    );
}

fn subject(identity: &str, role: FaceRole) -> FaceSubject {
    FaceSubject {
        identity: identity.into(),
        role,
        name: identity.into(),
    }
}

fn membership(place: FacePlace, item: ProjectionItem, depth: FaceDepth) -> ProjectionMembership {
    ProjectionMembership {
        place,
        aspect: FaceAspect::Plan,
        item,
        depth,
    }
}
