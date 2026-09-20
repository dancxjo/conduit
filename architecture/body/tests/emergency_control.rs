use conduit_body::*;
use conduit_core::{BootId, CheckedFormId, HostId, SignId, SourceDocumentId};

fn body_id() -> BodyId {
    Body::born(
        SourceDocumentId::from("source/emergency"),
        CheckedFormId::from("checked/emergency"),
        1,
        SignId::from("sign/emergency-born"),
    )
    .unwrap()
    .body_id
}

fn request(trigger: EmergencyTriggerClass, freshness: u64) -> EmergencyRequest {
    EmergencyRequest {
        request_id: format!("emergency/{freshness}"),
        body_id: body_id(),
        host_id: HostId::from("host/one"),
        boot_id: BootId::from("boot/one"),
        trigger,
        policy_id: EMERGENCY_CONTROL_POLICY.into(),
        freshness,
    }
}

fn control() -> EmergencyControl {
    EmergencyControl::admit(
        body_id(),
        HostId::from("host/one"),
        BootId::from("boot/one"),
        EmergencyPolicy {
            allow_keyboard_rescue: true,
            allow_physical: true,
            allow_acoustic: true,
            allow_remote: false,
            attempt_graceful_lull: true,
            revoke_local_execution: true,
            isolate_carriers: true,
            terminal_action: EmergencyMachineAction::Reset,
        },
    )
}

#[test]
fn accepted_trigger_only_requests_distinct_reductions() {
    let mut control = control();
    let outcome = control
        .inspect(&request(EmergencyTriggerClass::LocalAcousticEmergency, 1))
        .unwrap();
    assert!(control.triggered());
    assert_eq!(outcome.graceful_lull, EmergencyStepOutcome::Requested);
    assert_eq!(
        outcome.local_execution_revocation,
        EmergencyStepOutcome::Requested
    );
    assert_eq!(outcome.carrier_isolation, EmergencyStepOutcome::Requested);
    assert!(outcome.machine_action_requested);
    assert_eq!(outcome.machine_action, EmergencyMachineAction::Reset);
}

#[test]
fn trigger_authority_is_exact_and_one_shot() {
    let mut control = control();
    assert_eq!(
        control.inspect(&request(
            EmergencyTriggerClass::AuthenticatedRemoteEmergency,
            1
        )),
        Err(EmergencyRefusal::TriggerDisabled)
    );
    let mut stale = request(EmergencyTriggerClass::LocalPhysicalEmergency, 1);
    stale.boot_id = BootId::from("boot/old");
    assert_eq!(control.inspect(&stale), Err(EmergencyRefusal::StaleBoot));
    control
        .inspect(&request(EmergencyTriggerClass::LocalPhysicalEmergency, 2))
        .unwrap();
    assert_eq!(
        control.inspect(&request(EmergencyTriggerClass::LocalPhysicalEmergency, 3)),
        Err(EmergencyRefusal::AlreadyTriggered)
    );
}

#[test]
fn key_selection_refuses_duplicates_and_has_no_fallback() {
    assert_eq!(
        EmergencyKey::from_admitted_entropy([1, 1, 2]),
        Err(EmergencyKeyRefusal::DuplicateWord)
    );
    let key = EmergencyKey::from_admitted_entropy([0, 1, 2]).unwrap();
    assert_eq!(key.words().unwrap(), ["copper", "kestrel", "lantern"]);
}

#[test]
fn exact_phrase_triggers_once_and_wrong_order_resets() {
    let key = EmergencyKey::from_admitted_entropy([0, 1, 2]).unwrap();
    let mut matcher = EmergencySequenceMatcher::new(key).unwrap();
    assert_eq!(
        matcher.observe(AcousticObservation::Word {
            word_id: 0,
            ended_at_millis: 10
        }),
        AcousticDecision::Advanced
    );
    assert_eq!(
        matcher.observe(AcousticObservation::Word {
            word_id: 2,
            ended_at_millis: 20
        }),
        AcousticDecision::Reset
    );
    for (word_id, time) in [(0, 30), (1, 40)] {
        assert_eq!(
            matcher.observe(AcousticObservation::Word {
                word_id,
                ended_at_millis: time
            }),
            AcousticDecision::Advanced
        );
    }
    assert_eq!(
        matcher.observe(AcousticObservation::Word {
            word_id: 2,
            ended_at_millis: 50
        }),
        AcousticDecision::Triggered
    );
    assert_eq!(
        matcher.observe(AcousticObservation::Word {
            word_id: 2,
            ended_at_millis: 50
        }),
        AcousticDecision::Suppressed
    );
}

