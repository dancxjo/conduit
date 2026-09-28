use super::*;
use crate::{
    GenerativeNarratorRole, GenerativePresenterBounds, GenerativePresenterPolicy, Presentation,
    PresentationAction, PresentationActionAvailability, PresentationBasis,
    PresentationCompositionKind, PresentationCompositionRelation, PresentationContextBasis,
    PresentationDisclosure, PresentationDisclosureLevel, PresentationInteractionContext,
    PresentationProperty, PresentationPropertyValue, PresentationRelationship,
    PresentationRelationshipKind, PresentationRole, PresentationSubject, PresentationTemporalFact,
    PresentationTemporalRole, PresentationText, TemporalInstant, TemporalReference, TemporalScale,
};
use alloc::{string::String, vec};
use conduit_core::{
    kind_id, ArtifactId, Back, BackOfferBuilder, BootId, BoundedResourceRef, CapabilityId,
    ExecutionProfileId, HostAdvertisement, HostId, HostProfileId, ImplementationId,
    OfferGeneration, PlacementId, ResourceClassId, ResourceExtent, ResourceLifetime,
    ResourceSemanticIdentity, ResourceVersionIdentity, PROTOCOL_VERSION,
};
use conduit_form::ProfileCatalog;
use conduit_planner::{default_placements, plan};

fn request() -> GenerativePresenterRequest {
    let reference = BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest([7; 32]),
        content_profile: kind_id("biology/diagram@1"),
        access_class: ResourceClassId::from("content/public@1"),
        extent: ResourceExtent {
            bytes: 12,
            items: Some(1),
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([8; 32]),
            expires_at: None,
        },
    }
    .encode()
    .unwrap();
    let presentation = Presentation::new_with_semantics(
        4,
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
            PresentationSubject {
                identity: "concept/cell".into(),
                role: PresentationRole::Semantic(kind_id("biology/cell@1")),
                name: "Cell".into(),
            },
            PresentationSubject {
                identity: "concept/mitochondrion".into(),
                role: PresentationRole::Semantic(kind_id("biology/organelle@1")),
                name: "Mitochondrion".into(),
            },
        ],
        vec![PresentationRelationship {
            source: "concept/cell".into(),
            target: "concept/mitochondrion".into(),
            kind: PresentationRelationshipKind::Contains,
        }],
        vec![PresentationProperty {
            subject: "concept/mitochondrion".into(),
            name: "diagram".into(),
            value: PresentationPropertyValue::Content(reference),
        }],
        vec![PresentationText {
            subject: "concept/mitochondrion".into(),
            text: "Produces cellular energy".into(),
        }],
        vec![PresentationAction {
            identity: "lesson/answer".into(),
            intent: "education/answer@1".into(),
            target: "concept/mitochondrion".into(),
            name: "Answer".into(),
            arguments: vec![crate::FaceActionArgument::text(
                "lesson/answer-input".into(),
                "Answer".into(),
                1,
                128,
            )
            .unwrap()],
            disclosure: PresentationDisclosureLevel::CurrentAction,
            availability: PresentationActionAvailability::Available,
        }],
        vec![PresentationDisclosure {
            subject: "concept/mitochondrion".into(),
            level: PresentationDisclosureLevel::SelectedDetail,
        }],
    )
    .unwrap()
    .with_composition(vec![PresentationCompositionRelation {
        identity: "composition/emphasis".into(),
        source: "concept/mitochondrion".into(),
        target: "concept/cell".into(),
        kind: PresentationCompositionKind::Emphasize,
    }])
    .unwrap()
    .with_interaction_context(PresentationInteractionContext {
        identity: "context/lesson".into(),
        basis: vec![PresentationContextBasis {
            source: "concept/cell".into(),
            relationship: PresentationRelationshipKind::Contains,
            target: "concept/mitochondrion".into(),
        }],
    })
    .unwrap();
    GenerativePresenterRequest::from_presentation(
        "request/4".into(),
        GenerativePresenterPolicy {
            template_contract_revision: "template/1".into(),
            narrator_role: GenerativeNarratorRole::TransientFirstPersonBodyNarrator,
            instructions: "Describe only correlated semantic truth".into(),
        },
        presentation,
        None,
        GenerativePresenterBounds::reviewed_default(),
    )
    .unwrap()
}

