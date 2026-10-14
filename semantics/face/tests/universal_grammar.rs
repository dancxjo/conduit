use conduit_core::{
    kind_id, BoundedResourceRef, ResourceClassId, ResourceExtent, ResourceLifetime,
    ResourceSemanticIdentity, ResourceVersionIdentity,
};
use conduit_face::{
    render_linear_face, Face, FaceAction, FaceActionAvailability, FaceAspect, FaceBasis,
    FaceCompositionKind, FaceCompositionRelation, FaceContextBasis, FaceCursor, FaceDepth,
    FaceDisclosure, FaceDisclosureLevel, FaceInteractionContext, FaceNavigation, FacePlace,
    FaceProjection, FaceProperty, FacePropertyValue, FaceRelationship, FaceRelationshipKind,
    FaceRole, FaceSubject, GenerativeNarratorRole, GenerativePresenterBounds,
    GenerativePresenterPolicy, GenerativePresenterRequest, NavigationAspect, NavigationPlace,
    ProjectionItem, ProjectionMembership,
};

fn subject(identity: &str, role: &str, name: &str) -> FaceSubject {
    FaceSubject {
        identity: identity.into(),
        role: FaceRole::Semantic(kind_id(role)),
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

fn lesson_in(context: &str) -> Face {
    Face::new_with_semantics(
        3,
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
            subject("lesson/cell", "education/lesson", "The cell"),
            subject("concept/membrane", "biology/cell/membrane", "Cell membrane"),
            subject(
                "concept/mitochondrion",
                "biology/cell/organelle",
                "Mitochondrion",
            ),
        ],
        vec![FaceRelationship {
            source: "concept/membrane".into(),
            target: "concept/mitochondrion".into(),
            kind: FaceRelationshipKind::Semantic(kind_id("education/lesson/precedes")),
        }],
        vec![FaceProperty {
            subject: "concept/mitochondrion".into(),
            name: "diagram".into(),
            value: FacePropertyValue::Content(diagram_reference()),
        }],
        vec![],
        vec![FaceAction {
            identity: "lesson/answer".into(),
            intent: "education/answer@1".into(),
            target: "concept/mitochondrion".into(),
            name: "Answer".into(),
            arguments: vec![conduit_face::FaceActionArgument::text(
                "lesson/answer-text".into(),
                "Answer".into(),
                1,
                128,
            )
            .unwrap()],
            disclosure: FaceDisclosureLevel::CurrentAction,
            availability: FaceActionAvailability::Available,
        }],
        vec![
            FaceDisclosure {
                subject: "lesson/cell".into(),
                level: FaceDisclosureLevel::Primary,
            },
            FaceDisclosure {
                subject: "concept/mitochondrion".into(),
                level: FaceDisclosureLevel::SelectedDetail,
            },
        ],
    )
    .unwrap()
    .with_composition(vec![
        FaceCompositionRelation {
            identity: "composition/mitochondrion-emphasis".into(),
            source: "concept/mitochondrion".into(),
            target: "concept/membrane".into(),
            kind: FaceCompositionKind::Emphasize,
        },
        FaceCompositionRelation {
            identity: "composition/concepts-juxtaposed".into(),
            source: "concept/membrane".into(),
            target: "concept/mitochondrion".into(),
            kind: FaceCompositionKind::Juxtapose,
        },
    ])
    .unwrap()
    .with_interaction_context(FaceInteractionContext {
        identity: context.into(),
        basis: vec![FaceContextBasis {
            source: "concept/membrane".into(),
            relationship: FaceRelationshipKind::Semantic(kind_id("education/lesson/precedes")),
            target: "concept/mitochondrion".into(),
        }],
    })
    .unwrap()
}

fn lesson() -> Face {
    lesson_in("education/classroom/red-room")
}

#[test]
fn one_open_domain_face_survives_linear_graphical_and_generative_masks() {
    let face = lesson();

    let linear = render_linear_face(&face).unwrap().lines.join("\n");
    assert!(linear.contains("biology/cell/organelle"));
    assert!(linear.contains("education/lesson/precedes"));
    assert!(linear.contains("content:profile=\"biology/diagram@1\""));
    assert!(linear.contains("kind=Emphasize"));
    assert!(linear.contains("kind=Juxtapose"));

    let navigation = FaceNavigation::new(
        &face,
        vec![NavigationPlace {
            place: FacePlace::Program,
            root_subject: "lesson/cell".into(),
            name: "Lesson".into(),
            aspects: vec![NavigationAspect {
                aspect: FaceAspect::Structure,
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
    let projection = FaceProjection::new(
        &face,
        &navigation,
        vec![
            ProjectionMembership {
                place: FacePlace::Program,
                aspect: FaceAspect::Structure,
                item: ProjectionItem::Relationship(0),
                depth: FaceDepth::Context,
            },
            ProjectionMembership {
                place: FacePlace::Program,
                aspect: FaceAspect::Structure,
                item: ProjectionItem::Composition(0),
                depth: FaceDepth::Context,
            },
            ProjectionMembership {
                place: FacePlace::Program,
                aspect: FaceAspect::Structure,
                item: ProjectionItem::Property(0),
                depth: FaceDepth::Detail,
            },
        ],
    )
    .unwrap();
    let projected = projection
        .project(
            &face,
            &navigation,
            &FaceCursor {
                face: face.identity.clone(),
                navigation: navigation.identity.clone(),
                revision: face.revision,
                place: FacePlace::Program,
                aspect: FaceAspect::Structure,
                focus: None,
                depth: FaceDepth::Detail,
            },
        )
        .unwrap();
    assert_eq!(projected.items.len(), 3);

    let request = GenerativePresenterRequest::from_face(
        "request/lesson".into(),
        GenerativePresenterPolicy {
            template_contract_revision: "mask/generative@1".into(),
            narrator_role: GenerativeNarratorRole::TransientFirstPersonBodyNarrator,
            instructions: "Realize only supplied semantic truth".into(),
        },
        face.clone(),
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
    assert_eq!(round_trip.semantic_data.face, face);
}

#[test]
fn malformed_open_identities_and_content_refuse() {
    let mut face = lesson();
    face.subjects[1].role = FaceRole::Semantic(kind_id("organelle"));
    assert!(face.validate().is_err());

    face = lesson();
    face.properties[0].value = FacePropertyValue::Content(vec![0xff]);
    assert!(face.validate().is_err());
}

#[test]
fn interaction_context_is_exact_identity_bearing_truth() {
    let red = lesson_in("education/classroom/red-room");
    let teacher = lesson_in("education/classroom/teacher-overview");
    assert_ne!(red.identity, teacher.identity);

    let red_navigation = FaceNavigation::new(
        &red,
        vec![NavigationPlace {
            place: FacePlace::Program,
            root_subject: "lesson/cell".into(),
            name: "Lesson".into(),
            aspects: vec![NavigationAspect {
                aspect: FaceAspect::Structure,
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