#[test]
fn timeout_loss_stale_audio_and_overflow_fail_closed() {
    let key = EmergencyKey::from_admitted_entropy([0, 1, 2]).unwrap();
    for invalid in [
        AcousticObservation::InvalidConfidence,
        AcousticObservation::Overflow,
        AcousticObservation::MicrophoneLost,
        AcousticObservation::StaleGeneration,
    ] {
        let mut matcher = EmergencySequenceMatcher::new(key.clone()).unwrap();
        matcher.observe(AcousticObservation::Word {
            word_id: 0,
            ended_at_millis: 10,
        });
        let decision = matcher.observe(invalid);
        assert!(matches!(
            decision,
            AcousticDecision::Reset | AcousticDecision::Unavailable
        ));
        assert_eq!(
            matcher.observe(AcousticObservation::Word {
                word_id: 1,
                ended_at_millis: 20
            }),
            AcousticDecision::Reset
        );
    }
    let mut matcher = EmergencySequenceMatcher::new(key).unwrap();
    matcher.observe(AcousticObservation::Word {
        word_id: 0,
        ended_at_millis: 10,
    });
    assert_eq!(
        matcher.observe(AcousticObservation::Word {
            word_id: 1,
            ended_at_millis: 10 + u64::from(MAX_EMERGENCY_WORD_GAP_MILLIS) + 1
        }),
        AcousticDecision::Reset
    );
}

#[test]
fn birth_retains_exact_emergency_configuration_across_durable_roundtrip() {
    let body = Body::born(
        SourceDocumentId::from("source/emergency-biography"),
        CheckedFormId::from("checked/emergency-biography"),
        1,
        SignId::from("sign/emergency-biography-born"),
    )
    .unwrap();
    let membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let configuration = DurableEmergencyConfiguration {
        revision: 1,
        key: EmergencyKey::from_admitted_entropy([0, 1, 2]).unwrap(),
        detector_version: "fixed-keyword-spotter/en-us@1".into(),
        entropy_provider_id: "base/entropy/rdrand/generation-7".into(),
        acoustic_availability: EmergencyAcousticAvailability::Ready,
    };
    let biography = BodyBiographyEvidence::born_with_emergency(
        body,
        membership,
        "Emergency fixture".into(),
        configuration.clone(),
        2,
        SignId::from("sign/emergency-configuration-1"),
    )
    .unwrap();
    let bytes = serde_json::to_vec(&biography).unwrap();
    let restored: BodyBiographyEvidence = serde_json::from_slice(&bytes).unwrap();
    restored.validate().unwrap();
    assert_eq!(restored.emergency, Some(configuration));
}

#[test]
fn phrase_change_requires_monotonic_durable_evidence() {
    let body = Body::born(
        SourceDocumentId::from("source/emergency-change"),
        CheckedFormId::from("checked/emergency-change"),
        1,
        SignId::from("sign/emergency-change-born"),
    )
    .unwrap();
    let membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let first = DurableEmergencyConfiguration {
        revision: 1,
        key: EmergencyKey::from_admitted_entropy([0, 1, 2]).unwrap(),
        detector_version: "fixed-keyword-spotter/en-us@1".into(),
        entropy_provider_id: "base/entropy/rdrand/generation-7".into(),
        acoustic_availability: EmergencyAcousticAvailability::Ready,
    };
    let mut biography = BodyBiographyEvidence::born_with_emergency(
        body,
        membership,
        "Emergency change fixture".into(),
        first,
        2,
        SignId::from("sign/emergency-change-1"),
    )
    .unwrap();
    let mut replacement = biography.emergency.clone().unwrap();
    replacement.revision = 2;
    replacement.key = EmergencyKey::from_admitted_entropy([3, 4, 5]).unwrap();
    biography
        .reconfigure_emergency(
            replacement.clone(),
            3,
            SignId::from("sign/emergency-change-2"),
        )
        .unwrap();
    assert_eq!(biography.emergency, Some(replacement.clone()));

    let mut skipped = replacement;
    skipped.revision = 4;
    assert_eq!(
        biography.reconfigure_emergency(skipped, 4, SignId::from("sign/emergency-change-skipped")),
        Err(BodyBiographyError::InvalidSequence)
    );
}
