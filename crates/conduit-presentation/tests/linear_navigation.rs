use conduit_presentation::{
    observe_navigation, render_linear_navigation, LinearPresentationError, NavigationAspect,
    NavigationFollow, NavigationOperation, NavigationPlace, NavigationState, Presentation,
    PresentationAction, PresentationActionAvailability, PresentationAspect, PresentationBasis,
    PresentationCursor, PresentationDepth, PresentationDisclosureLevel, PresentationNavigation,
    PresentationPlace, PresentationProjection, PresentationProperty, PresentationPropertyValue,
    PresentationRelationship, PresentationRelationshipKind, PresentationRole, PresentationSubject,
    ProjectionItem, ProjectionMembership, ProjectionRefusal,
};

fn fixture() -> (
    Presentation,
    PresentationNavigation,
    PresentationProjection,
    PresentationCursor,
) {
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
            sign_ids: Vec::new(),
        },
        vec![
            subject("program", PresentationRole::Form),
            subject("gear/upper", PresentationRole::Gear),
            subject("body", PresentationRole::Body),
            subject("host/local", PresentationRole::Host),
        ],
        vec![PresentationRelationship {
            source: "gear/upper".into(),
            target: "host/local".into(),
            kind: PresentationRelationshipKind::Realizes,
        }],
        vec![
            PresentationProperty {
                subject: "gear/upper".into(),
                name: "kind-id".into(),
                value: PresentationPropertyValue::Identity("text/upper".into()),
            },
            PresentationProperty {
                subject: "host/local".into(),
                name: "boot-id".into(),
                value: PresentationPropertyValue::Identity("boot/local".into()),
            },
        ],
        Vec::new(),
        vec![PresentationAction {
            identity: "action/inspect-upper".into(),
            intent: "conduit.intent/inspect@1".into(),
            target: "gear/upper".into(),
            label: "Inspect Uppercase".into(),
            disclosure: PresentationDisclosureLevel::SelectedDetail,
            availability: PresentationActionAvailability::Unavailable {
                reason_code: "authority/not-admitted".into(),
                explanation: "Inspection authority is not admitted".into(),
            },
        }],
        Vec::new(),
    )
    .unwrap();
    let navigation = PresentationNavigation::new(
        &presentation,
        vec![
            NavigationPlace {
                place: PresentationPlace::Program,
                root_subject: "program".into(),
                label: "Program".into(),
                aspects: vec![NavigationAspect {
                    aspect: PresentationAspect::Plan,
                    focusable_subjects: vec!["program".into(), "gear/upper".into()],
                }],
            },
            NavigationPlace {
                place: PresentationPlace::Body,
                root_subject: "body".into(),
                label: "Body".into(),
                aspects: vec![NavigationAspect {
                    aspect: PresentationAspect::Plan,
                    focusable_subjects: vec!["body".into(), "host/local".into()],
                }],
            },
        ],
        vec![NavigationFollow {
            identity: "follow/gear/upper/host/local".into(),
            source_subject: "gear/upper".into(),
            relationship: PresentationRelationshipKind::Realizes,
            target_subject: "host/local".into(),
            target_place: PresentationPlace::Body,
            target_aspect: PresentationAspect::Plan,
        }],
    )
    .unwrap();
    let projection = PresentationProjection::new(
        &presentation,
        &navigation,
        vec![
            membership(
                PresentationPlace::Program,
                ProjectionItem::Subject("program".into()),
                PresentationDepth::Primary,
            ),
            membership(
                PresentationPlace::Program,
                ProjectionItem::Subject("gear/upper".into()),
                PresentationDepth::Primary,
            ),
            membership(
                PresentationPlace::Program,
                ProjectionItem::Relationship(0),
                PresentationDepth::Primary,
            ),
            membership(
                PresentationPlace::Program,
                ProjectionItem::Property(0),
                PresentationDepth::Detail,
            ),
            membership(
                PresentationPlace::Program,
                ProjectionItem::Action("action/inspect-upper".into()),
                PresentationDepth::Detail,
            ),
            membership(
                PresentationPlace::Body,
                ProjectionItem::Subject("body".into()),
                PresentationDepth::Primary,
            ),
            membership(
                PresentationPlace::Body,
                ProjectionItem::Subject("host/local".into()),
                PresentationDepth::Primary,
            ),
            membership(
                PresentationPlace::Body,
                ProjectionItem::Property(1),
                PresentationDepth::Exact,
            ),
        ],
    )
    .unwrap();
    let cursor = PresentationCursor {
        presentation: presentation.identity.clone(),
        navigation: navigation.identity.clone(),
        revision: presentation.revision,
        place: PresentationPlace::Program,
        aspect: PresentationAspect::Plan,
        focus: Some("gear/upper".into()),
        depth: PresentationDepth::Detail,
    };
    (presentation, navigation, projection, cursor)
}

#[test]
fn linear_manifestation_is_scoped_and_exposes_portable_navigation() {
    let (presentation, navigation, projection, cursor) = fixture();
    let observation = observe_navigation(&presentation, &navigation, &projection, &cursor).unwrap();
    let output = render_linear_navigation(&presentation, &navigation, &projection, &cursor)
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
        PresentationActionAvailability::Unavailable { .. }
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
    let (presentation, navigation, projection, cursor) = fixture();
    let before_identity = presentation.identity.clone();
    let before_basis = presentation.basis.clone();
    let mut state = NavigationState::new(&navigation, cursor, 4).unwrap();
    let followed = state
        .navigate(
            &presentation,
            &navigation,
            presentation.revision,
            NavigationOperation::Follow("follow/gear/upper/host/local".into()),
        )
        .unwrap();
    let output = render_linear_navigation(&presentation, &navigation, &projection, followed)
        .unwrap()
        .lines
        .join("\n");

    assert!(output.contains("CURSOR place=Body aspect=Plan focus=host/local depth=Detail"));
    assert!(output.contains("id=\"host/local\""));
    assert!(!output.contains("id=\"gear/upper\""));
    assert_eq!(presentation.identity, before_identity);
    assert_eq!(presentation.basis, before_basis);
}

#[test]
fn stale_cursor_refuses_before_any_linear_output() {
    let (presentation, navigation, projection, mut cursor) = fixture();
    cursor.revision += 1;
    assert_eq!(
        render_linear_navigation(&presentation, &navigation, &projection, &cursor),
        Err(LinearPresentationError::InvalidProjection(
            ProjectionRefusal::StalePresentation
        ))
    );
}

fn subject(identity: &str, role: PresentationRole) -> PresentationSubject {
    PresentationSubject {
        identity: identity.into(),
        role,
        label: identity.into(),
        accessibility_name: identity.into(),
    }
}

fn membership(
    place: PresentationPlace,
    item: ProjectionItem,
    depth: PresentationDepth,
) -> ProjectionMembership {
    ProjectionMembership {
        place,
        aspect: PresentationAspect::Plan,
        item,
        depth,
    }
}
