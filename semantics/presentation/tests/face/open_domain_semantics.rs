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
        presentation: Box::new(PresentationFragment {
            basis: PresentationContributionBasis {
                checked_plot_id: CheckedPlotId::from("checked/tutorial"),
                plan_id,
                active_play_id: play_id,
                required_interaction_context: Some(context_id.into()),
            },
            subjects: vec![
                PresentationSubject {
                    identity: "lesson/cell".into(),
                    role: PresentationRole::Semantic(kind_id("education/lesson")),
                    name: "The cell".into(),
                },
                PresentationSubject {
                    identity: "concept/mitochondrion".into(),
                    role: PresentationRole::Semantic(kind_id("biology/cell/organelle")),
                    name: "Mitochondrion".into(),
                },
            ],
            relationships: vec![conduit_presentation::PresentationRelationship {
                source: "lesson/cell".into(),
                kind: conduit_presentation::PresentationRelationshipKind::Semantic(kind_id(
                    "biology/cell/contains-organelle",
                )),
                target: "concept/mitochondrion".into(),
            }],
            composition: vec![PresentationCompositionRelation {
                identity: "composition/emphasize-mitochondrion".into(),
                source: "concept/mitochondrion".into(),
                target: "lesson/cell".into(),
                kind: PresentationCompositionKind::Emphasize,
            }],
            properties: vec![],
            text: vec![],
            actions: vec![PresentationAction {
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
            disclosures: vec![PresentationDisclosure {
                subject: "concept/mitochondrion".into(),
                level: PresentationDisclosureLevel::Primary,
            }],
            temporal_references: vec![],
            temporal_facts: vec![],
        }),
    };
    let face = Face::project(
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

    let specimen: conduit_presentation::Presentation = serde_json::from_str(include_str!(
        "../../../../proof/conformance/presentation-waist/domain-derived/open-domain-face.json"
    )).unwrap();
    assert_eq!(specimen, face.presentation, "shared renderer specimen differs from its domain-owned projection");
    assert_eq!(face.presentation.interaction_context.identity, context_id);
    let navigation = PresentationNavigation::new(
        &face.presentation,
        vec![NavigationPlace {
            place: PresentationPlace::Program,
            root_subject: "concept/mitochondrion".into(),
            name: "Lesson".into(),
            aspects: vec![NavigationAspect {
                aspect: PresentationAspect::Structure,
                focusable_subjects: vec!["concept/mitochondrion".into()],
            }],
        }],
        vec![],
    )
    .unwrap();
    let projection = PresentationProjection::new(
        &face.presentation,
        &navigation,
        vec![
            ProjectionMembership {
                place: PresentationPlace::Program,
                aspect: PresentationAspect::Structure,
                item: ProjectionItem::Subject("concept/mitochondrion".into()),
                depth: PresentationDepth::Primary,
            },
            ProjectionMembership {
                place: PresentationPlace::Program,
                aspect: PresentationAspect::Structure,
                item: ProjectionItem::Composition(0),
                depth: PresentationDepth::Primary,
            },
        ],
    )
    .unwrap();
    assert_eq!(
        projection
            .project(
                &face.presentation,
                &navigation,
                &PresentationCursor {
                    presentation: face.presentation.identity.clone(),
                    navigation: navigation.identity.clone(),
                    revision: face.presentation.revision,
                    place: PresentationPlace::Program,
                    aspect: PresentationAspect::Structure,
                    focus: None,
                    depth: PresentationDepth::Primary,
                },
            )
            .unwrap()
            .items
            .len(),
        2
    );
    assert!(render_linear_presentation(&face.presentation)
        .unwrap()
        .lines
        .iter()
        .any(|line| line.contains("biology/cell/organelle")));
    assert!(render_linear_presentation(&face.presentation)
        .unwrap()
        .lines
        .iter()
        .any(|line| line.contains("kind=Emphasize")));
    let linear = render_linear_presentation(&face.presentation).unwrap();
    assert!(linear
        .lines
        .iter()
        .any(|line| line.contains("biology/cell/contains-organelle")));
    let aural = conduit_presentation::plan_face_utterances(&face.presentation).unwrap();
    assert_eq!(
        aural.source_face_identity,
        face.presentation.identity.as_str()
    );
    assert_eq!(aural.source_face_revision, face.presentation.revision);
    assert!(aural.clauses.iter().any(|clause|
        matches!(&clause.provenance, conduit_presentation::FaceUtteranceProvenance::Subject(source)
            if source.identity() == "concept/mitochondrion")
        && clause.text.contains("biology/cell/organelle") && clause.text.contains("Mitochondrion")));
    let relationship_index = face
        .presentation
        .relationships
        .iter()
        .position(|relationship| {
            relationship.source == "lesson/cell" && relationship.target == "concept/mitochondrion"
        })
        .unwrap();
    assert!(aural.clauses.iter().any(|clause|
        matches!(&clause.provenance, conduit_presentation::FaceUtteranceProvenance::Relationship(source)
            if *source.index() == relationship_index as u32)
        && clause.text.contains("biology/cell/contains-organelle")));
    assert!(aural.clauses.iter().any(|clause|
        matches!(&clause.provenance, conduit_presentation::FaceUtteranceProvenance::Action(source)
            if source.identity() == "lesson/answer")));
    let generated = GenerativePresenterRequest::from_presentation(
        "request/lesson".into(),
        GenerativePresenterPolicy {
            template_contract_revision: "mask/generative@1".into(),
            narrator_role: GenerativeNarratorRole::TransientFirstPersonBodyNarrator,
            instructions: "Use only supplied truth".into(),
        },
        face.presentation.clone(),
        None,
        GenerativePresenterBounds::reviewed_default(),
    )
    .unwrap();
    assert_eq!(generated.semantic_data.presentation, face.presentation);
}
