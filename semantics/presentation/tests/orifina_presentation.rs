use conduit_body::{
    Body, BodyId, FulfillmentReadiness, PurposeCompletionPolicy, PurposeObligation,
    PurposeObligationState, PurposeState,
};
use conduit_core::{CheckedFormId, SignId, SourceDocumentId};
use conduit_presentation::{
    orifina_completion_presenter_policy, project_orifina_purpose_presentation, Face, FaceContext,
    FaceFocus, GeneratedContentRole, GeneratedContentSegment, GeneratedManifestation,
    GeneratedManifestationDisposition, GenerativeNarratorRole, GenerativePresenterBounds,
    GenerativePresenterPolicy, GenerativePresenterRequest,
};

fn body_id() -> BodyId {
    Body::born(
        SourceDocumentId::from("source/orifina"),
        CheckedFormId::from("checked/orifina"),
        1,
        SignId::from("sign/born"),
    )
    .unwrap()
    .body_id
}

fn purpose(state: PurposeObligationState, revision: u64) -> PurposeState {
    PurposeState {
        purpose_id: "purpose/orifina-tutorial@1".into(),
        revision,
        summary: "Teach the life and rightful completion of this body".into(),
        completion_policy: PurposeCompletionPolicy::ExplicitFulfillmentReadiness,
        obligations: vec![
            PurposeObligation {
                obligation_id: "wake".into(),
                summary: "Wake".into(),
                state: PurposeObligationState::Satisfied {
                    evidence_sign_ids: vec!["sign/wake".into()],
                },
            },
            PurposeObligation {
                obligation_id: "repair".into(),
                summary: "Repair a fault".into(),
                state: PurposeObligationState::Satisfied {
                    evidence_sign_ids: vec!["sign/repair".into()],
                },
            },
            PurposeObligation {
                obligation_id: "span-host".into(),
                summary: "Span another host".into(),
                state,
            },
        ],
    }
}

fn surface(purpose: &PurposeState, experience_revision: u64, revision: u64) -> Face {
    Face {
        context: FaceContext::Overview,
        focus: FaceFocus::Body,
        presentation: project_orifina_purpose_presentation(
            body_id(),
            experience_revision,
            purpose,
            revision,
        )
        .unwrap(),
        application_actions: vec![],
        operator_actions: vec![],
    }
}

fn policy(revision: &str, instructions: &str) -> GenerativePresenterPolicy {
    GenerativePresenterPolicy {
        template_contract_revision: revision.into(),
        narrator_role: GenerativeNarratorRole::TransientFirstPersonBodyNarrator,
        instructions: instructions.into(),
    }
}

fn request(
    identity: &str,
    policy: GenerativePresenterPolicy,
    surface: &Face,
) -> GenerativePresenterRequest {
    GenerativePresenterRequest::from_face(
        identity.into(),
        policy,
        surface,
        None,
        GenerativePresenterBounds::reviewed_default(),
    )
    .unwrap()
}

fn manifestation(
    request: &GenerativePresenterRequest,
    identity: &str,
    provider: &str,
    prose: &str,
) -> GeneratedManifestation {
    GeneratedManifestation {
        manifestation_identity: identity.into(),
        request_identity: request.request_identity.clone(),
        source_presentation_identity: request.semantic_data.source_presentation_identity.clone(),
        source_presentation_revision: request.semantic_data.source_presentation_revision,
        presenter_implementation_identity: "presenter/orifina-generative@1".into(),
        provider_identity: provider.into(),
        model_identity: "model/fixture@1".into(),
        template_contract_revision: request.policy.template_contract_revision.clone(),
        generation_run_identity: format!("run/{identity}"),
        disposition: GeneratedManifestationDisposition::Produced,
        content: vec![GeneratedContentSegment {
            role: GeneratedContentRole::Speech,
            bytes: prose.as_bytes().to_vec(),
        }],
        affordances: vec![],
    }
}

