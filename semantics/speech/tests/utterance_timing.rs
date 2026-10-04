#![cfg(feature = "semantic-bindings")]
use conduit_core::IeeeF32;
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::{semantic::*, utterance_timing::*};
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
        PhoneSpecification::unknown(),
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
fn phone() -> VoiceEvent {
    VoiceEvent::phone(SpeechPhoneInput {
        phone: C::iy,
        stress: EnglishStress::primary,
    })
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
#[test]
fn mixed_timeline_retains_exact_sources_and_one_cumulative_grid() {
    let source = utterance(vec![
        segment(),
        boundary(
            SpeechBoundarySpecification::known(SpeechBoundaryKind::Word).unwrap(),
            SpeechDurationSpecification::known(30, 1).unwrap(),
        ),
        segment(),
    ]);
    let profile = profile(vec![binding()]);
    let prepared = prepare_utterance_timing(&source, &profile).unwrap();
    assert!(core::ptr::eq(prepared.source(), &source));
    assert_eq!(
        prepared
            .spans()
            .iter()
            .map(|s| *s.frame_count())
            .collect::<Vec<_>>(),
        [266, 267, 267]
    );
    let SpeechUtteranceIntentEvent::Boundary(original) = &source.events().as_slice()[1] else {
        panic!()
    };
    let EventTimingReceipt::Boundary {
        intent,
        binding,
        checked,
        ..
    } = &prepared.receipts()[1]
    else {
        panic!()
    };
    assert!(core::ptr::eq(*intent, original));
    assert!(core::ptr::eq(*binding, &profile.get().as_slice()[0]));
    assert_eq!(checked.requested(), &SpeechBoundaryKind::Word);
    let SpeechUtteranceIntentEvent::Segment(original) = &source.events().as_slice()[2] else {
        panic!()
    };
    let EventTimingReceipt::Segment { intent, control } = &prepared.receipts()[2] else {
        panic!()
    };
    assert!(core::ptr::eq(*intent, original));
    assert_eq!(control.cycle().unwrap().whole_q8(), &17066);
    let events = [phone(), VoiceEvent::boundary(VoiceBoundary::word), phone()];
    let expected = pcm(prepared.renderer(&events).unwrap(), 128);
    assert_eq!(expected.len(), 800);
    assert!(expected[266..533].iter().all(|s| *s == 0));
    assert!(expected[..266].iter().any(|s| *s != 0));
    assert!(expected[533..].iter().any(|s| *s != 0));
    assert_eq!(pcm(prepared.renderer(&events).unwrap(), 1), expected);
    assert_eq!(pcm(prepared.renderer(&events).unwrap(), 63), expected);
    // A rejected staged block does not mutate the retained admitted cursor.
    let live = prepared.renderer(&events).unwrap();
    let mut candidate = live;
    let mut block = [0; 63];
    candidate.render(&mut block).unwrap();
    assert_eq!(live.rendered_frames(), 0);
    assert_eq!(pcm(live, 128), expected);
}
#[test]
fn event_shape_count_and_binding_refusals_remain_distinct() {
    let source = utterance(vec![
        segment(),
        boundary(
            SpeechBoundarySpecification::known(SpeechBoundaryKind::Word).unwrap(),
            SpeechDurationSpecification::known(30, 1).unwrap(),
        ),
    ]);
    let empty = profile(vec![]);
    assert!(matches!(
        prepare_utterance_timing(&source, &empty),
        Err(UtteranceTimingRefusal::MissingBoundaryBinding { event: 1 })
    ));
    let duplicate = profile(vec![binding(), binding()]);
    assert!(matches!(
        prepare_utterance_timing(&source, &duplicate),
        Err(UtteranceTimingRefusal::AmbiguousBoundaryBinding { event: 1 })
    ));
    let profile = profile(vec![binding()]);
    let prepared = prepare_utterance_timing(&source, &profile).unwrap();
    assert!(matches!(
        prepared.renderer(&[]),
        Err(UtteranceTimingRenderRefusal::EventCount)
    ));
    assert!(matches!(
        prepared.renderer(&[phone(), phone()]),
        Err(UtteranceTimingRenderRefusal::EventShape { event: 1 })
    ));
    assert!(matches!(
        prepared.renderer(&[phone(), VoiceEvent::boundary(VoiceBoundary::turn)]),
        Err(UtteranceTimingRenderRefusal::EventShape { event: 1 })
    ));
    assert!(matches!(
        prepared.renderer(&[
            VoiceEvent::boundary(VoiceBoundary::word),
            VoiceEvent::boundary(VoiceBoundary::word)
        ]),
        Err(UtteranceTimingRenderRefusal::EventShape { event: 0 })
    ));
}
#[test]
fn unresolved_boundary_quantities_keep_the_original_global_event() {
    let profile = profile(vec![binding()]);
    for state in [
        SpeechDurationSpecification::unknown(),
        SpeechDurationSpecification::unspecified(),
        SpeechDurationSpecification::not_applicable(),
        SpeechDurationSpecification::variable(
            BoundedSequence::try_from_iter([SpeechExactDuration::new(30, 1).unwrap()]).unwrap(),
        )
        .unwrap(),
        SpeechDurationSpecification::gradient(
            SpeechConfidence::new(IeeeF32::from_value(0.25)).unwrap(),
            SpeechExactDuration::new(30, 1).unwrap(),
        )
        .unwrap(),
    ] {
        let source = utterance(vec![
            segment(),
            boundary(
                SpeechBoundarySpecification::known(SpeechBoundaryKind::Word).unwrap(),
                state.clone(),
            ),
        ]);
        match prepare_utterance_timing(&source, &profile) {
            Err(UtteranceTimingRefusal::BoundaryDuration {
                event,
                specification,
            }) => {
                assert_eq!(event, 1);
                assert_eq!(specification, state)
            }
            _ => panic!(),
        }
    }
}
#[test]
fn empty_and_zero_duration_boundary_have_exact_terminal_behavior() {
    let profile = profile(vec![binding()]);
    let empty = utterance(vec![]);
    let prepared = prepare_utterance_timing(&empty, &profile).unwrap();
    assert!(prepared.renderer(&[]).unwrap().is_complete());
    let source = utterance(vec![boundary(
        SpeechBoundarySpecification::known(SpeechBoundaryKind::Word).unwrap(),
        SpeechDurationSpecification::known(1, 0).unwrap(),
    )]);
    let prepared = prepare_utterance_timing(&source, &profile).unwrap();
    assert_eq!(prepared.spans()[0].frame_count(), &0);
    let events = [VoiceEvent::boundary(VoiceBoundary::word)];
    assert!(pcm(prepared.renderer(&events).unwrap(), 128).is_empty());
}

#[test]
fn boundary_kinds_and_late_segment_refusals_keep_global_identity() {
    let profile = profile(vec![binding()]);
    for state in [
        SpeechBoundarySpecification::unknown(),
        SpeechBoundarySpecification::unspecified(),
        SpeechBoundarySpecification::not_applicable(),
        SpeechBoundarySpecification::variable(
            BoundedSequence::try_from_iter([SpeechBoundaryKind::Word]).unwrap(),
        )
        .unwrap(),
        SpeechBoundarySpecification::gradient(
            SpeechConfidence::new(IeeeF32::from_value(0.25)).unwrap(),
            SpeechBoundaryKind::Word,
        )
        .unwrap(),
    ] {
        let source = utterance(vec![
            segment(),
            boundary(
                state.clone(),
                SpeechDurationSpecification::known(30, 1).unwrap(),
            ),
        ]);
        match prepare_utterance_timing(&source, &profile) {
            Err(UtteranceTimingRefusal::BoundaryKind {
                event,
                specification,
            }) => {
                assert_eq!(event, 1);
                assert_eq!(specification, state)
            }
            _ => panic!("unresolved boundary kind must remain unresolved"),
        }
    }
    let SpeechUtteranceIntentEvent::Segment(original) = segment() else {
        panic!()
    };
    let later = SpeechUtteranceIntentEvent::segment(
        original.occurrence().clone(),
        original.phone().clone(),
        original.phoneme().clone(),
        SpeechSegmentProsodyIntent::new(
            original.prosody().duration().clone(),
            SpeechCycleSpecification::unknown(),
            original.prosody().relative_intensity().clone(),
        )
        .unwrap(),
        original.provenance().clone(),
        original.sources().clone(),
        original.stress().clone(),
        original.word_position().clone(),
    )
    .unwrap();
    let source = utterance(vec![
        segment(),
        boundary(
            SpeechBoundarySpecification::known(SpeechBoundaryKind::Word).unwrap(),
            SpeechDurationSpecification::known(30, 1).unwrap(),
        ),
        later,
    ]);
    assert!(matches!(
        prepare_utterance_timing(&source, &profile),
        Err(UtteranceTimingRefusal::Segment(
            conduit_speech::intent_prosody::IntentProsodyRefusal::Cycle { event: 2, .. }
        ))
    ));
    assert!(
        BoundedSequence::<SpeechFormantBoundaryBinding, 3>::try_from_iter([
            binding(),
            binding(),
            binding(),
            binding()
        ])
        .is_err()
    );
}
