use conduit_core::kind_id;
use conduit_presentation::{
    plan_face_utterances, FaceUtteranceClauseKind, FaceUtteranceProvenance, Presentation,
    PresentationAction, PresentationActionAvailability, PresentationBasis,
    PresentationCompositionKind, PresentationCompositionRelation, PresentationDisclosureLevel,
    PresentationRelationship, PresentationRelationshipKind, PresentationRole, PresentationSubject,
    PresentationText,
};

fn basis() -> PresentationBasis {
    PresentationBasis {
        body_id: None,
        wake_id: None,
        source_document_id: None,
        checked_form_id: None,
        expanded_form_id: None,
        plan_id: None,
        active_play_id: None,
        sign_ids: vec![],
    }
}

fn subject(identity: &str, name: &str, role: &str) -> PresentationSubject {
    PresentationSubject {
        identity: identity.into(),
        name: name.into(),
        role: PresentationRole::Semantic(kind_id(role)),
    }
}

fn source_destination_face() -> Presentation {
    Presentation::new_with_semantics(
        7,
        basis(),
        vec![
            subject("destination", "Backup", "file/location/destination"),
            subject("report", "report.txt", "file/item"),
            subject("source", "Archive", "file/location/source"),
        ],
        vec![
            PresentationRelationship {
                source: "report".into(),
                target: "destination".into(),
                kind: PresentationRelationshipKind::Semantic(kind_id("file/copy-destination")),
            },
            PresentationRelationship {
                source: "source".into(),
                target: "report".into(),
                kind: PresentationRelationshipKind::Contains,
            },
        ],
        vec![],
        vec![PresentationText {
            subject: "report".into(),
            text: "The selected report remains unchanged.".into(),
        }],
        vec![PresentationAction {
            identity: "copy-to-destination".into(),
            intent: "encounter/copy-to-destination".into(),
            target: "report".into(),
            name: "Copy to Backup".into(),
            arguments: vec![],
            disclosure: PresentationDisclosureLevel::CurrentAction,
            availability: PresentationActionAvailability::Available,
        }],
        vec![],
    )
    .unwrap()
    .with_composition(vec![
        PresentationCompositionRelation {
            identity: "composition/source-and-destination".into(),
            source: "source".into(),
            target: "destination".into(),
            kind: PresentationCompositionKind::Juxtapose,
        },
        PresentationCompositionRelation {
            identity: "composition/report-with-source".into(),
            source: "report".into(),
            target: "source".into(),
            kind: PresentationCompositionKind::Group,
        },
    ])
    .unwrap()
}

#[test]
fn source_destination_meaning_becomes_general_provenanced_clauses() {
    let face = source_destination_face();
    let plan = plan_face_utterances(&face).unwrap();

    assert_eq!(plan.source_face_identity, face.identity.as_str());
    assert_eq!(plan.source_face_revision, 7);
    assert_ne!(plan.digest, [0; 32]);
    assert_eq!(
        plan.clauses
            .iter()
            .filter(|clause| clause.kind == FaceUtteranceClauseKind::Subject)
            .map(|clause| clause.text.as_str())
            .collect::<Vec<_>>(),
        [
            "Role file/location/source: Archive.",
            "Role file/location/destination: Backup.",
            "Role file/item: report.txt.",
        ]
    );
    for (expected, provenance) in [
        (
            "Role file/location/source: Archive.",
            FaceUtteranceProvenance::Subject {
                identity: "source".into(),
            },
        ),
        (
            "Archive contains report.txt.",
            FaceUtteranceProvenance::Relationship { index: 1 },
        ),
        (
            "report.txt has relationship file/copy-destination to Backup.",
            FaceUtteranceProvenance::Relationship { index: 0 },
        ),
        (
            "Consider Archive and Backup together.",
            FaceUtteranceProvenance::Composition {
                identity: "composition/source-and-destination".into(),
            },
        ),
        (
            "The selected report remains unchanged.",
            FaceUtteranceProvenance::Text { index: 0 },
        ),
        (
            "Available action: Copy to Backup, for report.txt.",
            FaceUtteranceProvenance::Action {
                identity: "copy-to-destination".into(),
            },
        ),
    ] {
        let clause = plan
            .clauses
            .iter()
            .find(|clause| clause.text == expected)
            .unwrap_or_else(|| panic!("missing derived clause {expected:?}"));
        assert_eq!(clause.provenance, provenance);
    }
}

#[test]
fn semantic_dependencies_precede_identity_and_face_wording_stays_verbatim() {
    let face = Presentation::new(
        1,
        basis(),
        vec![
            subject("alpha", "Alpha", "example/item"),
            subject("zulu", "Zulu", "example/item"),
        ],
        vec![PresentationRelationship {
            source: "zulu".into(),
            target: "alpha".into(),
            kind: PresentationRelationshipKind::Semantic(kind_id("example/precedes")),
        }],
        vec![],
        vec![PresentationText {
            subject: "alpha".into(),
            text: "Keep punctuation: [exact] / untouched!".into(),
        }],
    )
    .unwrap();
    let plan = plan_face_utterances(&face).unwrap();
    let subjects = plan
        .clauses
        .iter()
        .filter(|clause| clause.kind == FaceUtteranceClauseKind::Subject)
        .map(|clause| clause.text.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        subjects,
        ["Role example/item: Zulu.", "Role example/item: Alpha."]
    );
    assert!(plan
        .clauses
        .iter()
        .any(|clause| clause.text == "Keep punctuation: [exact] / untouched!"));
}

#[test]
fn unavailable_and_refused_actions_remain_distinct_without_becoming_invocations() {
    let mut face = Presentation::new_with_semantics(
        1,
        basis(),
        vec![subject("item", "Item", "example/item")],
        vec![],
        vec![],
        vec![],
        vec![
            PresentationAction {
                identity: "later".into(),
                intent: "example/later".into(),
                target: "item".into(),
                name: "Try later".into(),
                arguments: vec![],
                disclosure: PresentationDisclosureLevel::CurrentAction,
                availability: PresentationActionAvailability::Unavailable {
                    reason_code: "example/not-ready".into(),
                    explanation: "The source is still arriving.".into(),
                },
            },
            PresentationAction {
                identity: "forbidden".into(),
                intent: "example/forbidden".into(),
                target: "item".into(),
                name: "Use protected source".into(),
                arguments: vec![],
                disclosure: PresentationDisclosureLevel::CurrentAction,
                availability: PresentationActionAvailability::Refused {
                    reason_code: "example/no-authority".into(),
                    explanation: "Current authority does not permit it.".into(),
                },
            },
        ],
        vec![],
    )
    .unwrap();
    // The constructor derives identity before this deterministic ordering test;
    // rebuild through the public constructor after reversing the source records.
    face.actions.reverse();
    face = Presentation::new_with_semantics(
        face.revision,
        face.basis,
        face.subjects,
        face.relationships,
        face.properties,
        face.text,
        face.actions,
        face.disclosures,
    )
    .unwrap();
    let plan = plan_face_utterances(&face).unwrap();
    let actions = plan
        .clauses
        .iter()
        .filter(|clause| clause.kind == FaceUtteranceClauseKind::Action)
        .map(|clause| clause.text.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        actions,
        [
            "Refused action: Use protected source, for Item. Reason example/no-authority: Current authority does not permit it.",
            "Unavailable action: Try later, for Item. Reason example/not-ready: The source is still arriving.",
        ]
    );
}