fn candidate(request: &GenerativePresenterRequest) -> GeneratedManifestationCandidate {
    let mut candidate = GeneratedManifestationCandidate {
        candidate_identity: String::new(),
        request_identity: request.request_identity.clone(),
        source_presentation_identity: request.semantic_data.source_presentation_identity.clone(),
        source_presentation_revision: request.semantic_data.source_presentation_revision,
        presenter_implementation_identity: "presenter/fixture@1".into(),
        provider_identity: "provider/fixture@1".into(),
        model_identity: "model/fixture@1".into(),
        template_contract_revision: request.policy.template_contract_revision.clone(),
        mask_contract_revision: "mask/spoken-contract@1".into(),
        generation_run_identity: "run/4".into(),
        disposition: GeneratedManifestationDisposition::Produced,
        content: vec![GeneratedContentSegment {
            role: GeneratedContentRole::Speech,
            source_text_index: 0,
            bytes: b"The diagram emphasizes the mitochondrion. You may answer.".to_vec(),
        }],
        affordances: vec![GeneratedActionAffordance {
            action_identity: "lesson/answer".into(),
            source_presentation_revision: 4,
        }],
        correlations: vec![
            GeneratedSemanticCorrelation::Relationship {
                index: 0,
                source: "concept/cell".into(),
                target: "concept/mitochondrion".into(),
                kind: PresentationRelationshipKind::Contains,
            },
            GeneratedSemanticCorrelation::Text {
                index: 0,
                subject: "concept/mitochondrion".into(),
            },
            GeneratedSemanticCorrelation::Property {
                index: 0,
                subject: "concept/mitochondrion".into(),
                name: "diagram".into(),
            },
            GeneratedSemanticCorrelation::TypedContent {
                index: 0,
                subject: "concept/mitochondrion".into(),
                name: "diagram".into(),
                content_profile: "biology/diagram@1".into(),
            },
            GeneratedSemanticCorrelation::Composition {
                index: 0,
                identity: "composition/emphasis".into(),
            },
            GeneratedSemanticCorrelation::Action {
                index: 0,
                identity: "lesson/answer".into(),
                intent: "education/answer@1".into(),
                target: "concept/mitochondrion".into(),
            },
            GeneratedSemanticCorrelation::ActionArgument {
                action_index: 0,
                argument_index: 0,
                name: "lesson/answer-input".into(),
                value_kind: "value/text".into(),
            },
            GeneratedSemanticCorrelation::Disclosure {
                index: 0,
                subject: "concept/mitochondrion".into(),
                level: PresentationDisclosureLevel::SelectedDetail,
            },
            GeneratedSemanticCorrelation::Context {
                index: 0,
                source: "concept/cell".into(),
                target: "concept/mitochondrion".into(),
                relationship: PresentationRelationshipKind::Contains,
            },
        ],
    };
    candidate.candidate_identity = candidate.digest();
    candidate
}

fn assessment(candidate: &GeneratedManifestationCandidate) -> GeneratedValidatorAssessment {
    GeneratedValidatorAssessment {
        assessment_identity: "assessment/4".into(),
        candidate_digest: candidate.digest(),
        source_presentation_identity: candidate.source_presentation_identity.clone(),
        source_presentation_revision: candidate.source_presentation_revision,
        mask_identity: "mask/spoken@1".into(),
        mask_contract_revision: "mask/spoken-contract@1".into(),
        disposition: GeneratedValidationDisposition::Accepted,
        refusal_code: None,
        accepted_correlations: candidate.correlations.clone(),
    }
}

fn session() -> GeneratedValidationSession {
    GeneratedValidationSession::selected(
        "validator/semantic@1".into(),
        "back/semantic-validator@1".into(),
        "mask/spoken@1".into(),
        "mask/spoken-contract@1".into(),
    )
    .unwrap()
}

fn validator_plan() -> (conduit_core::Plan, PlacementId) {
    plan_for_kind(GENERATED_VALIDATOR_KIND)
}

