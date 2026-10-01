use conduit_core::{
    kind_id, BoundedResourceRef, ResourceClassId, ResourceExtent, ResourceLifetime,
    ResourceSemanticIdentity, ResourceVersionIdentity,
};
use conduit_presentation::{
    render_linear_presentation, GenerativeNarratorRole, GenerativePresenterBounds,
    GenerativePresenterPolicy, GenerativePresenterRequest, NavigationAspect, NavigationPlace,
    Presentation, PresentationAction, PresentationActionAvailability, PresentationAspect,
    PresentationBasis, PresentationCompositionKind, PresentationCompositionRelation,
    PresentationContextBasis, PresentationCursor, PresentationDepth, PresentationDisclosure,
    PresentationDisclosureLevel, PresentationInteractionContext, PresentationNavigation,
    PresentationPlace, PresentationProjection, PresentationProperty, PresentationPropertyValue,
    PresentationRelationship, PresentationRelationshipKind, PresentationRole, PresentationSubject,
    ProjectionItem, ProjectionMembership,
};

fn subject(identity: &str, role: &str, name: &str) -> PresentationSubject {
    PresentationSubject {
        identity: identity.into(),
        role: PresentationRole::Semantic(kind_id(role)),
        name: name.into(),
    }
}

fn diagram_reference() -> Vec<u8> {
    BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest([7; 32]),
        content_profile: kind_id("biology/diagram@1"),
        access_class: ResourceClassId::from("content/public@1"),
        extent: ResourceExtent {
            bytes: 4_096,
            items: Some(1),
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([8; 32]),
            expires_at: None,
        },
    }
    .encode()
    .unwrap()
}

fn lesson_in(context: &str) -> Presentation {
    Presentation::new_with_semantics(
        3,
        PresentationBasis {
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
            subject("lesson/cell", "education/lesson", "The cell"),
            subject("concept/membrane", "biology/cell/membrane", "Cell membrane"),
            subject(
                "concept/mitochondrion",
                "biology/cell/organelle",
                "Mitochondrion",
            ),
        ],
        vec![PresentationRelationship {
            source: "concept/membrane".into(),
            target: "concept/mitochondrion".into(),
            kind: PresentationRelationshipKind::Semantic(kind_id("education/lesson/precedes")),
        }],
        vec![PresentationProperty {
            subject: "concept/mitochondrion".into(),
            name: "diagram".into(),
            value: PresentationPropertyValue::Content(diagram_reference()),
        }],
        vec![],
        vec![PresentationAction {
            identity: "lesson/answer".into(),
            intent: "education/answer@1".into(),
            target: "concept/mitochondrion".into(),
            name: "Answer".into(),
            arguments: vec![conduit_presentation::FaceActionArgument::text(
                "lesson/answer-text".into(),
                "Answer".into(),
                1,
                128,
            )
            .unwrap()],
            disclosure: PresentationDisclosureLevel::CurrentAction,
            availability: PresentationActionAvailability::Available,
        }],
        vec![
            PresentationDisclosure {
                subject: "lesson/cell".into(),
                level: PresentationDisclosureLevel::Primary,
            },
            PresentationDisclosure {
                subject: "concept/mitochondrion".into(),
                level: PresentationDisclosureLevel::SelectedDetail,
            },
        ],
    )
    .unwrap()
    .with_composition(vec![
        PresentationCompositionRelation {
            identity: "composition/mitochondrion-emphasis".into(),
            source: "concept/mitochondrion".into(),
            target: "concept/membrane".into(),
            kind: PresentationCompositionKind::Emphasize,
        },
        PresentationCompositionRelation {
            identity: "composition/concepts-juxtaposed".into(),
            source: "concept/membrane".into(),
            target: "concept/mitochondrion".into(),
            kind: PresentationCompositionKind::Juxtapose,
        },
    ])
    .unwrap()
    .with_interaction_context(PresentationInteractionContext {
        identity: context.into(),
        basis: vec![PresentationContextBasis {
            source: "concept/membrane".into(),
            relationship: PresentationRelationshipKind::Semantic(kind_id(
                "education/lesson/precedes",
            )),
            target: "concept/mitochondrion".into(),
        }],
    })
    .unwrap()
}

fn lesson() -> Presentation {
    lesson_in("education/classroom/red-room")
}

