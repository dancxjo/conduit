use super::*;
use conduit_body::{Body, EmergencyMachineAction, EmergencyStepOutcome};
use conduit_core::{
    BaseEnforcementClass, BaseImplementationId, BaseInstanceId, BaseProviderEntry, CheckedFormId,
    HostBaseId, HostBaseKindId, SignId, SourceDocumentId,
};
use conduit_emergency_keyword_spotter::{extract_features, FeatureVector, TEMPLATE_FRAMES};

type WordAudio = [[i16; SAMPLES_PER_FRAME]; TEMPLATE_FRAMES];

fn audio(amplitude: i16, period: usize) -> WordAudio {
    core::array::from_fn(|frame| {
        core::array::from_fn(|sample| {
            let value = amplitude + frame as i16 * 20;
            if (sample / period).is_multiple_of(2) {
                value
            } else {
                -value
            }
        })
    })
}

fn template(word_id: u8, audio: &WordAudio) -> KeywordTemplate {
    let frames: [FeatureVector; TEMPLATE_FRAMES] =
        core::array::from_fn(|index| extract_features(&audio[index]));
    KeywordTemplate {
        word_id,
        frames,
        maximum_distance: 500,
    }
}

fn words_and_templates() -> ([WordAudio; TARGET_WORDS], [KeywordTemplate; TARGET_WORDS]) {
    let words = [audio(1_000, 16), audio(2_000, 10), audio(3_000, 6)];
    let templates = [
        template(0, &words[0]),
        template(1, &words[1]),
        template(2, &words[2]),
    ];
    (words, templates)
}

fn configuration() -> DurableEmergencyConfiguration {
    DurableEmergencyConfiguration {
        revision: 1,
        key: conduit_body::EmergencyKey::from_admitted_entropy([0, 1, 2]).unwrap(),
        detector_version: PROFILE_ID.into(),
        entropy_provider_id: "base/entropy/generation-7".into(),
        acoustic_availability: EmergencyAcousticAvailability::Ready,
    }
}

fn body_id() -> BodyId {
    Body::born(
        SourceDocumentId::from("source/acoustic-emergency"),
        CheckedFormId::from("checked/acoustic-emergency"),
        1,
        SignId::from("sign/acoustic-emergency-born"),
    )
    .unwrap()
    .body_id
}

fn policy() -> EmergencyPolicy {
    EmergencyPolicy {
        allow_keyboard_rescue: false,
        allow_physical: false,
        allow_acoustic: true,
        allow_remote: false,
        attempt_graceful_lull: true,
        revoke_local_execution: true,
        isolate_carriers: true,
        terminal_action: EmergencyMachineAction::Halt,
    }
}

fn provider() -> BaseProviderEntry {
    BaseProviderEntry {
        base_id: HostBaseId::from("base/microphone"),
        provider_instance_id: BaseInstanceId::from("base/microphone/instance-9"),
        provider_generation: 9,
        implementation_id: BaseImplementationId::from("std/alsa-capture@1"),
        mechanism_family: HostBaseKindId::from(ALSA_MICROPHONE_BASE_KIND),
        enforcement_class: BaseEnforcementClass::Cooperative,
        lifecycle: BaseLifecycle::Ready,
        capabilities: Vec::new(),
        resources: Vec::new(),
    }
}

fn basis() -> AcousticMicrophoneBasis {
    AcousticMicrophoneBasis::from_registry_entry(&provider(), false).unwrap()
}

fn fixture() -> (AcousticEmergencyAdapter, [WordAudio; TARGET_WORDS]) {
    let (words, templates) = words_and_templates();
    let adapter = AcousticEmergencyAdapter::admit(
        &configuration(),
        basis(),
        body_id(),
        HostId::from("host/one"),
        BootId::from("boot/one"),
        policy(),
        templates,
    )
    .unwrap();
    (adapter, words)
}

fn feed_word(
    adapter: &mut AcousticEmergencyAdapter,
    audio: &WordAudio,
    first_sequence: u64,
) -> AcousticEmergencyDecision {
    let mut decision = AcousticEmergencyDecision::Waiting;
    for (offset, samples) in audio.iter().enumerate() {
        decision = adapter.observe_validated_frame(
            "base/microphone/instance-9",
            ValidatedPcmFrame {
                microphone_generation: 9,
                sequence: first_sequence + offset as u64,
                samples,
            },
        );
    }
    decision
}

