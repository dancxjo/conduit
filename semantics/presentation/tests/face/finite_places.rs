//! #5355 finite Commons-place domain fixture; not the #5359 Commons product.
use super::*;
use conduit_presentation::{
    plan_face_utterances, PresentationRelationship, PresentationRelationshipKind,
};

const PLACES: [(&str, &str); 3] = [
    ("commons/hub", "Hub"),
    ("commons/workshop", "Workshop"),
    ("commons/creche", "Crèche"),
];

#[test]
fn finite_place_fixture_preserves_named_containment_and_bounded_agency() {
    let checked = CheckedPlotId::from("checked/commons-domain-fixture");
    let source = SourceDocumentId::from("source/commons-domain-fixture");
    let body = Body::born(
        source.clone(),
        checked.clone(),
        1,
        SignId::from("sign/commons-fixture-born"),
    )
    .unwrap();
    let (body, wake) = body
        .wake(2, SignId::from("sign/commons-fixture-awake"))
        .unwrap();
    let plan = seal_plan(
        PlotIdentity {
            source_document_id: source,
            checked_plot_id: checked.clone(),
            expanded_plot_id: ExpandedPlotId::from("expanded/commons-domain-fixture"),
        },
        vec![],
    );
    let wake = wake
        .plan_ready(&plan, SignId::from("sign/commons-fixture-plan"))
        .unwrap();
    let play = bind_active_play(
        &plan.plan_id,
        &"host/commons-fixture".into(),
        &"boot/commons-fixture".into(),
        3,
    );
    let wake = wake
        .play_started(&play, SignId::from("sign/commons-fixture-play"))
        .unwrap();
    let fragment = PresentationFragment {
        basis: PresentationContributionBasis {
            checked_plot_id: checked.clone(),
            plan_id: plan.plan_id.clone(),
            active_play_id: play.active_play_id.clone(),
            required_interaction_context: None,
        },
        subjects: PLACES
            .iter()
            .map(|(identity, name)| PresentationSubject {
                identity: (*identity).into(),
                name: (*name).into(),
                role: PresentationRole::Semantic(kind_id("commons/place")),
            })
            .collect(),
        relationships: PLACES[1..]
            .iter()
            .map(|(identity, _)| PresentationRelationship {
                source: "commons/hub".into(),
                target: (*identity).into(),
                kind: PresentationRelationshipKind::Semantic(kind_id("commons/contains-place")),
            })
            .collect(),
        composition: vec![],
        properties: vec![],
        text: vec![],
        actions: PLACES
            .iter()
            .map(|(identity, name)| PresentationAction {
                identity: format!("{identity}/inspect"),
                intent: "commons/inspect@1".into(),
                target: (*identity).into(),
                name: format!("Inspect {name}"),
                arguments: vec![],
                disclosure: PresentationDisclosureLevel::CurrentAction,
                availability: PresentationActionAvailability::Available,
            })
            .collect(),
        disclosures: PLACES
            .iter()
            .map(|(identity, _)| PresentationDisclosure {
                subject: (*identity).into(),
                level: PresentationDisclosureLevel::Primary,
            })
            .collect(),
        temporal_references: vec![],
        temporal_facts: vec![],
    };
    let body_before = serde_json::to_value(&body).unwrap();
    let face = Face::project(
        &body,
        Some(&wake),
        1,
        FaceContext::ResidentPlot(checked),
        FaceFocus::Contribution {
            role: FaceContributionRole::Foreground,
            node_key: Some("commons/hub".into()),
        },
        vec![FaceContribution::from_presentation(
            FaceContributionRole::Foreground,
            fragment,
        )],
    )
    .unwrap()
    .presentation;
    let specimen: conduit_presentation::Presentation = serde_json::from_str(include_str!(
        "../../../../proof/conformance/presentation-waist/domain-derived/finite-place-face.json"
    ))
    .unwrap();
    assert_eq!(
        specimen, face,
        "renderer specimen differs from finite domain truth"
    );
    let linear = render_linear_presentation(&face).unwrap();
    let aural = plan_face_utterances(&face).unwrap();
    assert_eq!(
        face.subjects
            .iter()
            .filter(|subject| subject.identity.starts_with("commons/"))
            .count(),
        3
    );
    assert_eq!(
        face.relationships
            .iter()
            .filter(|relation| relation.source == "commons/hub")
            .count(),
        2
    );
    for (identity, name) in PLACES {
        assert!(linear
            .lines
            .iter()
            .any(|line| line.contains(identity) && line.contains(name)));
        assert!(aural.clauses.iter().any(|clause| clause.provenance
            == conduit_presentation::FaceUtteranceProvenance::subject(identity.into()).unwrap()
            && clause.text.contains(name)));
        assert!(face
            .actions
            .iter()
            .any(|action| action.identity == format!("{identity}/inspect")
                && action.target == identity
                && action.arguments.is_empty()));
    }
    assert!(
        !face
            .relationships
            .iter()
            .any(|relation| relation.kind == PresentationRelationshipKind::Connects),
        "containment does not invent physical or authorized connectivity"
    );
    assert_eq!(serde_json::to_value(&body).unwrap(), body_before);
    assert_eq!(linear.presentation_id, face.identity);
    assert_eq!(aural.source_face_identity, face.identity.as_str());
}
