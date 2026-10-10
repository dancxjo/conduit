use super::*;
use conduit_body::{
    AuthenticatedHostObservation, BodyBiographyEvidence, BodyLifecycleSession, BodyMembership,
    BodyPlayIdentity, MembershipProofId, PartId,
};
use conduit_core::{
    bind_sign, encode_count, kind_id, CheckedPlotId, CheckedValueContract, ExpandedPlotId,
    IntervalEndpoint, OfferGeneration, PlanId, Quantity, SourceDocumentId, Unit, ValueConstraint,
    COUNT_ENCODED_LEN, COUNT_INFO_ID, DISTANCE_INFO_ID, QUANTITY_ENCODED_LEN,
};
use conduit_plot::TextPatternExpression;
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
        CheckedPlotId::from("checked/browser-mask-test"),
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
            checked_plot_id: Some(CheckedPlotId::from("checked/application")),
            expanded_plot_id: Some(ExpandedPlotId::from("expanded/application")),
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
                            minimum: Some(Quantity::new(1, Unit::Meter).into()),
                            maximum: Some(Quantity::new(2, Unit::Meter).into()),
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
                value: Quantity::new(150, Unit::Centimeter).encode().to_vec(),
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
    let host = crate::installed_browser::membership_advertisement(
        HostId::from("host/browser"),
        BootId::from("boot/browser"),
    );
    let planned = plan::planned_mask(&host, plan::MASK_SOURCE, "browser-graphical").unwrap();
    let resident = conduit_body::ResidentPlot::new(
        planned.mask.plot_identity.source_document_id.clone(),
        planned.mask.plot_identity.checked_plot_id.clone(),
    );
    let born = conduit_body::Body::born(
        resident.source_document_id.clone(),
        resident.checked_plot_id.clone(),
        1,
        SignId::from("sign/body-born-plan-basis"),
    )
    .unwrap();
    let body = born.body_id.clone();
    let (_, wake) = born.wake(1, SignId::from("sign/wake")).unwrap();
    let body_plan = conduit_body::BodyPlan::seal(
        &wake,
        vec![conduit_body::BodyPlotPlan {
            plot: resident,
            plan: planned.plan,
        }],
    )
    .unwrap();
    (body, wake, body_plan)
}

