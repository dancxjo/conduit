use conduit_body::{
    Body, BodyCharacterPurpose, BodyFulfillment, CharacterOrientationCue, CharacterProfile,
    CharacterPurposeContinuityRefusal, FulfillmentObligation, PurposeCompletionPolicy,
    PurposeObligation, PurposeObligationState, PurposeState,
};
use conduit_core::{
    bind_active_play, seal_plan, AuthorityGrantId, CheckedFormId, ExpandedFormId, FormIdentity,
    HostId, Plan, SignId, SourceDocumentId,
};

fn body() -> Body {
    Body::born(
        SourceDocumentId::from("source/orifina"),
        CheckedFormId::from("checked/orifina"),
        1,
        SignId::from("sign/born"),
    )
    .unwrap()
}

fn obligation(state: PurposeObligationState) -> PurposeObligation {
    PurposeObligation {
        obligation_id: "tutorial-finished".into(),
        summary: "Finish the real Body tutorial".into(),
        state,
    }
}

fn purpose(revision: u64, state: PurposeObligationState) -> PurposeState {
    PurposeState {
        purpose_id: "purpose/orifina-tutorial@1".into(),
        revision,
        summary: "Teach one real Body lifecycle".into(),
        completion_policy: PurposeCompletionPolicy::ExplicitFulfillmentReadiness,
        obligations: vec![obligation(state)],
    }
}

fn plan(identity: &str) -> Plan {
    seal_plan(
        FormIdentity {
            source_document_id: SourceDocumentId::from("source/orifina"),
            checked_form_id: CheckedFormId::from("checked/orifina"),
            expanded_form_id: ExpandedFormId::from(identity),
        },
        vec![],
    )
}

#[test]
fn character_and_purpose_survive_replan_host_change_lull_and_new_wake() {
    let initial = body();
    let orientation = BodyCharacterPurpose::establish(
        &initial,
        CharacterProfile::purposeful_completion("character/orifina@1".into(), 1),
        purpose(1, PurposeObligationState::Pending),
    )
    .unwrap();
    let retained_truth = orientation.clone();
    let retained_truth: BodyCharacterPurpose =
        serde_json::from_slice(&serde_json::to_vec(&retained_truth).unwrap()).unwrap();
    let (awake, wake) = initial.wake(1, SignId::from("sign/woke")).unwrap();
    let first = plan("expanded/first");
    let wake = wake
        .plan_ready(&first, SignId::from("sign/planned-first"))
        .unwrap();
    let first_play = bind_active_play(
        &first.plan_id,
        &HostId::from("host/first"),
        &"boot/first".into(),
        1,
    );
    let wake = wake
        .play_started(&first_play, SignId::from("sign/playing-first"))
        .unwrap()
        .became_unsatisfied(&first.plan_id, SignId::from("sign/unsatisfied"))
        .unwrap();
    let replacement = plan("expanded/replacement");
    let wake = wake
        .plan_ready(&replacement, SignId::from("sign/planned-replacement"))
        .unwrap();
    let replacement_play = bind_active_play(
        &replacement.plan_id,
        &HostId::from("host/replacement"),
        &"boot/replacement".into(),
        2,
    );
    let wake = wake
        .play_started(&replacement_play, SignId::from("sign/playing-replacement"))
        .unwrap();
    let lulled = wake.lull(SignId::from("sign/lulled")).unwrap();
    let retained = awake
        .retain_after_lull(&lulled, SignId::from("sign/retained"))
        .unwrap();
    let (rewoken, _) = retained.wake(2, SignId::from("sign/rewoken")).unwrap();

    assert_eq!(orientation, retained_truth);
    orientation.validate_for(&rewoken).unwrap();
    assert_eq!(
        orientation.active_context(&rewoken).unwrap().cues,
        vec![CharacterOrientationCue::ContinueUsefulWork]
    );
}

#[test]
fn purpose_revision_is_explicit_and_cannot_change_character() {
    let body = body();
    let orientation = BodyCharacterPurpose::establish(
        &body,
        CharacterProfile::purposeful_completion("character/orifina@1".into(), 4),
        purpose(7, PurposeObligationState::Pending),
    )
    .unwrap();
    let next = orientation
        .revise_purpose(purpose(
            8,
            PurposeObligationState::Satisfied {
                evidence_sign_ids: vec![SignId::from("sign/tutorial-finished")],
            },
        ))
        .unwrap();

    assert_eq!(next.character, orientation.character);
    assert_eq!(next.purpose.revision, 8);
    assert_eq!(
        orientation.revise_purpose(purpose(9, PurposeObligationState::Pending)),
        Err(CharacterPurposeContinuityRefusal::StalePurposeRevision)
    );
}

#[test]
fn fulfilled_body_retains_inspection_but_has_no_active_character_context() {
    let body = body();
    let orientation = BodyCharacterPurpose::establish(
        &body,
        CharacterProfile::purposeful_completion("character/orifina@1".into(), 1),
        purpose(
            1,
            PurposeObligationState::Satisfied {
                evidence_sign_ids: vec![SignId::from("sign/tutorial-finished")],
            },
        ),
    )
    .unwrap();
    let fulfilled = body
        .fulfill(
            BodyFulfillment {
                final_wake_id: None,
                authority_grant_id: AuthorityGrantId::from("authority/fulfill/1"),
                attribution: "operator/travis".into(),
                settled_obligations: vec![FulfillmentObligation {
                    obligation_id: "tutorial-finished".into(),
                    settlement_sign_id: SignId::from("sign/tutorial-finished"),
                }],
            },
            SignId::from("sign/fulfilled"),
        )
        .unwrap();

    assert_eq!(
        orientation.active_context(&fulfilled),
        Err(CharacterPurposeContinuityRefusal::BodyFulfilled)
    );
    assert_eq!(
        orientation.inspect_context().unwrap().cues,
        vec![CharacterOrientationCue::WelcomeAppropriateFulfillment]
    );
}

#[test]
fn orientation_state_contains_no_host_model_provider_or_authority_identity() {
    let source = include_str!("../src/character_purpose_continuity.rs");
    for forbidden in [
        "HostId",
        "BootId",
        "PlanId",
        "model_id",
        "provider_id",
        "AuthorityGrantId",
        "fulfill(",
    ] {
        assert!(!source.contains(forbidden));
    }
}