fn plan_for_kind(kind_identity: &str) -> (conduit_core::Plan, PlacementId) {
    let validator = spoken_mask_kinds()
        .into_iter()
        .find(|kind| kind.kind_id.as_str() == kind_identity)
        .unwrap();
    let mut profiles = ProfileCatalog::new();
    profiles.insert_kind(validator.clone()).unwrap();
    let source = alloc::format!("form validation {{\n    validator: {kind_identity}\n}}\n");
    let checked = conduit_form::parse(&source, &profiles).unwrap();
    let host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/validator"),
        boot_id: BootId::from("boot/validator"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("validator/test@1"),
        bases: vec![],
        resources: vec![],
        capabilities: vec![BackOfferBuilder::new(
            validator,
            Back {
                capability_id: CapabilityId::from("back/generated-validator"),
                execution_profile_id: ExecutionProfileId::from("validator/test@1"),
                implementation_id: ImplementationId::from("implementation/generated-validator@1"),
                artifact_id: ArtifactId::from("artifact/generated-validator@1"),
                host_calls: vec![],
                resource_requirements: vec![],
                authority_requirements: vec![],
            },
        )
        .build()],
        planner_capabilities: vec![],
    };
    let choices = default_placements(&checked, core::slice::from_ref(&host)).unwrap();
    let plan = plan(&checked, &[host], &choices, &[]).unwrap();
    let placement = plan.fragments[0].placements[0].placement_id.clone();
    (plan, placement)
}

#[test]
fn accepted_output_requires_exact_typed_content_composition_and_action_correlations() {
    let request = request();
    let candidate = candidate(&request);
    let outcome = session()
        .retain_unplanned(&request, candidate.clone(), assessment(&candidate))
        .unwrap();
    let GeneratedValidationOutcome::Accepted(accepted) = outcome else {
        panic!("accepted outcome")
    };
    assert_eq!(
        accepted.validation_receipt().assessment_identity,
        "assessment/4"
    );
    assert_eq!(accepted.content().len(), 1);
}

#[test]
fn deterministic_validator_accepts_only_exact_presentation_wording() {
    let request = request();
    let mut candidate = candidate(&request);
    candidate.content[0].bytes = b"Produces cellular energy".to_vec();
    candidate.candidate_identity = candidate.digest();
    let envelope = GeneratedValidationEnvelope {
        request,
        candidate: candidate.clone(),
    };
    let assessment = assess_generated_output_exactly(
        &envelope,
        "assessment/exact".into(),
        "mask/spoken@1".into(),
    );
    assert_eq!(
        assessment.disposition,
        GeneratedValidationDisposition::Accepted
    );
    assert_eq!(assessment.accepted_correlations, candidate.correlations);

    let mut thought = envelope.clone();
    thought.candidate.content[0].role = GeneratedContentRole::PresentedThought;
    thought.candidate.candidate_identity = thought.candidate.digest();
    assert_eq!(
        assess_generated_output_exactly(
            &thought,
            "assessment/thought".into(),
            "mask/spoken@1".into(),
        )
        .disposition,
        GeneratedValidationDisposition::Accepted
    );

    let mut paraphrase = envelope;
    paraphrase.candidate.content[0].bytes = b"It makes energy for the cell".to_vec();
    paraphrase.candidate.candidate_identity = paraphrase.candidate.digest();
    let assessment = assess_generated_output_exactly(
        &paraphrase,
        "assessment/paraphrase".into(),
        "mask/spoken@1".into(),
    );
    assert_eq!(
        assessment.disposition,
        GeneratedValidationDisposition::Refused
    );
    assert!(assessment.accepted_correlations.is_empty());

    let mut invented_index = paraphrase;
    invented_index.candidate.content[0].source_text_index = 9;
    invented_index.candidate.content[0].bytes = b"Produces cellular energy".to_vec();
    invented_index.candidate.candidate_identity = invented_index.candidate.digest();
    assert_eq!(
        assess_generated_output_exactly(
            &invented_index,
            "assessment/invented-index".into(),
            "mask/spoken@1".into(),
        )
        .disposition,
        GeneratedValidationDisposition::Refused
    );
}