fn production_tutorial_basis() -> (
    BodyLifecycleSession,
    conduit_body::Wake,
    conduit_body::BodyPlan,
) {
    let host_id = HostId::from("host/browser");
    let boot_id = BootId::from("boot/browser");
    let host = crate::installed_browser::membership_advertisement(host_id.clone(), boot_id.clone());
    let planned = plan::planned_mask(&host, plan::MASK_SOURCE, "browser-graphical").unwrap();
    let resident = conduit_body::ResidentPlot::new(
        planned.mask.plot_identity.source_document_id.clone(),
        planned.mask.plot_identity.checked_plot_id.clone(),
    );
    let body = conduit_body::Body::born(
        resident.source_document_id.clone(),
        resident.checked_plot_id.clone(),
        1,
        SignId::from("sign/production-tutorial-born"),
    )
    .unwrap();
    let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let mut evidence =
        BodyBiographyEvidence::born(body.clone(), membership.clone(), "Roseau".into()).unwrap();
    let part = PartId::bind(&body.body_id, "browser", 1).unwrap();
    let proof = MembershipProofId::bind("proof/browser").unwrap();
    let admitted = membership
        .admit(
            &body.body_id,
            membership.revision,
            part.clone(),
            proof.clone(),
            SignId::from("sign/production-tutorial-admitted"),
        )
        .unwrap();
    let present = membership
        .observe_present(
            &body.body_id,
            membership.revision,
            &part,
            AuthenticatedHostObservation {
                host_id: host_id.clone(),
                boot_id: boot_id.clone(),
                offer_generation: OfferGeneration(1),
                proof_id: proof,
                sequence: 1,
            },
            SignId::from("sign/production-tutorial-present"),
        )
        .unwrap();
    evidence
        .append_membership_events(membership, &[(admitted, 2), (present, 3)])
        .unwrap();
    let mut session = BodyLifecycleSession::open(evidence).unwrap();
    let proposal = session
        .propose(
            vec![conduit_body::BodyPlotPlan {
                plot: resident,
                plan: planned.plan,
            }],
            &host_id,
            &boot_id,
        )
        .unwrap()
        .clone();
    let play = BodyPlayIdentity::bind(&proposal.plan, 1);
    let sign =
        |sequence| bind_sign(&host_id, &boot_id, Some(&play.active_play_id), sequence).sign_id;
    let wake = proposal
        .wake
        .body_plan_ready(&proposal.plan, sign(0))
        .unwrap()
        .body_play_started(&proposal.plan, &play, sign(1))
        .unwrap();
    session
        .started(&host_id, &boot_id, play, wake.clone())
        .unwrap();
    (session, wake, proposal.plan)
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
        semantic => PresentationCompositionKind::semantic(semantic.to_owned())
            .expect("specimen semantic composition identity is bounded"),
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
            checked_plot_id: Some(CheckedPlotId::from(format!(
                "checked/face-specimen/{}",
                specimen.identity
            ))),
            expanded_plot_id: Some(ExpandedPlotId::from(format!(
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
        vec![runtime.planned.mask.plot_identity.clone()]
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
    invalid_interaction.arguments[2].value = Quantity::new(3, Unit::Meter).encode().to_vec();
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
    let (mut alternate, alternate_effect) = runtime
        .alternate(initial.wardrobe_action.body_id.clone())
        .unwrap();
    alternate
        .acknowledge(&acknowledgement(&alternate_effect))
        .unwrap();
    let alternate_observation = alternate.observation();
    let (mut replacement, replacement_effect) = alternate
        .replacement(initial.wardrobe_action.body_id.clone())
        .unwrap();
    replacement
        .acknowledge(&acknowledgement(&replacement_effect))
        .unwrap();
    let replacement_observation = replacement.observation();
    let (mut restored, restored_effect) = replacement
        .restored(initial.wardrobe_action.body_id.clone())
        .unwrap();
    restored
        .acknowledge(&acknowledgement(&restored_effect))
        .unwrap();
    let restored_observation = restored.observation();
    let journey = restored
        .actualize_journey(&[
            initial,
            alternate_observation,
            replacement_observation,
            restored_observation,
        ])
        .unwrap();
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
    assert_ne!(journey[2].show_id, journey[0].show_id);
    for unavailable in &journey[3..=5] {
        assert_eq!(&unavailable.plan_id, initial_plan);
        assert!(unavailable.show_id.is_none());
    }
    assert_ne!(journey[6].plan_id, *initial_plan);
    assert_eq!(journey[7].plan_id, journey[6].plan_id);
    assert!(journey[7].show_id.is_some());
    assert!(journey[9].show_id.is_some());
    assert_eq!(
        journey
            .iter()
            .filter_map(|outcome| outcome.show_id.as_deref())
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        4
    );
}

#[test]
fn production_tutorial_face_admits_one_show_bound_browser_interaction() {
    let (body, wake, body_plan) = production_tutorial_basis();
    let face = conduit_tutorial_plot::face_presentation(
        &body,
        13,
        conduit_tutorial_plot::TutorialPlayback::Playing,
    )
    .unwrap();
    let action = face
        .actions
        .iter()
        .find(|action| action.intent == "conduit.intent/tutorial-next@1")
        .expect("production tutorial Face exposes its current action")
        .clone();
    let argument = action
        .arguments
        .first()
        .expect("production tutorial action owns its input contract")
        .clone();
    let (mut runtime, effect) = BrowserMaskRuntime::prepare(
        body.evidence().body.body_id.clone(),
        HostId::from("host/browser"),
        BootId::from("boot/browser"),
        face.clone(),
        wake,
        body_plan,
    )
    .expect("ordinary browser Mask plans the production tutorial Face");
    runtime
        .acknowledge(&acknowledgement(&effect))
        .expect("exact browser acknowledgement makes the production Show available");

    let proposed = |value: Vec<u8>, sequence| BrowserMaskInteraction {
        show_id: effect.show_id.clone(),
        presentation_id: effect.presentation_id.clone(),
        presentation_revision: effect.presentation_revision,
        action_id: action.identity.clone(),
        target: action.target.clone(),
        arguments: vec![FaceInteractionArgument {
            name: argument.name.clone(),
            value_kind: argument.contract.value_kind.as_str().into(),
            value,
        }],
        sequence,
    };

    assert_eq!(
        runtime.interact(&proposed(vec![b'x'; 257], 1)).unwrap_err(),
        "browser Mask interaction refused: OversizeValue"
    );
    assert_eq!(
        runtime.interact(&proposed(vec![0xff], 2)).unwrap_err(),
        "browser Mask interaction refused: MalformedEncoding"
    );
    assert!(runtime.observation().interaction.is_none());

    let receipt = runtime
        .interact(&proposed(action.name.as_bytes().to_vec(), 3))
        .expect("valid production participation crosses the ordinary Mask interaction Fore");
    assert_eq!(receipt.semantic_action, action);
    assert_eq!(receipt.correlation.presentation_id, face.identity);
    assert_eq!(receipt.correlation.presentation_revision, face.revision);
    assert_eq!(receipt.correlation.interaction.show_id, effect.show_id);
    assert_eq!(
        receipt.correlation.interaction.arguments[0].name,
        "input/tutorial-action"
    );
    assert_eq!(
        runtime
            .observation()
            .interaction
            .expect("accepted interaction remains in bounded Mask evidence")
            .correlation,
        receipt.correlation
    );
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
fn every_face_specimen_executes_through_the_ordinary_browser_mask_plot() {
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
        assert_eq!(observation.planned_mask.mask.plot_name, "browser-graphical");
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