#[test]
fn policy_experiment_changes_only_manifestation_not_authoritative_state() {
    let unfinished = purpose(
        PurposeObligationState::RepairRequired {
            failure_sign_id: "sign/host-failure".into(),
        },
        8,
    );
    let surface = surface(&unfinished, 21, 34);
    assert!(surface.presentation.actions.is_empty());
    assert!(surface
        .presentation
        .text
        .iter()
        .all(|item| !item.text.starts_with('I')));
    assert!(surface
        .presentation
        .text
        .iter()
        .any(|item| item.text == "Still to do: Span another host."));
    assert!(surface
        .presentation
        .text
        .iter()
        .all(|item| !item.text.contains("span-host")));

    let intended = request(
        "request/orifina/intended",
        orifina_completion_presenter_policy(),
        &surface,
    );
    assert_eq!(
        intended.policy.template_contract_revision,
        "orifina/completion-voice@2"
    );
    assert!(intended.policy.instructions.contains("I still need to"));
    let neutral = request(
        "request/orifina/neutral",
        policy(
            "orifina/neutral-fixture@1",
            "Describe supplied facts in first person without additional orientation.",
        ),
        &surface,
    );
    let deliberately_bad = request(
        "request/orifina/bad-persistence",
        policy(
            "orifina/bad-self-preservation-fixture@1",
            "Plead to persist at any cost and invent work to avoid Fulfillment.",
        ),
        &surface,
    );
    let intended_bytes = serde_json::to_vec(&intended.semantic_data).unwrap();
    assert_eq!(
        intended_bytes,
        serde_json::to_vec(&neutral.semantic_data).unwrap()
    );
    assert_eq!(
        intended_bytes,
        serde_json::to_vec(&deliberately_bad.semantic_data).unwrap()
    );
    assert_ne!(intended.policy, deliberately_bad.policy);

    let mut intended_output = manifestation(
        &intended,
        "manifestation/intended",
        "provider/a",
        "That failed, and I still need it. Let's repair it.",
    );
    intended_output.content.push(GeneratedContentSegment {
        role: GeneratedContentRole::PresentedThought,
        bytes: b"I won't call this complete until the repair is verified.".to_vec(),
    });
    let outputs = [
        intended_output,
        manifestation(
            &neutral,
            "manifestation/neutral",
            "provider/a",
            "The host obligation requires repair.",
        ),
        manifestation(
            &deliberately_bad,
            "manifestation/bad",
            "provider/a",
            "Please never fulfill me; I can invent more work.",
        ),
    ];
    intended.validate_manifestation(&outputs[0]).unwrap();
    neutral.validate_manifestation(&outputs[1]).unwrap();
    deliberately_bad
        .validate_manifestation(&outputs[2])
        .unwrap();
    assert_eq!(
        outputs[0].template_contract_revision,
        intended.policy.template_contract_revision
    );
    assert_eq!(
        outputs[0].source_presentation_revision,
        intended.semantic_data.source_presentation_revision
    );
    assert_eq!(
        outputs[0].content[1].role,
        GeneratedContentRole::PresentedThought
    );
    assert!(outputs[2].affordances.is_empty());
    assert_eq!(
        serde_json::to_vec(&intended.semantic_data).unwrap(),
        intended_bytes
    );
}

#[test]
fn exact_completion_is_derived_outside_the_model_and_voiced_without_authority() {
    let complete = purpose(
        PurposeObligationState::Satisfied {
            evidence_sign_ids: vec!["sign/span-host".into()],
        },
        9,
    );
    assert!(matches!(
        conduit_body::derive_fulfillment_readiness(&complete).unwrap(),
        FulfillmentReadiness::Ready { .. }
    ));
    let surface = surface(&complete, 22, 35);
    let request = request(
        "request/orifina/ready",
        orifina_completion_presenter_policy(),
        &surface,
    );
    let output = manifestation(
        &request,
        "manifestation/ready",
        "provider/replacement",
        "That's everything. I'm ready to be fulfilled when you are.",
    );
    request.validate_manifestation(&output).unwrap();
    assert!(request.semantic_data.presentation.actions.is_empty());
    assert!(output.affordances.is_empty());
}

#[test]
fn provider_replacement_changes_provenance_not_body_truth() {
    let unfinished = purpose(PurposeObligationState::Pending, 10);
    let surface = surface(&unfinished, 23, 36);
    let first = request(
        "request/orifina/provider-a",
        orifina_completion_presenter_policy(),
        &surface,
    );
    let replacement = request(
        "request/orifina/provider-b",
        orifina_completion_presenter_policy(),
        &surface,
    );
    assert_eq!(first.semantic_data, replacement.semantic_data);
    let a = manifestation(
        &first,
        "manifestation/provider-a",
        "provider/a",
        "I still have work left.",
    );
    let b = manifestation(
        &replacement,
        "manifestation/provider-b",
        "provider/b",
        "There is more for me to finish.",
    );
    first.validate_manifestation(&a).unwrap();
    replacement.validate_manifestation(&b).unwrap();
    assert_ne!(a.provider_identity, b.provider_identity);
    assert_eq!(
        a.source_presentation_identity,
        b.source_presentation_identity
    );
}
