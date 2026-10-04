#![cfg(feature = "semantic-bindings")]
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::semantic::*;
use conduit_speech::{
    EnglishPhone as C, EnglishStress, Renderer, SpeechPhoneInput, VoiceBoundary, VoiceEvent,
};

fn provenance() -> SpeechEvidenceProvenance {
    SpeechEvidenceProvenance::new("timing fixture".into(), SpeechEvidenceSource::Manual, None)
        .unwrap()
}
fn sources() -> BoundedSequence<LanguageSegmentRef, 8> {
    BoundedSequence::try_from_iter([LanguageSegmentRef::text(
        LanguageTextSegmentKind::Word,
        SpeechLanguageId::new("en".into()).unwrap(),
        ListeningTextRange::new(1, 0).unwrap(),
        LanguageTextRevisionId::new("source revision".into()).unwrap(),
        LanguageTextId::new("source".into()).unwrap(),
    )
    .unwrap()])
    .unwrap()
}
fn segment() -> SpeechUtteranceIntentEvent {
    SpeechUtteranceIntentEvent::segment(
        LanguageSpeechTokenRef::new(
            SpeechInventoryId::new("inventory".into()).unwrap(),
            SpeechLanguageId::new("en".into()).unwrap(),
            0,
            SpeechSegmentRevisionId::new("revision".into()).unwrap(),
            SpeechSegmentSequenceId::new("phones".into()).unwrap(),
            SpeechUtteranceId::new("utterance".into()).unwrap(),
        )
        .unwrap(),
        PhoneSpecification::known(PhoneId::new("opaque/t".into()).unwrap()).unwrap(),
        PhonemeSpecification::unspecified(),
        SpeechSegmentProsodyIntent::new(
            SpeechDurationSpecification::known(30, 1).unwrap(),
            SpeechCycleSpecification::known(120, 1).unwrap(),
            SpeechIntensitySpecification::known(1, 1).unwrap(),
        )
        .unwrap(),
        provenance(),
        sources(),
        StressSpecification::unknown(),
        SpeechPositionSpecification::unspecified(),
    )
    .unwrap()
}
fn boundary(
    kind: SpeechBoundarySpecification,
    duration: SpeechDurationSpecification,
) -> SpeechUtteranceIntentEvent {
    SpeechUtteranceIntentEvent::boundary(duration, kind, provenance(), sources()).unwrap()
}
fn utterance(events: Vec<SpeechUtteranceIntentEvent>) -> SpeechUtteranceIntent {
    SpeechUtteranceIntent::new(
        BoundedSequence::try_from_iter(events).unwrap(),
        SpeechInventoryId::new("inventory".into()).unwrap(),
        SpeechLanguageId::new("en".into()).unwrap(),
        provenance(),
        SpeechSegmentRevisionId::new("revision".into()).unwrap(),
        SpeechUtteranceId::new("utterance".into()).unwrap(),
    )
    .unwrap()
}
fn binding() -> SpeechFormantBoundaryBinding {
    SpeechFormantBoundaryBinding::new(SpeechBoundaryKind::Word, SpeechFormantBoundary::Word)
        .unwrap()
}
fn profile(bindings: Vec<SpeechFormantBoundaryBinding>) -> SpeechFormantBoundaryProfile {
    SpeechFormantBoundaryProfile::new(BoundedSequence::try_from_iter(bindings).unwrap()).unwrap()
}
fn pcm(mut renderer: Renderer<'_>, block: usize) -> Vec<i16> {
    let mut buffer = [0; 128];
    let mut result = Vec::new();
    while !renderer.is_complete() {
        let n = renderer.render(&mut buffer[..block]).unwrap();
        result.extend_from_slice(&buffer[..n]);
    }
    result
}