#[test]
fn one_open_domain_presentation_survives_linear_graphical_and_generative_masks() {
    let presentation = lesson();

    let linear = render_linear_presentation(&presentation)
        .unwrap()
        .lines
        .join("\n");
    assert!(linear.contains("biology/cell/organelle"));
    assert!(linear.contains("education/lesson/precedes"));
    assert!(linear.contains("content:profile=\"biology/diagram@1\""));
    assert!(linear.contains("kind=Emphasize"));
    assert!(linear.contains("kind=Juxtapose"));

    let navigation = PresentationNavigation::new(
        &presentation,
        vec![NavigationPlace {
            place: PresentationPlace::Program,
            root_subject: "lesson/cell".into(),
            name: "Lesson".into(),
            aspects: vec![NavigationAspect {
                aspect: PresentationAspect::Structure,
                focusable_subjects: vec![
                    "lesson/cell".into(),
                    "concept/membrane".into(),
                    "concept/mitochondrion".into(),
                ],
            }],
        }],
        vec![],
    )
    .unwrap();
    let projection = PresentationProjection::new(
        &presentation,
        &navigation,
        vec![
            ProjectionMembership {
                place: PresentationPlace::Program,
                aspect: PresentationAspect::Structure,
                item: ProjectionItem::Relationship(0),
                depth: PresentationDepth::Context,
            },
            ProjectionMembership {
                place: PresentationPlace::Program,
                aspect: PresentationAspect::Structure,
                item: ProjectionItem::Composition(0),
                depth: PresentationDepth::Context,
            },
            ProjectionMembership {
                place: PresentationPlace::Program,
                aspect: PresentationAspect::Structure,
                item: ProjectionItem::Property(0),
                depth: PresentationDepth::Detail,
            },
        ],
    )
    .unwrap();
    let projected = projection
        .project(
            &presentation,
            &navigation,
            &PresentationCursor {
                presentation: presentation.identity.clone(),
                navigation: navigation.identity.clone(),
                revision: presentation.revision,
                place: PresentationPlace::Program,
                aspect: PresentationAspect::Structure,
                focus: None,
                depth: PresentationDepth::Detail,
            },
        )
        .unwrap();
    assert_eq!(projected.items.len(), 3);

    let request = GenerativePresenterRequest::from_presentation(
        "request/lesson".into(),
        GenerativePresenterPolicy {
            template_contract_revision: "mask/generative@1".into(),
            narrator_role: GenerativeNarratorRole::TransientFirstPersonBodyNarrator,
            instructions: "Realize only supplied semantic truth".into(),
        },
        presentation.clone(),
        None,
        GenerativePresenterBounds::reviewed_default(),
    )
    .unwrap();
    let encoded = serde_json::to_vec(&request).unwrap();
    let wire_text = core::str::from_utf8(&encoded).unwrap();
    assert!(wire_text.contains("\"name\":\"The cell\""));
    assert!(!wire_text.contains("\"label\":"));
    assert!(!wire_text.contains("\"accessibility_name\":"));
    let round_trip: GenerativePresenterRequest = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(round_trip.semantic_data.presentation, presentation);
}

#[test]
fn malformed_open_identities_and_content_refuse() {
    let mut presentation = lesson();
    presentation.subjects[1].role = PresentationRole::Semantic(kind_id("organelle"));
    assert!(presentation.validate().is_err());

    presentation = lesson();
    presentation.properties[0].value = PresentationPropertyValue::Content(vec![0xff]);
    assert!(presentation.validate().is_err());
}

#[test]
fn interaction_context_is_exact_identity_bearing_truth() {
    let red = lesson_in("education/classroom/red-room");
    let teacher = lesson_in("education/classroom/teacher-overview");
    assert_ne!(red.identity, teacher.identity);

    let red_navigation = PresentationNavigation::new(
        &red,
        vec![NavigationPlace {
            place: PresentationPlace::Program,
            root_subject: "lesson/cell".into(),
            name: "Lesson".into(),
            aspects: vec![NavigationAspect {
                aspect: PresentationAspect::Structure,
                focusable_subjects: vec!["concept/mitochondrion".into()],
            }],
        }],
        vec![],
    )
    .unwrap();
    assert!(red_navigation.validate(&teacher).is_err());

    let mut invented = red.clone();
    invented.interaction_context.basis[0].target = "lesson/cell".into();
    assert!(invented.validate().is_err());
}
