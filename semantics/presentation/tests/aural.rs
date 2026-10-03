use conduit_core::{
    kind_id, BaseImplementationId, BoundedResourceRef, CheckedValueContract, IntervalEndpoint,
    Quantity, QuantityUnit, ResourceClassId, ResourceExtent, ResourceLifetime,
    ResourceSemanticIdentity, ResourceVersionIdentity, ValueConstraint, COUNT_ENCODED_LEN,
    COUNT_INFO_ID, DISTANCE_INFO_ID, QUANTITY_ENCODED_LEN, SCALAR_ENCODED_LEN, SCALAR_INFO_ID,
};
use conduit_plot::TextPatternExpression;
use conduit_presentation::{
    plan_face_utterances, FaceActionArgument, FaceUtteranceClauseKind, FaceUtteranceProvenance,
    Presentation, PresentationAction, PresentationActionAvailability, PresentationBasis,
    PresentationCompositionKind, PresentationCompositionRelation, PresentationDisclosureLevel,
    PresentationProperty, PresentationPropertyValue, PresentationRelationship,
    PresentationRelationshipKind, PresentationRole, PresentationSubject, PresentationText,
    UTF8_TEXT_VALUE_KIND,
};

fn basis() -> PresentationBasis {
    PresentationBasis {
        body_id: None,
        wake_id: None,
        source_document_id: None,
        checked_plot_id: None,
        expanded_plot_id: None,
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
            FaceUtteranceProvenance::subject("source".into()).unwrap(),
        ),
        (
            "Archive contains report.txt.",
            FaceUtteranceProvenance::relationship(1).unwrap(),
        ),
        (
            "report.txt has relationship file/copy-destination to Backup.",
            FaceUtteranceProvenance::relationship(0).unwrap(),
        ),
        (
            "Consider Archive and Backup together.",
            FaceUtteranceProvenance::composition("composition/source-and-destination".into())
                .unwrap(),
        ),
        (
            "The selected report remains unchanged.",
            FaceUtteranceProvenance::text(0).unwrap(),
        ),
        (
            "Available action: Copy to Backup, for report.txt.",
            FaceUtteranceProvenance::action("copy-to-destination".into()).unwrap(),
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

fn participation_face(pattern_maximum: u16) -> Presentation {
    let lowercase = TextPatternExpression::Repeat {
        expression: Box::new(TextPatternExpression::ScalarRange {
            first: 'a' as u32,
            last: 'z' as u32,
        }),
        minimum: 1,
        maximum: pattern_maximum,
    }
    .compile(u32::from(pattern_maximum))
    .unwrap();
    let argument = |name: &str, value_name: &str, contract| FaceActionArgument {
        name: name.into(),
        value_name: value_name.into(),
        contract,
    };
    Presentation::new_with_semantics(
        4,
        basis(),
        vec![subject("sample", "Sample", "example/sample")],
        vec![],
        vec![],
        vec![],
        vec![PresentationAction {
            identity: "sample/configure".into(),
            intent: "example/configure".into(),
            target: "sample".into(),
            name: "Configure sample".into(),
            arguments: vec![
                argument(
                    "sample/name",
                    "Lowercase name",
                    CheckedValueContract::new(
                        UTF8_TEXT_VALUE_KIND.into(),
                        u32::from(pattern_maximum),
                        vec![
                            ValueConstraint::ByteLength {
                                minimum: 1,
                                maximum: u32::from(pattern_maximum),
                            },
                            ValueConstraint::TextPattern {
                                pattern: lowercase,
                                anchored_start: true,
                                anchored_end: true,
                                negated: false,
                            },
                        ],
                    )
                    .unwrap(),
                ),
                argument(
                    "sample/count",
                    "Sample count",
                    CheckedValueContract::new(
                        COUNT_INFO_ID.into(),
                        COUNT_ENCODED_LEN as u32,
                        vec![ValueConstraint::UnsignedRange {
                            minimum: Some(2),
                            maximum: Some(4),
                            minimum_endpoint: IntervalEndpoint::Inclusive,
                            maximum_endpoint: IntervalEndpoint::Exclusive,
                        }],
                    )
                    .unwrap(),
                ),
                argument(
                    "sample/offset",
                    "Sample offset",
                    CheckedValueContract::new(
                        SCALAR_INFO_ID.into(),
                        SCALAR_ENCODED_LEN as u32,
                        vec![ValueConstraint::SignedRange {
                            minimum: Some(-2),
                            maximum: Some(2),
                            minimum_endpoint: IntervalEndpoint::Exclusive,
                            maximum_endpoint: IntervalEndpoint::Inclusive,
                        }],
                    )
                    .unwrap(),
                ),
                argument(
                    "sample/distance",
                    "Sampling distance",
                    CheckedValueContract::new(
                        DISTANCE_INFO_ID.into(),
                        QUANTITY_ENCODED_LEN as u32,
                        vec![ValueConstraint::QuantityRange {
                            minimum: Some(Quantity::new(1, QuantityUnit::Meter)),
                            maximum: Some(Quantity::new(2, QuantityUnit::Meter)),
                            minimum_endpoint: IntervalEndpoint::Inclusive,
                            maximum_endpoint: IntervalEndpoint::Inclusive,
                        }],
                    )
                    .unwrap(),
                ),
                argument(
                    "sample/mode",
                    "Sampling mode",
                    CheckedValueContract::new(
                        UTF8_TEXT_VALUE_KIND.into(),
                        8,
                        vec![ValueConstraint::CanonicalMembership {
                            members: vec![b"careful".to_vec(), b"quick".to_vec()],
                            negated: false,
                        }],
                    )
                    .unwrap(),
                ),
            ],
            disclosure: PresentationDisclosureLevel::CurrentAction,
            availability: PresentationActionAvailability::Available,
        }],
        vec![],
    )
    .unwrap()
}

#[test]
fn exact_inward_participation_contract_survives_aural_projection() {
    let face = participation_face(8);
    let plan = plan_face_utterances(&face).unwrap();
    let arguments = plan
        .clauses
        .iter()
        .filter(|clause| clause.kind == FaceUtteranceClauseKind::ActionArgument)
        .collect::<Vec<_>>();

    assert_eq!(arguments.len(), 5);
    for (name, expected) in [
        (
            "sample/name",
            "between 1 and 8 bytes; the exact checked portable text pattern as a whole-value match with 9 states, start state 0, at most 8 characters, and at most 8 match steps",
        ),
        (
            "sample/count",
            "a value from inclusive 2 through exclusive 4",
        ),
        (
            "sample/offset",
            "a value from exclusive -2 through inclusive 2",
        ),
        (
            "sample/distance",
            "a quantity from inclusive 1 length/meter through inclusive 2 length/meter",
        ),
        (
            "sample/mode",
            "one of the canonical values \"careful\", \"quick\"",
        ),
    ] {
        let clause = arguments
            .iter()
            .find(|clause| {
                clause.provenance
                    == FaceUtteranceProvenance::action_argument(
                        "sample/configure".into(),
                        name.into(),
                    )
                    .unwrap()
            })
            .unwrap_or_else(|| panic!("missing aural Face argument {name}"));
        assert!(clause.text.contains(expected), "{}", clause.text);
    }

    let changed = plan_face_utterances(&participation_face(7)).unwrap();
    assert_ne!(plan.source_face_identity, changed.source_face_identity);
    assert_ne!(plan.digest, changed.digest);
}

fn content_reference() -> Vec<u8> {
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

fn property_face(count: u64) -> Presentation {
    Presentation::new_with_semantics(
        2,
        basis(),
        vec![subject("sample", "Sample", "example/sample")],
        vec![],
        vec![
            PresentationProperty {
                subject: "sample".into(),
                name: "identity".into(),
                value: PresentationPropertyValue::Identity("sample/exact@1".into()),
            },
            PresentationProperty {
                subject: "sample".into(),
                name: "implementation".into(),
                value: PresentationPropertyValue::BaseImplementationId(BaseImplementationId::from(
                    "sample/std@1",
                )),
            },
            PresentationProperty {
                subject: "sample".into(),
                name: "description".into(),
                value: PresentationPropertyValue::Text("Retain exact wording".into()),
            },
            PresentationProperty {
                subject: "sample".into(),
                name: "observations".into(),
                value: PresentationPropertyValue::Count(count),
            },
            PresentationProperty {
                subject: "sample".into(),
                name: "offset".into(),
                value: PresentationPropertyValue::Signed(-3),
            },
            PresentationProperty {
                subject: "sample".into(),
                name: "ready".into(),
                value: PresentationPropertyValue::Flag(true),
            },
            PresentationProperty {
                subject: "sample".into(),
                name: "name-contract".into(),
                value: PresentationPropertyValue::ValueContract(
                    CheckedValueContract::new(
                        UTF8_TEXT_VALUE_KIND.into(),
                        12,
                        vec![ValueConstraint::ByteLength {
                            minimum: 1,
                            maximum: 12,
                        }],
                    )
                    .unwrap(),
                ),
            },
            PresentationProperty {
                subject: "sample".into(),
                name: "diagram".into(),
                value: PresentationPropertyValue::Content(content_reference()),
            },
        ],
        vec![],
        vec![],
        vec![],
    )
    .unwrap()
}

#[test]
fn exact_properties_survive_both_linear_and_aural_masks() {
    let face = property_face(4);
    let aural = plan_face_utterances(&face).unwrap();
    let properties = aural
        .clauses
        .iter()
        .filter(|clause| clause.kind == FaceUtteranceClauseKind::Property)
        .collect::<Vec<_>>();
    assert_eq!(properties.len(), face.properties.len());
    for (index, expected) in [
        "identity sample/exact@1",
        "base implementation sample/std@1",
        "text \"Retain exact wording\"",
        "count 4",
        "signed value -3",
        "flag true",
        "a value contract of kind value/text permitting at most 12 bytes: between 1 and 12 bytes",
        "bounded content of profile biology/diagram@1, access class content/public@1, 4096 bytes, and 1 item",
    ]
    .into_iter()
    .enumerate()
    {
        let clause = properties
            .iter()
            .find(|clause| {
                clause.provenance == FaceUtteranceProvenance::property(index as u32).unwrap()
            })
            .unwrap_or_else(|| panic!("missing aural property {index}"));
        assert!(clause.text.contains(expected), "{}", clause.text);
    }

    let linear = conduit_presentation::render_linear_presentation(&face).unwrap();
    assert_eq!(
        linear
            .lines
            .iter()
            .filter(|line| line.starts_with("PROPERTY "))
            .count(),
        face.properties.len()
    );

    let changed = plan_face_utterances(&property_face(5)).unwrap();
    assert_ne!(aural.source_face_identity, changed.source_face_identity);
    assert_ne!(aural.digest, changed.digest);
}

#[test]
fn read_all_includes_disclosure_and_temporal_context_with_exact_provenance() {
    use conduit_presentation::{
        PresentationDisclosure, PresentationTemporalFact, PresentationTemporalRole,
        TemporalInstant, TemporalReference, TemporalScale,
    };
    let instant = |ticks| TemporalInstant {
        ticks,
        scale: TemporalScale::Seconds,
        clock_basis: "clock/scene".into(),
        resolution_ticks: 1,
        uncertainty_ticks: 0,
    };
    let reference = TemporalReference {
        identity: "reference/current".into(),
        instant: instant(20),
    };
    let fact = PresentationTemporalFact::new(
        "event".into(),
        PresentationTemporalRole::Observation,
        None,
        instant(10),
        &reference,
    )
    .unwrap();
    let face = Presentation::new_with_semantics_and_temporal(
        3,
        basis(),
        vec![subject("event", "Arrived", "journey/event")],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![PresentationDisclosure {
            subject: "event".into(),
            level: PresentationDisclosureLevel::Primary,
        }],
        vec![reference],
        vec![fact],
    )
    .unwrap();
    let plan = plan_face_utterances(&face).unwrap();
    for (kind, provenance, wording) in [
        (
            FaceUtteranceClauseKind::Disclosure,
            FaceUtteranceProvenance::disclosure(0).unwrap(),
            "Primary content: Arrived",
        ),
        (
            FaceUtteranceClauseKind::TemporalReference,
            FaceUtteranceProvenance::temporal_reference(0).unwrap(),
            "reference/current is tick 20",
        ),
        (
            FaceUtteranceClauseKind::TemporalFact,
            FaceUtteranceProvenance::temporal_fact(0).unwrap(),
            "Arrived was observed at tick 10",
        ),
    ] {
        assert!(plan.clauses.iter().any(|clause| {
            clause.kind == kind && clause.provenance == provenance && clause.text.contains(wording)
        }));
    }
}