#[test]
fn exact_phrase_requests_only_admitted_authority_reductions_once() {
    let (mut adapter, words) = fixture();
    assert_eq!(
        feed_word(&mut adapter, &words[0], 1),
        AcousticEmergencyDecision::PhraseAdvanced
    );
    assert_eq!(
        feed_word(&mut adapter, &words[1], 9),
        AcousticEmergencyDecision::PhraseAdvanced
    );
    let AcousticEmergencyDecision::AuthorityReductionRequested(outcome) =
        feed_word(&mut adapter, &words[2], 17)
    else {
        panic!("complete phrase did not request authority reduction");
    };
    assert_eq!(
        outcome.trigger,
        EmergencyTriggerClass::LocalAcousticEmergency
    );
    assert_eq!(outcome.graceful_lull, EmergencyStepOutcome::Requested);
    assert_eq!(
        outcome.local_execution_revocation,
        EmergencyStepOutcome::Requested
    );
    assert_eq!(outcome.carrier_isolation, EmergencyStepOutcome::Requested);
    assert_eq!(outcome.machine_action, EmergencyMachineAction::Halt);
    assert_eq!(
        adapter.observe_validated_frame(
            "base/microphone/instance-9",
            ValidatedPcmFrame {
                microphone_generation: 9,
                sequence: 25,
                samples: &words[0][0],
            },
        ),
        AcousticEmergencyDecision::Suppressed
    );
}

#[test]
fn mute_loss_replacement_and_overflow_fail_closed_without_retry() {
    let (mut muted, words) = fixture();
    feed_word(&mut muted, &words[0], 1);
    assert_eq!(
        muted.physically_muted(),
        AcousticEmergencyDecision::Unavailable(AcousticEmergencyAvailability::PhysicallyMuted)
    );
    assert_eq!(
        feed_word(&mut muted, &words[1], 9),
        AcousticEmergencyDecision::Unavailable(AcousticEmergencyAvailability::PhysicallyMuted)
    );

    let (mut lost, words) = fixture();
    assert_eq!(
        lost.provider_lost(),
        AcousticEmergencyDecision::Unavailable(AcousticEmergencyAvailability::ProviderUnavailable)
    );
    assert_eq!(
        feed_word(&mut lost, &words[0], 1),
        AcousticEmergencyDecision::Unavailable(AcousticEmergencyAvailability::ProviderUnavailable)
    );

    let (mut replaced, words) = fixture();
    assert_eq!(
        replaced.observe_validated_frame(
            "base/microphone/instance-10",
            ValidatedPcmFrame {
                microphone_generation: 10,
                sequence: 1,
                samples: &words[0][0],
            },
        ),
        AcousticEmergencyDecision::Unavailable(AcousticEmergencyAvailability::ProviderReplaced)
    );

    let (mut overflow, _) = fixture();
    assert_eq!(
        overflow.input_overflow(),
        AcousticEmergencyDecision::Unavailable(AcousticEmergencyAvailability::InputOverflow)
    );
}

#[test]
fn admission_requires_current_unmuted_provider_and_exact_birth_contract() {
    let (_, templates) = words_and_templates();
    for (provider, muted, expected) in [
        (provider(), true, AcousticEmergencyRefusal::PhysicallyMuted),
        (
            BaseProviderEntry {
                lifecycle: BaseLifecycle::Lost,
                ..provider()
            },
            false,
            AcousticEmergencyRefusal::InvalidProvider,
        ),
    ] {
        assert!(matches!(
            AcousticMicrophoneBasis::from_registry_entry(&provider, muted),
            Err(error) if error == expected
        ));
    }
    let mut wrong_detector = configuration();
    wrong_detector.detector_version = "other/detector@1".into();
    assert!(matches!(
        AcousticEmergencyAdapter::admit(
            &wrong_detector,
            basis(),
            body_id(),
            HostId::from("host/one"),
            BootId::from("boot/one"),
            policy(),
            templates,
        ),
        Err(AcousticEmergencyRefusal::WrongDetector)
    ));
    let (adapter, _) = fixture();
    assert_eq!(adapter.availability(), AcousticEmergencyAvailability::Ready);
    assert_eq!(adapter.provider_id(), "base/microphone/instance-9");
    assert_eq!(adapter.provider_generation(), 9);
}
