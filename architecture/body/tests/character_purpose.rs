use conduit_body::{
    derive_character_context, derive_fulfillment_readiness, CharacterOrientationCue,
    CharacterProfile, CharacterPurposeRefusal, FulfillmentReadiness,
    FulfillmentReadinessReasonKind, PurposeCompletionPolicy, PurposeObligation,
    PurposeObligationState, PurposeState, MAX_PURPOSE_OBLIGATIONS,
};
use conduit_core::SignId;

fn sign(value: &str) -> SignId {
    SignId::from(value)
}

fn obligation(id: &str, state: PurposeObligationState) -> PurposeObligation {
    PurposeObligation {
        obligation_id: id.into(),
        summary: format!("Complete {id}"),
        state,
    }
}

fn purpose(obligations: Vec<PurposeObligation>) -> PurposeState {
    PurposeState {
        purpose_id: "purpose/tutorial@1".into(),
        revision: 7,
        summary: "Teach one real Body lifecycle".into(),
        completion_policy: PurposeCompletionPolicy::ExplicitFulfillmentReadiness,
        obligations,
    }
}

#[test]
fn readiness_is_derived_only_from_exact_finite_obligation_truth() {
    let pending = purpose(vec![
        obligation(
            "born",
            PurposeObligationState::Satisfied {
                evidence_sign_ids: vec![sign("sign/born")],
            },
        ),
        obligation(
            "repair",
            PurposeObligationState::RepairRequired {
                failure_sign_id: sign("sign/failure"),
            },
        ),
        obligation(
            "invite-host",
            PurposeObligationState::CompletionEvidenceMissing,
        ),
    ]);
    let readiness = derive_fulfillment_readiness(&pending).unwrap();
    let FulfillmentReadiness::NotReady { reasons, .. } = readiness else {
        panic!("unfinished exact purpose must not become ready");
    };
    assert_eq!(reasons.len(), 2);
    assert_eq!(
        reasons[0].kind,
        FulfillmentReadinessReasonKind::RepairRequired
    );
    assert_eq!(
        reasons[1].kind,
        FulfillmentReadinessReasonKind::CompletionEvidenceMissing
    );

    let complete = purpose(vec![
        obligation(
            "born",
            PurposeObligationState::Satisfied {
                evidence_sign_ids: vec![sign("sign/born")],
            },
        ),
        obligation(
            "repair",
            PurposeObligationState::Satisfied {
                evidence_sign_ids: vec![sign("sign/repaired")],
            },
        ),
        obligation(
            "invite-host",
            PurposeObligationState::Satisfied {
                evidence_sign_ids: vec![sign("sign/host-joined")],
            },
        ),
    ]);
    assert!(matches!(
        derive_fulfillment_readiness(&complete).unwrap(),
        FulfillmentReadiness::Ready {
            purpose_revision: 7,
            ..
        }
    ));
}

#[test]
fn character_orients_toward_work_repair_and_investigation_not_termination() {
    let state = purpose(vec![
        obligation("unfinished", PurposeObligationState::Pending),
        obligation(
            "fault",
            PurposeObligationState::RepairRequired {
                failure_sign_id: sign("sign/fault"),
            },
        ),
        obligation(
            "uncertain",
            PurposeObligationState::Uncertain {
                evidence_sign_ids: vec![sign("sign/tentative")],
            },
        ),
        obligation(
            "conflict",
            PurposeObligationState::Disputed {
                evidence_sign_ids: vec![sign("sign/open"), sign("sign/closed")],
            },
        ),
    ]);
    let context = derive_character_context(
        &CharacterProfile::purposeful_completion("character/orifina@1".into(), 1),
        &state,
    )
    .unwrap();
    assert_eq!(
        context.cues,
        vec![
            CharacterOrientationCue::ContinueUsefulWork,
            CharacterOrientationCue::RepairBeforeCompletion,
            CharacterOrientationCue::InvestigateCompletion,
            CharacterOrientationCue::PreserveDisagreement,
        ]
    );
    assert!(!context
        .cues
        .contains(&CharacterOrientationCue::WelcomeAppropriateFulfillment));
}

#[test]
fn appropriate_fulfillment_orientation_appears_only_after_evidenced_completion() {
    let complete = purpose(vec![obligation(
        "done",
        PurposeObligationState::Satisfied {
            evidence_sign_ids: vec![sign("sign/done")],
        },
    )]);
    let context = derive_character_context(
        &CharacterProfile::purposeful_completion("character/orifina@1".into(), 1),
        &complete,
    )
    .unwrap();
    assert_eq!(
        context.cues,
        vec![CharacterOrientationCue::WelcomeAppropriateFulfillment]
    );
}

#[test]
fn no_fulfillment_condition_remains_first_class_for_long_running_bodies() {
    let mut ongoing = purpose(vec![obligation(
        "observe",
        PurposeObligationState::Satisfied {
            evidence_sign_ids: vec![sign("sign/observed")],
        },
    )]);
    ongoing.completion_policy = PurposeCompletionPolicy::NoFulfillmentCondition;
    assert!(matches!(
        derive_fulfillment_readiness(&ongoing).unwrap(),
        FulfillmentReadiness::Unavailable { .. }
    ));
}

#[test]
fn malformed_unbounded_and_unsupported_evidence_refuses() {
    let mut duplicate = purpose(vec![
        obligation("same", PurposeObligationState::Pending),
        obligation("same", PurposeObligationState::Pending),
    ]);
    assert_eq!(
        duplicate.validate(),
        Err(CharacterPurposeRefusal::DuplicateObligation)
    );

    duplicate.obligations = (0..=MAX_PURPOSE_OBLIGATIONS)
        .map(|index| {
            obligation(
                &format!("obligation-{index}"),
                PurposeObligationState::Pending,
            )
        })
        .collect();
    assert_eq!(
        duplicate.validate(),
        Err(CharacterPurposeRefusal::ObligationBound)
    );

    let missing = purpose(vec![obligation(
        "claimed",
        PurposeObligationState::Satisfied {
            evidence_sign_ids: vec![],
        },
    )]);
    assert_eq!(
        missing.validate(),
        Err(CharacterPurposeRefusal::MissingEvidence)
    );

    let unsupported_dispute = purpose(vec![obligation(
        "disputed",
        PurposeObligationState::Disputed {
            evidence_sign_ids: vec![sign("sign/only-one-side")],
        },
    )]);
    assert_eq!(
        unsupported_dispute.validate(),
        Err(CharacterPurposeRefusal::MissingEvidence)
    );
}

#[test]
fn semantic_records_have_no_host_presenter_or_lifecycle_authority_fields() {
    let state = purpose(vec![obligation("wait", PurposeObligationState::Pending)]);
    let context = derive_character_context(
        &CharacterProfile::purposeful_completion("character/orifina@1".into(), 1),
        &state,
    )
    .unwrap();
    let encoded = serde_json::to_value(context).unwrap();
    for forbidden in [
        "host_id",
        "boot_id",
        "provider_id",
        "model_id",
        "prompt",
        "authority",
        "fulfill_action",
    ] {
        assert!(encoded.get(forbidden).is_none());
    }
}