#[test]
fn validator_session_requires_exact_kind_placement_and_current_plan_binding() {
    let (plan, placement) = validator_plan();
    let session = GeneratedValidationSession::from_plan(
        &plan,
        &placement,
        "mask/spoken@1".into(),
        "mask/spoken-contract@1".into(),
    )
    .unwrap();
    let request = request();
    let mut exact_candidate = candidate(&request);
    exact_candidate.content[0].bytes = b"Produces cellular energy".to_vec();
    exact_candidate.candidate_identity = exact_candidate.digest();
    let exact_assessment = assess_generated_output_exactly(
        &GeneratedValidationEnvelope {
            request: request.clone(),
            candidate: exact_candidate.clone(),
        },
        "assessment/planned".into(),
        "mask/spoken@1".into(),
    );
    let outcome = session
        .retain(&plan, &request, exact_candidate, exact_assessment)
        .unwrap();
    let GeneratedValidationOutcome::Accepted(generated) = outcome else {
        panic!("accepted outcome")
    };
    assert_eq!(generated.validation_receipt().plan_id(), &plan.plan_id);
    assert_eq!(generated.validation_receipt().placement_id(), &placement);
    assert_eq!(
        generated
            .validation_receipt()
            .source_presentation_identity(),
        request.semantic_data.source_presentation_identity
    );
    assert_eq!(
        generated
            .validation_receipt()
            .source_presentation_revision(),
        request.semantic_data.source_presentation_revision
    );
    assert_eq!(
        generated.validation_receipt().mask_identity(),
        "mask/spoken@1"
    );
    assert_eq!(
        generated.validation_receipt().mask_contract_revision(),
        "mask/spoken-contract@1"
    );
    assert_eq!(
        generated
            .validation_receipt()
            .validator_implementation_identity(),
        "implementation/generated-validator@1"
    );
    assert_eq!(
        generated.validation_receipt().validator_back_identity(),
        "back/generated-validator"
    );
    assert_eq!(
        generated.validation_receipt().host_id(),
        &HostId::from("host/validator")
    );
    assert_eq!(
        generated.validation_receipt().boot_id(),
        &BootId::from("boot/validator")
    );

    let mut wrong_kind = plan.clone();
    wrong_kind.fragments[0].placements[0].kind_id = kind_id("presentation/not-validator");
    assert!(matches!(
        GeneratedValidationSession::from_plan(
            &wrong_kind,
            &placement,
            "mask/spoken@1".into(),
            "mask/spoken-contract@1".into(),
        ),
        Err(GeneratedValidationError::InvalidPlan)
    ));
    assert!(matches!(
        GeneratedValidationSession::from_plan(
            &plan,
            &PlacementId::from("placement/missing"),
            "mask/spoken@1".into(),
            "mask/spoken-contract@1".into(),
        ),
        Err(GeneratedValidationError::MissingValidatorPlacement)
    ));
    let (other_plan, other_placement) = plan_for_kind(PRESENTATION_TO_GENERATIVE_REQUEST_KIND);
    assert!(matches!(
        GeneratedValidationSession::from_plan(
            &other_plan,
            &other_placement,
            "mask/spoken@1".into(),
            "mask/spoken-contract@1".into(),
        ),
        Err(GeneratedValidationError::WrongValidatorKind)
    ));

    let stale_session = GeneratedValidationSession::from_plan(
        &plan,
        &placement,
        "mask/spoken@1".into(),
        "mask/spoken-contract@1".into(),
    )
    .unwrap();
    let mut stale_plan = plan.clone();
    stale_plan.plan_id = conduit_core::PlanId::from("plan/stale");
    let candidate = candidate(&request);
    assert_eq!(
        stale_session.retain(
            &stale_plan,
            &request,
            candidate.clone(),
            assessment(&candidate),
        ),
        Err(GeneratedValidationError::StaleValidatorBinding)
    );
}

#[test]
fn rejects_invented_out_of_range_and_duplicate_correlations_and_uncorrelated_actions() {
    let request = request();
    for bad in [
        GeneratedSemanticCorrelation::TypedContent {
            index: 9,
            subject: "concept/mitochondrion".into(),
            name: "diagram".into(),
            content_profile: "biology/diagram@1".into(),
        },
        GeneratedSemanticCorrelation::Composition {
            index: 0,
            identity: "invented".into(),
        },
    ] {
        let mut value = candidate(&request);
        value.correlations[0] = bad;
        assert_eq!(
            request.validate_candidate(&value),
            Err(GenerativePresenterRefusal::InvalidSemanticCorrelation)
        );
    }
    let mut duplicate = candidate(&request);
    duplicate
        .correlations
        .push(duplicate.correlations[0].clone());
    assert_eq!(
        request.validate_candidate(&duplicate),
        Err(GenerativePresenterRefusal::DuplicateSemanticCorrelation)
    );
    let mut action = candidate(&request);
    action
        .correlations
        .retain(|item| !matches!(item, GeneratedSemanticCorrelation::Action { .. }));
    assert_eq!(
        request.validate_candidate(&action),
        Err(GenerativePresenterRefusal::UncorrelatedAction)
    );
}