#[allow(dead_code)]
#[path = "common/native_phone.rs"]
mod fixture;
use conduit_speech::intent_realization::*;
fn inventory() -> SpeechInventory {
    fixture::inventory("inventory", "en", vec![fixture::definition("opaque/t")])
}
fn voice() -> SpeechFormantVoiceProfile {
    SpeechFormantVoiceProfile::new(
        "fixture".into(),
        SpeechInventoryId::new("inventory".into()).unwrap(),
        SpeechLanguageId::new("en".into()).unwrap(),
        BoundedSequence::try_from_iter([SpeechFormantPhoneBinding::new(
            fixture::definition("opaque/t"),
            EnglishPhone::T,
        )
        .unwrap()])
        .unwrap(),
    )
    .unwrap()
}
#[test]
fn original_identity_and_pcm_survive_ordered_realization() {
    let source = utterance(vec![
        segment(),
        boundary(
            SpeechBoundarySpecification::known(SpeechBoundaryKind::Word).unwrap(),
            SpeechDurationSpecification::known(30, 1).unwrap(),
        ),
        segment(),
    ]);
    let inventory = inventory();
    let voice = voice();
    let boundaries = profile(vec![binding()]);
    let prepared = prepare_intent_realization(&source, &inventory, &voice, &boundaries).unwrap();
    assert!(core::ptr::eq(prepared.source(), &source));
    assert!(core::ptr::eq(prepared.profile(), &voice));
    assert_eq!(
        prepared
            .phones()
            .iter()
            .map(|p| p.source().event())
            .collect::<Vec<_>>(),
        [0, 2]
    );
    for receipt in prepared.phones() {
        let SpeechUtteranceIntentEvent::Segment(original) =
            &source.events().as_slice()[receipt.source().event()]
        else {
            panic!()
        };
        assert!(core::ptr::eq(receipt.source().segment(), original));
        assert!(core::ptr::eq(
            receipt.binding(),
            &voice.phones().as_slice()[0]
        ));
        assert!(core::ptr::eq(
            receipt.source().definition(),
            &inventory.phones().as_slice()[0]
        ));
    }
    let direct = VoiceEvent::phone(SpeechPhoneInput {
        phone: C::t,
        stress: EnglishStress::unknown,
    });
    let events = [direct, VoiceEvent::boundary(VoiceBoundary::word), direct];
    assert_eq!(prepared.events(), events);
    let expected = pcm(prepared.timing().renderer(&events).unwrap(), 128);
    assert_eq!(pcm(prepared.renderer().unwrap(), 1), expected);
    assert_eq!(pcm(prepared.renderer().unwrap(), 63), expected);
    assert_eq!(expected.len(), 800);
    assert!(expected[266..533].iter().all(|s| *s == 0));
}
#[test]
fn late_phone_and_profile_errors_return_no_partial_preparation() {
    let source = utterance(vec![
        boundary(
            SpeechBoundarySpecification::known(SpeechBoundaryKind::Word).unwrap(),
            SpeechDurationSpecification::known(30, 1).unwrap(),
        ),
        segment(),
    ]);
    let boundaries = profile(vec![binding()]);
    let voice = voice();
    let absent = fixture::inventory("inventory", "en", vec![]);
    assert!(matches!(
        prepare_intent_realization(&source, &absent, &voice, &boundaries),
        Err(IntentRealizationRefusal::Phone {
            event: 1,
            reason: conduit_speech::intent_inventory::IntentInventoryRefusal::MissingDefinition
        })
    ));
    let inventory = inventory();
    let empty_voice = SpeechFormantVoiceProfile::new(
        "empty".into(),
        inventory.identity().clone(),
        inventory.language().clone(),
        BoundedSequence::new(),
    )
    .unwrap();
    assert!(matches!(
        prepare_intent_realization(&source, &inventory, &empty_voice, &boundaries),
        Err(IntentRealizationRefusal::Profile {
            event: 1,
            reason: conduit_speech::profile_admission::ProfileRefusal::UnsupportedPhone
        })
    ));
}
#[test]
fn empty_realization_is_terminal() {
    let source = utterance(vec![]);
    let inventory = inventory();
    let voice = voice();
    let boundaries = profile(vec![]);
    let prepared = prepare_intent_realization(&source, &inventory, &voice, &boundaries).unwrap();
    assert!(prepared.renderer().unwrap().is_complete());
    assert!(prepared.phones().is_empty());
}

#[test]
fn unresolved_phone_keeps_original_state_and_global_index() {
    for state in [
        PhoneSpecification::unknown(),
        PhoneSpecification::unspecified(),
        PhoneSpecification::not_applicable(),
    ] {
        let SpeechUtteranceIntentEvent::Segment(original) = segment() else {
            panic!()
        };
        let unresolved = SpeechUtteranceIntentEvent::segment(
            original.occurrence().clone(),
            state.clone(),
            original.phoneme().clone(),
            original.prosody().clone(),
            original.provenance().clone(),
            original.sources().clone(),
            original.stress().clone(),
            original.word_position().clone(),
        )
        .unwrap();
        let source = utterance(vec![
            boundary(
                SpeechBoundarySpecification::known(SpeechBoundaryKind::Word).unwrap(),
                SpeechDurationSpecification::known(30, 1).unwrap(),
            ),
            unresolved,
        ]);
        let inventory = inventory();
        let voice = voice();
        let boundaries = profile(vec![binding()]);
        match prepare_intent_realization(&source, &inventory, &voice, &boundaries) {
            Err(IntentRealizationRefusal::Phone {
                event,
                reason: conduit_speech::intent_inventory::IntentInventoryRefusal::Unresolved(actual),
            }) => {
                assert_eq!(event, 1);
                assert_eq!(actual, state);
            }
            _ => panic!("unresolved phone must refuse"),
        }
    }
}
