use super::*;

fn finite_wording_candidate(
    request: &GenerativePresenterRequest,
) -> GeneratedManifestationCandidate {
    let proposal = GeneratedWordingProposal {
        source_presentation_identity: request.semantic_data.source_presentation_identity.clone(),
        source_presentation_revision: request.semantic_data.source_presentation_revision,
        clauses: vec![
            GeneratedWordingClause::Text {
                index: 0,
                subject: "concept/mitochondrion".into(),
                value: "Produces cellular energy".into(),
                style: GeneratedWordingStyle::Guided,
            },
            GeneratedWordingClause::Action {
                index: 0,
                identity: "lesson/answer".into(),
                name: "Answer".into(),
                style: GeneratedWordingStyle::Direct,
            },
        ],
    };
    let speech = proposal
        .render_exact(&request.semantic_data.presentation)
        .unwrap();
    let mut value = candidate(request);
    value.content = vec![GeneratedContentSegment {
        role: GeneratedContentRole::Speech,
        source_text_index: 0,
        bytes: speech.into_bytes(),
    }];
    value.correlations = vec![
        GeneratedSemanticCorrelation::Text {
            index: 0,
            subject: "concept/mitochondrion".into(),
        },
        GeneratedSemanticCorrelation::Action {
            index: 0,
            identity: "lesson/answer".into(),
            intent: "education/answer@1".into(),
            target: "concept/mitochondrion".into(),
        },
    ];
    value.raw_provider_output = Some(
        serde_json::json!({
            "proposal": &proposal,
            "suggested_action_identities": ["lesson/answer"]
        })
        .to_string(),
    );
    value.wording_proposal = Some(proposal);
    value.candidate_identity = value.digest();
    value
}

#[test]
fn finite_model_wording_is_rebuilt_from_exact_face_and_retains_provider_bytes() {
    let mut request = request();
    request.policy.template_contract_revision = FINITE_FACE_WORDING_TEMPLATE_REVISION.into();
    let candidate = finite_wording_candidate(&request);
    assert_eq!(
        candidate.content[0].bytes.as_slice(),
        b"Current message: Produces cellular energy. You can Answer."
    );
    let assessment = assess_generated_output_exactly(
        &GeneratedValidationEnvelope {
            request: request.clone(),
            candidate: candidate.clone(),
        },
        "assessment/wording".into(),
        "mask/spoken@1".into(),
    );
    assert_eq!(
        assessment.disposition,
        GeneratedValidationDisposition::Accepted
    );
    let GeneratedValidationOutcome::Accepted(accepted) = session()
        .retain_unplanned(&request, candidate.clone(), assessment)
        .unwrap()
    else {
        panic!("current finite wording must be accepted")
    };
    assert_eq!(accepted.candidate(), &candidate);
    assert_eq!(
        accepted.validation_receipt().raw_provider_output(),
        candidate.raw_provider_output.as_deref()
    );
    assert_eq!(
        accepted.validation_receipt().candidate_digest(),
        candidate.candidate_identity
    );
}

#[test]
fn finite_wording_rejects_stale_face_invented_claims_and_forged_acceptance() {
    let mut request = request();
    request.policy.template_contract_revision = FINITE_FACE_WORDING_TEMPLATE_REVISION.into();
    for change in ["revision", "text", "action", "speech", "raw"] {
        let mut candidate = finite_wording_candidate(&request);
        match change {
            "revision" => {
                candidate
                    .wording_proposal
                    .as_mut()
                    .unwrap()
                    .source_presentation_revision += 1
            }
            "text" => {
                let GeneratedWordingClause::Text { value, .. } =
                    &mut candidate.wording_proposal.as_mut().unwrap().clauses[0]
                else {
                    unreachable!()
                };
                *value = "Invented cellular energy".into();
            }
            "action" => {
                let GeneratedWordingClause::Action { identity, .. } =
                    &mut candidate.wording_proposal.as_mut().unwrap().clauses[1]
                else {
                    unreachable!()
                };
                *identity = "disk.erase".into();
            }
            "speech" => candidate.content[0].bytes = b"Invented explanation".to_vec(),
            "raw" => {
                candidate.raw_provider_output = Some("x".repeat(MAX_RAW_PRESENTER_OUTPUT_BYTES + 1))
            }
            _ => unreachable!(),
        }
        candidate.candidate_identity = candidate.digest();
        let checked_assessment = assess_generated_output_exactly(
            &GeneratedValidationEnvelope {
                request: request.clone(),
                candidate: candidate.clone(),
            },
            "assessment/refused-wording".into(),
            "mask/spoken@1".into(),
        );
        assert_eq!(
            checked_assessment.disposition,
            GeneratedValidationDisposition::Refused,
            "{change}"
        );
        assert_eq!(
            session().retain_unplanned(&request, candidate.clone(), assessment(&candidate)),
            Err(GeneratedValidationError::InvalidAssessmentContract),
            "{change}"
        );
    }
}

