use super::*;
use crate::{
    GenerativeNarratorRole, GenerativePresenterBounds, GenerativePresenterPolicy, Presentation,
    PresentationAction, PresentationActionAvailability, PresentationBasis,
    PresentationCompositionKind, PresentationCompositionRelation, PresentationContextBasis,
    PresentationDisclosure, PresentationDisclosureLevel, PresentationInput,
    PresentationInteractionContext, PresentationProperty, PresentationPropertyValue,
    PresentationRelationship, PresentationRelationshipKind, PresentationRole, PresentationSubject,
    PresentationTemporalFact, PresentationTemporalRole, PresentationText, TemporalInstant,
    TemporalReference, TemporalScale,
};
use alloc::{string::String, vec};
use conduit_core::{
    kind_id, BoundedResourceRef, ResourceClassId, ResourceExtent, ResourceLifetime,
    ResourceSemanticIdentity, ResourceVersionIdentity,
};

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
    let presentation = Presentation::new_with_interactions(
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
            disclosure: PresentationDisclosureLevel::CurrentAction,
            availability: PresentationActionAvailability::Available,
        }],
        vec![PresentationInput {
            identity: "lesson/answer-input".into(),
            target: "concept/mitochondrion".into(),
            value_kind: "value/text".into(),
            maximum_bytes: 128,
            allow_empty: false,
            name: "Answer".into(),
            submit_action: "lesson/answer".into(),
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
            GeneratedSemanticCorrelation::Input {
                index: 0,
                identity: "lesson/answer-input".into(),
                target: "concept/mitochondrion".into(),
                value_kind: "value/text".into(),
                submit_action: "lesson/answer".into(),
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

#[test]
fn accepted_output_requires_exact_typed_content_composition_and_action_correlations() {
    let request = request();
    let candidate = candidate(&request);
    let outcome = session()
        .retain(&request, candidate.clone(), assessment(&candidate))
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
        session().retain(&request, candidate.clone(), wrong),
        Err(GeneratedValidationError::CandidateDigestMismatch)
    );
    let mut wrong = assessment(&candidate);
    wrong.source_presentation_revision += 1;
    assert_eq!(
        session().retain(&request, candidate.clone(), wrong),
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
        .retain(&request, candidate.clone(), assessment(&candidate)),
        Err(GeneratedValidationError::MaskMismatch)
    );
    let mut wrong = assessment(&candidate);
    wrong.disposition = GeneratedValidationDisposition::Refused;
    wrong.refusal_code = Some("unsupported-claim".into());
    wrong.accepted_correlations.clear();
    let outcome = session().retain(&request, candidate, wrong).unwrap();
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
