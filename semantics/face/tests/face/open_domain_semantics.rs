//! Domain-owned open semantic IDs through the ordinary contribution boundary.
use super::*;

#[test]
fn ordinary_plot_contributes_universal_truth_without_an_application_view() {
    let (body, wake, plan_id, play_id) = playing();
    let context_id = "face/context/tutorial/checked/tutorial/focus/contribution/tutorial/node/concept/mitochondrion";
    let contribution = FaceContribution {
        role: FaceContributionRole::Tutorial,
        checked_plot_id: CheckedPlotId::from("checked/tutorial"),
        plan_id: plan_id.clone(),
        active_play_id: play_id.clone(),
        face: Box::new(FaceFragment {
            basis: FaceContributionBasis {
                checked_plot_id: CheckedPlotId::from("checked/tutorial"),
                plan_id,
                active_play_id: play_id,
                required_interaction_context: Some(context_id.into()),
            },
            subjects: vec![
                FaceSubject {
                    identity: "lesson/cell".into(),
                    role: FaceRole::Semantic(kind_id("education/lesson")),
                    name: "The cell".into(),
                },
                FaceSubject {
                    identity: "concept/mitochondrion".into(),
                    role: FaceRole::Semantic(kind_id("biology/cell/organelle")),
                    name: "Mitochondrion".into(),
                },
            ],
            relationships: vec![conduit_face::FaceRelationship {
                source: "lesson/cell".into(),
                kind: conduit_face::FaceRelationshipKind::Semantic(kind_id(
                    "biology/cell/contains-organelle",
                )),
                target: "concept/mitochondrion".into(),
            }],
            composition: vec![FaceCompositionRelation {
                identity: "composition/emphasize-mitochondrion".into(),
                source: "concept/mitochondrion".into(),
                target: "lesson/cell".into(),
                kind: FaceCompositionKind::Emphasize,
            }],
            properties: vec![],
            text: vec![],
            actions: vec![FaceAction {
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
            disclosures: vec![FaceDisclosure {
                subject: "concept/mitochondrion".into(),
                level: FaceDisclosureLevel::Primary,
            }],
            temporal_references: vec![],
            temporal_facts: vec![],
        }),
    };
    let face = BodyFace::project(
        &body,
        Some(&wake),
        30,
        FaceContext::Tutorial(CheckedPlotId::from("checked/tutorial")),
        FaceFocus::Contribution {
            role: FaceContributionRole::Tutorial,
            node_key: Some("concept/mitochondrion".into()),
        },
        vec![contribution],
    )
    .unwrap();

    let specimen: conduit_face::Face = serde_json::from_str(include_str!(
        "../../../../proof/conformance/face-waist/domain-derived/open-domain-face.json"
    ))
    .unwrap();
    assert_eq!(
        specimen, face.face,
        "shared renderer specimen differs from its domain-owned projection"
    );
    assert_eq!(face.face.interaction_context.identity, context_id);
    let navigation = FaceNavigation::new(
        &face.face,
        vec![NavigationPlace {
            place: FacePlace::Program,
            root_subject: "concept/mitochondrion".into(),
            name: "Lesson".into(),
            aspects: vec![NavigationAspect {
                aspect: FaceAspect::Structure,
                focusable_subjects: vec!["concept/mitochondrion".into()],
            }],
        }],
        vec![],
    )
    .unwrap();
    let projection = FaceProjection::new(
        &face.face,
        &navigation,
        vec![
            ProjectionMembership {
                place: FacePlace::Program,
                aspect: FaceAspect::Structure,
                item: ProjectionItem::Subject("concept/mitochondrion".into()),
                depth: FaceDepth::Primary,
            },
            ProjectionMembership {
                place: FacePlace::Program,
                aspect: FaceAspect::Structure,
                item: ProjectionItem::Composition(0),
                depth: FaceDepth::Primary,
            },
        ],
    )
    .unwrap();
    assert_eq!(
        projection
            .project(
                &face.face,
                &navigation,
                &FaceCursor {
                    face: face.face.identity.clone(),
                    navigation: navigation.identity.clone(),
                    revision: face.face.revision,
                    place: FacePlace::Program,
                    aspect: FaceAspect::Structure,
                    focus: None,
                    depth: FaceDepth::Primary,
                },
            )
            .unwrap()
            .items
            .len(),
        2
    );
    assert!(render_linear_face(&face.face)
        .unwrap()
        .lines
        .iter()
        .any(|line| line.contains("biology/cell/organelle")));
    assert!(render_linear_face(&face.face)
        .unwrap()
        .lines
        .iter()
        .any(|line| line.contains("kind=Emphasize")));
    let linear = render_linear_face(&face.face).unwrap();
    assert!(linear
        .lines
        .iter()
        .any(|line| line.contains("biology/cell/contains-organelle")));
    let aural = conduit_face::plan_face_utterances(&face.face).unwrap();
    assert_eq!(aural.source_face_identity, face.face.identity.as_str());
    assert_eq!(aural.source_face_revision, face.face.revision);
    assert!(aural.clauses.iter().any(
        |clause| matches!(&clause.provenance, conduit_face::FaceUtteranceProvenance::Subject(source)
            if source.identity() == "concept/mitochondrion")
            && clause.text.contains("biology/cell/organelle")
            && clause.text.contains("Mitochondrion")
    ));
    let relationship_index = face
        .face
        .relationships
        .iter()
        .position(|relationship| {
            relationship.source == "lesson/cell" && relationship.target == "concept/mitochondrion"
        })
        .unwrap();
    assert!(aural.clauses.iter().any(|clause|
        matches!(&clause.provenance, conduit_face::FaceUtteranceProvenance::Relationship(source)
            if *source.index() == relationship_index as u32)
        && clause.text.contains("biology/cell/contains-organelle")));
    assert!(aural.clauses.iter().any(
        |clause| matches!(&clause.provenance, conduit_face::FaceUtteranceProvenance::Action(source)
            if source.identity() == "lesson/answer")
    ));
    let generated = GenerativePresenterRequest::from_face(
        "request/lesson".into(),
        GenerativePresenterPolicy {
            template_contract_revision: "mask/generative@1".into(),
            narrator_role: GenerativeNarratorRole::TransientFirstPersonBodyNarrator,
            instructions: "Use only supplied truth".into(),
        },
        face.face.clone(),
        None,
        GenerativePresenterBounds::reviewed_default(),
    )
    .unwrap();
    assert_eq!(generated.semantic_data.face, face.face);
}