#[test]
fn terminal_dispositions_have_no_output() {
    let request = request();
    let mut value = candidate(&request);
    value.disposition = GeneratedManifestationDisposition::Refused;
    assert_eq!(
        request.validate_candidate(&value),
        Err(GenerativePresenterRefusal::OutputForTerminalDisposition)
    );
    value.content.clear();
    value.affordances.clear();
    value.correlations.clear();
    value.candidate_identity = value.digest();
    request.validate_candidate(&value).unwrap();
}

#[test]
fn receipt_rejects_digest_provenance_presentation_and_mask_mismatch() {
    let request = request();
    let candidate = candidate(&request);
    let mut wrong = assessment(&candidate);
    wrong.candidate_digest = "sha256:wrong".into();
    assert_eq!(
        session().retain_unplanned(&request, candidate.clone(), wrong),
        Err(GeneratedValidationError::CandidateDigestMismatch)
    );
    let mut wrong = assessment(&candidate);
    wrong.source_presentation_revision += 1;
    assert_eq!(
        session().retain_unplanned(&request, candidate.clone(), wrong),
        Err(GeneratedValidationError::PresentationMismatch)
    );
    assert_eq!(
        GeneratedValidationSession::selected(
            "validator/semantic@1".into(),
            "back/semantic-validator@1".into(),
            "mask/other@1".into(),
            "mask/spoken-contract@1".into()
        )
        .unwrap()
        .retain_unplanned(&request, candidate.clone(), assessment(&candidate)),
        Err(GeneratedValidationError::MaskMismatch)
    );
    let mut wrong = assessment(&candidate);
    wrong.disposition = GeneratedValidationDisposition::Refused;
    wrong.refusal_code = Some("unsupported-claim".into());
    wrong.accepted_correlations.clear();
    let outcome = session()
        .retain_unplanned(&request, candidate, wrong)
        .unwrap();
    let GeneratedValidationOutcome::Terminal(receipt) = outcome else {
        panic!("terminal outcome")
    };
    assert_eq!(
        receipt.disposition(),
        GeneratedValidationDisposition::Refused
    );
    assert_eq!(receipt.terminal_code(), Some("unsupported-claim"));
}

#[test]
fn temporal_reference_and_fact_correlations_are_exact() {
    let instant = TemporalInstant {
        ticks: 10,
        scale: TemporalScale::Seconds,
        clock_basis: "clock/lesson".into(),
        resolution_ticks: 1,
        uncertainty_ticks: 0,
    };
    let reference = TemporalReference {
        identity: "time/now".into(),
        instant: instant.clone(),
    };
    let fact = PresentationTemporalFact::new(
        "concept/cell".into(),
        PresentationTemporalRole::Observation,
        None,
        instant,
        &reference,
    )
    .unwrap();
    let presentation = Presentation::new_with_semantics_and_temporal(
        5,
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
        vec![PresentationSubject {
            identity: "concept/cell".into(),
            role: PresentationRole::Semantic(kind_id("biology/cell@1")),
            name: "Cell".into(),
        }],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![reference],
        vec![fact],
    )
    .unwrap();
    let request = GenerativePresenterRequest::from_presentation(
        "request/time".into(),
        GenerativePresenterPolicy {
            template_contract_revision: "template/1".into(),
            narrator_role: GenerativeNarratorRole::TransientFirstPersonBodyNarrator,
            instructions: "Use exact time".into(),
        },
        presentation,
        None,
        GenerativePresenterBounds::reviewed_default(),
    )
    .unwrap();
    crate::generated_correlation::validate_correlation(
        &request,
        &GeneratedSemanticCorrelation::TemporalReference {
            index: 0,
            identity: "time/now".into(),
        },
    )
    .unwrap();
    crate::generated_correlation::validate_correlation(
        &request,
        &GeneratedSemanticCorrelation::TemporalFact {
            index: 0,
            subject: "concept/cell".into(),
            reference: "time/now".into(),
            role: PresentationTemporalRole::Observation,
        },
    )
    .unwrap();
}