#[test]
fn refused_wording_retains_raw_model_output_without_speech() {
    let mut request = request();
    request.policy.template_contract_revision = FINITE_FACE_WORDING_TEMPLATE_REVISION.into();
    let mut candidate = finite_wording_candidate(&request);
    candidate.disposition = GeneratedManifestationDisposition::Refused;
    candidate.content.clear();
    candidate.correlations.clear();
    candidate.affordances.clear();
    candidate
        .wording_proposal
        .as_mut()
        .unwrap()
        .source_presentation_revision += 1;
    candidate.candidate_identity = candidate.digest();
    let assessment = assess_generated_output_exactly(
        &GeneratedValidationEnvelope {
            request: request.clone(),
            candidate: candidate.clone(),
        },
        "assessment/refused-stale-wording".into(),
        "mask/spoken@1".into(),
    );
    assert_eq!(
        assessment.disposition,
        GeneratedValidationDisposition::Refused
    );
    let GeneratedValidationOutcome::Terminal(receipt) = session()
        .retain_unplanned(&request, candidate.clone(), assessment)
        .unwrap()
    else {
        panic!("stale wording must retain a terminal refusal")
    };
    assert_eq!(
        receipt.raw_provider_output(),
        candidate.raw_provider_output.as_deref()
    );
    assert_eq!(receipt.terminal_code(), Some("not-exactly-reconstructible"));
}

#[test]
fn count_property_wording_uses_only_the_current_face_value() {
    let presentation = Presentation::new_with_semantics(
        9,
        PresentationBasis {
            body_id: None,
            wake_id: None,
            source_document_id: None,
            checked_plot_id: None,
            expanded_plot_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: vec![],
        },
        vec![PresentationSubject {
            identity: "clock/current".into(),
            role: PresentationRole::Status,
            name: "Clock".into(),
        }],
        vec![],
        vec![PresentationProperty {
            subject: "clock/current".into(),
            name: "interval seconds".into(),
            value: PresentationPropertyValue::Count(5),
        }],
        vec![],
        vec![],
        vec![PresentationDisclosure {
            subject: "clock/current".into(),
            level: PresentationDisclosureLevel::Primary,
        }],
    )
    .unwrap();
    let request = GenerativePresenterRequest::from_presentation(
        "request/clock-wording".into(),
        GenerativePresenterPolicy {
            template_contract_revision: FINITE_FACE_WORDING_TEMPLATE_REVISION.into(),
            narrator_role: GenerativeNarratorRole::TransientFirstPersonBodyNarrator,
            instructions: "Choose current clock facts.".into(),
        },
        presentation,
        None,
        GenerativePresenterBounds::reviewed_default(),
    )
    .unwrap();
    let proposal = GeneratedWordingProposal {
        source_presentation_identity: request.semantic_data.source_presentation_identity.clone(),
        source_presentation_revision: 9,
        clauses: vec![GeneratedWordingClause::Property {
            index: 0,
            subject: "clock/current".into(),
            name: "interval seconds".into(),
            value: "5".into(),
            style: GeneratedWordingStyle::Guided,
        }],
    };
    assert_eq!(
        proposal
            .render_exact(&request.semantic_data.presentation)
            .unwrap(),
        "For Clock, interval seconds is 5."
    );
    let mut candidate = candidate(&request);
    candidate.content[0].bytes = proposal
        .render_exact(&request.semantic_data.presentation)
        .unwrap()
        .into_bytes();
    candidate.affordances.clear();
    candidate.correlations = vec![GeneratedSemanticCorrelation::Property {
        index: 0,
        subject: "clock/current".into(),
        name: "interval seconds".into(),
    }];
    candidate.raw_provider_output = Some(
        serde_json::json!({"proposal": &proposal, "suggested_action_identities": []}).to_string(),
    );
    candidate.wording_proposal = Some(proposal);
    candidate.candidate_identity = candidate.digest();
    let envelope = GeneratedValidationEnvelope {
        request: request.clone(),
        candidate: candidate.clone(),
    };
    assert_eq!(
        assess_generated_output_exactly(
            &envelope,
            "assessment/clock".into(),
            "mask/spoken@1".into()
        )
        .disposition,
        GeneratedValidationDisposition::Accepted
    );
    let GeneratedWordingClause::Property { value, .. } =
        &mut candidate.wording_proposal.as_mut().unwrap().clauses[0]
    else {
        unreachable!()
    };
    *value = "6".into();
    candidate.candidate_identity = candidate.digest();
    assert_eq!(
        assess_generated_output_exactly(
            &GeneratedValidationEnvelope { request, candidate },
            "assessment/invented-interval".into(),
            "mask/spoken@1".into(),
        )
        .disposition,
        GeneratedValidationDisposition::Refused
    );
}

#[test]
fn wording_never_offers_an_unavailable_action() {
    let original = request().semantic_data.presentation;
    let mut action = original.actions[0].clone();
    action.availability = PresentationActionAvailability::Unavailable {
        reason_code: "host-offline".into(),
        explanation: "The selected host is offline.".into(),
    };
    let current = Presentation::new_with_semantics(
        original.revision,
        original.basis,
        original.subjects,
        original.relationships,
        original.properties,
        original.text,
        vec![action],
        original.disclosures,
    )
    .unwrap();
    let proposal = GeneratedWordingProposal {
        source_presentation_identity: current.identity.as_str().into(),
        source_presentation_revision: current.revision,
        clauses: vec![GeneratedWordingClause::Action {
            index: 0,
            identity: "lesson/answer".into(),
            name: "Answer".into(),
            style: GeneratedWordingStyle::Guided,
        }],
    };
    assert_eq!(
        proposal.render_exact(&current),
        Err(GeneratedWordingRefusal::UnavailableAction)
    );
}
