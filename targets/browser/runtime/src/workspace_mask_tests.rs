use super::*;
use conduit_core::{
    encode_count, CheckedFormId, CheckedValueContract, ExpandedFormId, IntervalEndpoint, PlanId,
    Quantity, QuantityUnit, SourceDocumentId, ValueConstraint, COUNT_ENCODED_LEN, COUNT_INFO_ID,
    DISTANCE_INFO_ID, QUANTITY_ENCODED_LEN,
};
use conduit_form::TextPatternExpression;
use conduit_presentation::{
    PresentationAction, PresentationActionAvailability, PresentationBasis,
    PresentationCompositionKind, PresentationCompositionRelation, PresentationContextBasis,
    PresentationDisclosureLevel, PresentationInteractionContext, PresentationRelationship,
    PresentationRelationshipKind, PresentationRole, PresentationSubject, PresentationText,
    UTF8_TEXT_VALUE_KIND,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct FaceSpecimen {
    identity: String,
    presentation: FaceSpecimenValue,
}

#[derive(Debug, Deserialize)]
struct FaceSpecimenValue {
    subjects: Vec<FaceSpecimenSubject>,
    relationships: Vec<FaceSpecimenRelationship>,
    composition: Vec<FaceSpecimenComposition>,
    actions: Vec<FaceSpecimenAction>,
}

#[derive(Debug, Deserialize)]
struct FaceSpecimenSubject {
    identity: String,
    name: String,
    role: String,
}

#[derive(Debug, Deserialize)]
struct FaceSpecimenRelationship {
    source: String,
    kind: String,
    target: String,
}

#[derive(Debug, Deserialize)]
struct FaceSpecimenComposition {
    source: String,
    kind: String,
    target: String,
}

#[derive(Debug, Deserialize)]
struct FaceSpecimenAction {
    identity: String,
    intent: String,
    target: String,
    name: String,
    availability: String,
}

fn body_id() -> BodyId {
    conduit_body::Body::born(
        SourceDocumentId::from("source/browser-mask-test"),
        CheckedFormId::from("checked/browser-mask-test"),
        1,
        SignId::from("sign/body-born"),
    )
    .unwrap()
    .body_id
}

fn presentation() -> Presentation {
    let lowercase = TextPatternExpression::Repeat {
        expression: Box::new(TextPatternExpression::ScalarRange {
            first: 'a' as u32,
            last: 'z' as u32,
        }),
        minimum: 1,
        maximum: 32,
    }
    .compile(32)
    .expect("reviewed browser Face pattern compiles during checking");
    Presentation::new_with_semantics(
        7,
        PresentationBasis {
            body_id: Some(body_id()),
            wake_id: None,
            source_document_id: Some(SourceDocumentId::from("source/application")),
            checked_form_id: Some(CheckedFormId::from("checked/application")),
            expanded_form_id: Some(ExpandedFormId::from("expanded/application")),
            plan_id: Some(PlanId::from("plan/application")),
            active_play_id: None,
            sign_ids: vec![SignId::from("sign/presentation")],
        },
        vec![PresentationSubject {
            identity: "body/browser-mask-test".into(),
            role: PresentationRole::Body,
            name: "Test Body".into(),
        }],
        vec![],
        vec![],
        vec![PresentationText {
            subject: "body/browser-mask-test".into(),
            text: "One canonical journey".into(),
        }],
        vec![PresentationAction {
            identity: "body.inspect".into(),
            intent: "conduit.intent/inspect@1".into(),
            target: "body/browser-mask-test".into(),
            name: "Inspect".into(),
            arguments: vec![
                conduit_presentation::FaceActionArgument {
                    name: "input/inspect".into(),
                    value_name: "Lowercase subject".into(),
                    contract: CheckedValueContract::new(
                        UTF8_TEXT_VALUE_KIND.into(),
                        32,
                        vec![
                            ValueConstraint::ByteLength {
                                minimum: 1,
                                maximum: 32,
                            },
                            ValueConstraint::TextPattern {
                                pattern: lowercase,
                                anchored_start: true,
                                anchored_end: true,
                                negated: false,
                            },
                        ],
                    )
                    .expect("reviewed browser Face pattern contract is canonical"),
                },
                conduit_presentation::FaceActionArgument {
                    name: "input/count".into(),
                    value_name: "Inspection count from two through four".into(),
                    contract: CheckedValueContract::new(
                        COUNT_INFO_ID.into(),
                        COUNT_ENCODED_LEN as u32,
                        vec![ValueConstraint::UnsignedRange {
                            minimum: Some(2),
                            maximum: Some(4),
                            minimum_endpoint: IntervalEndpoint::Inclusive,
                            maximum_endpoint: IntervalEndpoint::Inclusive,
                        }],
                    )
                    .expect("reviewed browser Face count range is canonical"),
                },
                conduit_presentation::FaceActionArgument {
                    name: "input/distance".into(),
                    value_name: "Inspection distance from one through two meters".into(),
                    contract: CheckedValueContract::new(
                        DISTANCE_INFO_ID.into(),
                        QUANTITY_ENCODED_LEN as u32,
                        vec![ValueConstraint::QuantityRange {
                            minimum: Some(Quantity::new(1, QuantityUnit::Meter)),
                            maximum: Some(Quantity::new(2, QuantityUnit::Meter)),
                            minimum_endpoint: IntervalEndpoint::Inclusive,
                            maximum_endpoint: IntervalEndpoint::Inclusive,
                        }],
                    )
                    .expect("reviewed browser Face quantity range is canonical"),
                },
                conduit_presentation::FaceActionArgument {
                    name: "input/mode".into(),
                    value_name: "Inspection mode".into(),
                    contract: CheckedValueContract::new(
                        UTF8_TEXT_VALUE_KIND.into(),
                        8,
                        vec![ValueConstraint::CanonicalMembership {
                            members: vec![b"careful".to_vec(), b"quick".to_vec()],
                            negated: false,
                        }],
                    )
                    .expect("reviewed browser Face finite membership is canonical"),
                },
            ],
            disclosure: PresentationDisclosureLevel::CurrentAction,
            availability: PresentationActionAvailability::Available,
        }],
        vec![],
    )
    .unwrap()
}

fn interaction(effect: &BrowserMaskEffect) -> BrowserMaskInteraction {
    BrowserMaskInteraction {
        show_id: effect.show_id.clone(),
        presentation_id: effect.presentation_id.clone(),
        presentation_revision: effect.presentation_revision,
        action_id: "body.inspect".into(),
        target: "body/browser-mask-test".into(),
        arguments: vec![
            FaceInteractionArgument {
                name: "input/inspect".into(),
                value_kind: UTF8_TEXT_VALUE_KIND.into(),
                value: b"body".to_vec(),
            },
            FaceInteractionArgument {
                name: "input/count".into(),
                value_kind: COUNT_INFO_ID.into(),
                value: encode_count(3).to_vec(),
            },
            FaceInteractionArgument {
                name: "input/distance".into(),
                value_kind: DISTANCE_INFO_ID.into(),
                value: Quantity::new(150, QuantityUnit::Centimeter)
                    .encode()
                    .to_vec(),
            },
            FaceInteractionArgument {
                name: "input/mode".into(),
                value_kind: UTF8_TEXT_VALUE_KIND.into(),
                value: b"careful".to_vec(),
            },
        ],
        sequence: 1,
    }
}

fn acknowledgement(effect: &BrowserMaskEffect) -> BrowserMaskAcknowledgement {
    BrowserMaskAcknowledgement {
        show_id: effect.show_id.clone(),
        manifestation_id: effect.manifestation_id.clone(),
        mask_plan_id: effect.mask_plan_id.clone(),
        active_play_id: effect.mask_play.active_play_id.clone(),
        placement_id: effect.placement_id.clone(),
        presentation_id: effect.presentation_id.clone(),
        presentation_revision: effect.presentation_revision,
    }
}

fn contextual_presentation(context: &str) -> Presentation {
    let base = presentation();
    let mut subjects = base.subjects;
    subjects.push(PresentationSubject {
        identity: "participant/current".into(),
        role: PresentationRole::Semantic(kind_id("human/participant")),
        name: "Current participant".into(),
    });
    let mut relationships = base.relationships;
    relationships.push(PresentationRelationship {
        source: "body/browser-mask-test".into(),
        target: "participant/current".into(),
        kind: PresentationRelationshipKind::Contains,
    });
    Presentation::new_with_semantics(
        base.revision,
        base.basis,
        subjects,
        relationships,
        base.properties,
        base.text,
        base.actions,
        base.disclosures,
    )
    .unwrap()
    .with_interaction_context(PresentationInteractionContext {
        identity: format!("context/{context}"),
        basis: vec![PresentationContextBasis {
            source: "body/browser-mask-test".into(),
            relationship: PresentationRelationshipKind::Contains,
            target: "participant/current".into(),
        }],
    })
    .unwrap()
}

fn body_plan_basis() -> (BodyId, conduit_body::Wake, conduit_body::BodyPlan) {
    let planned = plan::planned_mask(
        HostId::from("host/browser"),
        BootId::from("boot/browser"),
        plan::MASK_SOURCE,
        "browser-graphical",
    )
    .unwrap();
    let resident = conduit_body::ResidentForm::new(
        planned.mask.form_identity.source_document_id.clone(),
        planned.mask.form_identity.checked_form_id.clone(),
    );
    let born = conduit_body::Body::born(
        resident.source_document_id.clone(),
        resident.checked_form_id.clone(),
        1,
        SignId::from("sign/body-born-plan-basis"),
    )
    .unwrap();
    let body = born.body_id.clone();
    let (_, wake) = born.wake(1, SignId::from("sign/wake")).unwrap();
    let body_plan = conduit_body::BodyPlan::seal(
        &wake,
        vec![conduit_body::BodyFormPlan {
            form: resident,
            plan: planned.plan,
        }],
    )
    .unwrap();
    (body, wake, body_plan)
}

fn face_specimens() -> Vec<FaceSpecimen> {
    serde_json::from_str(include_str!(
        "../../../../proof/conformance/presentation-waist/specimens.json"
    ))
    .expect("shared Face specimens must remain valid JSON")
}

fn relationship_kind(kind: &str) -> PresentationRelationshipKind {
    match kind {
        "Contains" => PresentationRelationshipKind::Contains,
        "Connects" => PresentationRelationshipKind::Connects,
        "Describes" => PresentationRelationshipKind::Describes,
        "Realizes" => PresentationRelationshipKind::Realizes,
        "Observes" => PresentationRelationshipKind::Observes,
        semantic => PresentationRelationshipKind::Semantic(kind_id(semantic)),
    }
}

fn composition_kind(kind: &str) -> PresentationCompositionKind {
    match kind {
        "Group" => PresentationCompositionKind::Group,
        "Contrast" => PresentationCompositionKind::Contrast,
        "Juxtapose" => PresentationCompositionKind::Juxtapose,
        "Emphasize" => PresentationCompositionKind::Emphasize,
        "Subordinate" => PresentationCompositionKind::Subordinate,
        "Associate" => PresentationCompositionKind::Associate,
        "RevealAfter" => PresentationCompositionKind::RevealAfter,
        semantic => PresentationCompositionKind::Semantic(kind_id(semantic)),
    }
}

fn specimen_face(specimen: &FaceSpecimen, body: BodyId, revision: u64) -> Presentation {
    let value = &specimen.presentation;
    Presentation::new_with_semantics(
        revision,
        PresentationBasis {
            body_id: Some(body),
            wake_id: None,
            source_document_id: Some(SourceDocumentId::from(format!(
                "source/face-specimen/{}",
                specimen.identity
            ))),
            checked_form_id: Some(CheckedFormId::from(format!(
                "checked/face-specimen/{}",
                specimen.identity
            ))),
            expanded_form_id: Some(ExpandedFormId::from(format!(
                "expanded/face-specimen/{}",
                specimen.identity
            ))),
            plan_id: Some(PlanId::from(format!(
                "plan/face-specimen/{}",
                specimen.identity
            ))),
            active_play_id: None,
            sign_ids: vec![SignId::from(format!(
                "sign/face-specimen/{}",
                specimen.identity
            ))],
        },
        value
            .subjects
            .iter()
            .map(|subject| PresentationSubject {
                identity: subject.identity.clone(),
                role: PresentationRole::Semantic(kind_id(&subject.role)),
                name: subject.name.clone(),
            })
            .collect(),
        value
            .relationships
            .iter()
            .map(|relationship| PresentationRelationship {
                source: relationship.source.clone(),
                target: relationship.target.clone(),
                kind: relationship_kind(&relationship.kind),
            })
            .collect(),
        vec![],
        vec![],
        value
            .actions
            .iter()
            .map(|action| {
                assert_eq!(action.availability, "available");
                PresentationAction {
                    identity: action.identity.clone(),
                    intent: action.intent.clone(),
                    target: action.target.clone(),
                    name: action.name.clone(),
                    arguments: vec![],
                    disclosure: PresentationDisclosureLevel::CurrentAction,
                    availability: PresentationActionAvailability::Available,
                }
            })
            .collect(),
        vec![],
    )
    .expect("Face specimen must construct the migration-era Presentation value")
    .with_composition(
        value
            .composition
            .iter()
            .enumerate()
            .map(|(index, relation)| PresentationCompositionRelation {
                identity: format!("composition/{}/{index}", specimen.identity),
                source: relation.source.clone(),
                target: relation.target.clone(),
                kind: composition_kind(&relation.kind),
            })
            .collect(),
    )
    .expect("Face specimen composition must remain valid")
}

#[test]
fn show_becomes_available_only_after_exact_browser_acknowledgement() {
    let (body, wake, body_plan) = body_plan_basis();
    let (mut runtime, effect) = BrowserMaskRuntime::prepare(
        body,
        HostId::from("host/browser"),
        BootId::from("boot/browser"),
        presentation(),
        wake,
        body_plan,
    )
    .unwrap();
    assert_eq!(
        runtime.show.show.lifecycle,
        ManifestationLifecycle::Prepared
    );
    assert_ne!(
        runtime.presentation.basis.plan_id.as_ref(),
        Some(&runtime.planned.plan.plan_id)
    );
    let mut stale = acknowledgement(&effect);
    stale.presentation_revision += 1;
    assert!(runtime.acknowledge(&stale).is_err());
    assert_eq!(
        runtime.show.show.lifecycle,
        ManifestationLifecycle::Prepared
    );
    runtime.acknowledge(&acknowledgement(&effect)).unwrap();
    assert_eq!(
        runtime.show.show.lifecycle,
        ManifestationLifecycle::Available
    );
    assert_eq!(
        runtime.wardrobe_action.resulting_wardrobe.worn,
        vec![runtime.planned.mask.form_identity.clone()]
    );
    let mut stale_interaction = interaction(&effect);
    stale_interaction.presentation_revision += 1;
    assert!(runtime.interact(&stale_interaction).is_err());
    let mut invalid_interaction = interaction(&effect);
    invalid_interaction.arguments[0].value = b"Body".to_vec();
    assert_eq!(
        runtime.interact(&invalid_interaction).unwrap_err(),
        "browser Mask interaction refused: ViolatedConstraint"
    );
    let mut invalid_interaction = interaction(&effect);
    invalid_interaction.arguments[1].value = encode_count(5).to_vec();
    assert_eq!(
        runtime.interact(&invalid_interaction).unwrap_err(),
        "browser Mask interaction refused: ViolatedConstraint"
    );
    let mut invalid_interaction = interaction(&effect);
    invalid_interaction.arguments[2].value =
        Quantity::new(3, QuantityUnit::Meter).encode().to_vec();
    assert_eq!(
        runtime.interact(&invalid_interaction).unwrap_err(),
        "browser Mask interaction refused: ViolatedConstraint"
    );
    let mut invalid_interaction = interaction(&effect);
    invalid_interaction.arguments[3].value = b"reckless".to_vec();
    assert_eq!(
        runtime.interact(&invalid_interaction).unwrap_err(),
        "browser Mask interaction refused: ViolatedConstraint"
    );
    let receipt = runtime.interact(&interaction(&effect)).unwrap();
    assert_eq!(receipt.semantic_action.identity, "body.inspect");
    assert_eq!(receipt.correlation.interaction.show_id, effect.show_id);
    let observation = runtime.observation();
    assert_eq!(
        observation
            .interaction
            .as_ref()
            .unwrap()
            .correlation
            .interaction
            .identity,
        receipt.correlation.interaction.identity
    );
    assert_eq!(observation.execution.fore.len(), 3);
    let kinds = observation
        .execution
        .remote_signs
        .iter()
        .map(|sign| sign.kind.as_str())
        .collect::<Vec<_>>();
    for required in [
        "RemoteInputAdmitted",
        "RemoteInputClosed",
        "RemoteValueOffered",
        "RemoteValueAccepted",
        "RemoteValueDelivered",
        "RemoteOutputClosed",
    ] {
        assert!(kinds.contains(&required), "missing {required}: {kinds:?}");
    }

    let initial = runtime.observation();
    let (mut replacement, replacement_effect) = runtime
        .replacement(initial.wardrobe_action.body_id.clone())
        .unwrap();
    replacement
        .acknowledge(&acknowledgement(&replacement_effect))
        .unwrap();
    let journey = replacement.actualize_journey(&initial).unwrap();
    assert_eq!(journey.len(), 10);
    assert_eq!(
        journey
            .iter()
            .map(|outcome| outcome.action_id)
            .collect::<Vec<_>>(),
        conduit_presentation::MASK_JOURNEY_ACTIONS.map(conduit_presentation::MaskJourneyAction::id)
    );
    let initial_plan = &journey[0].plan_id;
    assert_eq!(&journey[2].plan_id, initial_plan);
    assert_eq!(journey[2].show_id, journey[0].show_id);
    for unavailable in &journey[3..=5] {
        assert_eq!(&unavailable.plan_id, initial_plan);
        assert!(unavailable.show_id.is_none());
    }
    assert_ne!(journey[6].plan_id, *initial_plan);
    assert_eq!(journey[7].plan_id, journey[6].plan_id);
    assert!(journey[7].show_id.is_some());
    assert!(journey[9].show_id.is_some());
}

#[test]
fn ordinary_mask_fore_refuses_same_revision_interaction_from_another_face_context() {
    let teacher = contextual_presentation("teacher");
    let student = contextual_presentation("student");
    assert_eq!(teacher.revision, student.revision);
    assert_ne!(teacher.identity, student.identity);

    let (body, wake, body_plan) = body_plan_basis();
    let (mut runtime, effect) = BrowserMaskRuntime::prepare(
        body,
        HostId::from("host/browser"),
        BootId::from("boot/browser"),
        teacher,
        wake,
        body_plan,
    )
    .unwrap();
    runtime.acknowledge(&acknowledgement(&effect)).unwrap();

    let mut stale = interaction(&effect);
    stale.presentation_id = student.identity.as_str().into();
    assert_eq!(
        runtime.interact(&stale).unwrap_err(),
        "browser Mask interaction is stale or mismatched"
    );
    assert!(runtime.interaction_receipt.is_none());
    assert!(runtime.pending_interaction_node.is_some());

    let receipt = runtime.interact(&interaction(&effect)).unwrap();
    assert_eq!(
        receipt.correlation.interaction.face_id,
        effect.presentation_id
    );
}

#[test]
fn every_face_specimen_executes_through_the_ordinary_browser_mask_form() {
    let specimens = face_specimens();
    assert_eq!(specimens.len(), 10);

    for (index, specimen) in specimens.iter().enumerate() {
        let (body, wake, body_plan) = body_plan_basis();
        let face = specimen_face(specimen, body.clone(), index as u64 + 1);
        let expected_identity = face.identity.clone();
        let expected_revision = face.revision;
        let (mut runtime, effect) = BrowserMaskRuntime::prepare(
            body,
            HostId::from(format!("host/browser/{}", specimen.identity)),
            BootId::from(format!("boot/browser/{}", specimen.identity)),
            face,
            wake,
            body_plan,
        )
        .unwrap_or_else(|error| {
            panic!(
                "{} did not reach the browser Mask: {error}",
                specimen.identity
            )
        });

        assert_eq!(effect.schema, "conduit.browser/mask-effect@1");
        assert_eq!(effect.presentation_id, expected_identity.as_str());
        assert_eq!(effect.presentation_revision, expected_revision);
        assert_eq!(
            runtime.show.show.lifecycle,
            ManifestationLifecycle::Prepared
        );
        runtime
            .acknowledge(&acknowledgement(&effect))
            .unwrap_or_else(|error| {
                panic!("{} did not emit its exact Show: {error}", specimen.identity)
            });

        let observation = runtime.observation();
        assert_eq!(observation.presentation.identity, expected_identity);
        assert_eq!(
            observation.mask_show.show.lifecycle,
            ManifestationLifecycle::Available
        );
        assert_eq!(observation.planned_mask.mask.form_name, "browser-graphical");
        assert_eq!(observation.execution.fore.len(), 3);
        assert!(observation
            .execution
            .remote_signs
            .iter()
            .any(|sign| { sign.kind == "RemoteInputAdmitted" }));
        assert!(observation
            .execution
            .remote_signs
            .iter()
            .any(|sign| { sign.kind == "RemoteValueDelivered" }));

        if specimen.identity == "source-destination" {
            assert!(observation
                .presentation
                .relationships
                .iter()
                .any(|relationship| {
                    relationship.source == "report"
                        && relationship.target == "destination"
                        && relationship.kind
                            == PresentationRelationshipKind::Semantic(kind_id(
                                "file/copy-destination",
                            ))
                }));
            assert!(observation.presentation.composition.iter().any(|relation| {
                relation.source == "source"
                    && relation.target == "destination"
                    && relation.kind == PresentationCompositionKind::Juxtapose
            }));
            assert!(observation.presentation.actions.iter().any(|action| {
                action.identity == "copy-to-destination"
                    && action.intent == "encounter/copy-to-destination"
                    && action.target == "report"
                    && action.availability == PresentationActionAvailability::Available
            }));
        }
    }
}
