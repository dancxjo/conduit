use conduit_speech::{
    pronounce, EnglishPhoneme as P, EnglishPosition as W, EnglishPronunciationOrigin as O,
    EnglishStress as S, Renderer, TextRefusal, VoiceBoundary as B, VoiceEvent, MAXIMUM_EVENTS,
};
fn segments(events: &[VoiceEvent]) -> Vec<conduit_speech::TextSpeechSegment> {
    events
        .iter()
        .filter_map(|event| {
            if let VoiceEvent::pronounced(value) = event {
                Some(*value)
            } else {
                None
            }
        })
        .collect()
}
#[test]
fn dictionary_and_spelling_decisions_preserve_source_ranges_and_uncertainty() {
    let mut storage = [VoiceEvent::boundary(B::word); MAXIMUM_EVENTS];
    let batch = pronounce("Hello, chat!", &mut storage).unwrap();
    assert_eq!(batch.source(), "Hello, chat!");
    let values = segments(batch.events());
    assert_eq!(
        values
            .iter()
            .map(|value| value.realization.phoneme)
            .collect::<Vec<_>>(),
        [P::h, P::ax, P::l, P::ow, P::ch, P::ae, P::t]
    );
    assert!(values[..4].iter().all(|value| value.origin == O::dictionary
        && value.source_scalar_start == 0
        && value.source_scalar_end == 5));
    assert_eq!(values[3].realization.stress, S::primary);
    assert!(values[4..].iter().all(
        |value| value.origin == O::spelling_rule && value.realization.stress == S::unspecified
    ));
    assert_eq!(
        (values[4].source_scalar_start, values[4].source_scalar_end),
        (7, 9)
    );
    assert_eq!(values[4].realization.position, W::initial);
    assert_eq!(values[6].realization.position, W::r#final);
}
#[test]
fn refusal_is_atomic_and_never_returns_a_silently_truncated_prefix() {
    for (source, expected) in [
        (
            "hello é".into(),
            TextRefusal::UnsupportedCharacter {
                scalar: 6,
                codepoint: 233,
            },
        ),
        (
            "hello @".into(),
            TextRefusal::UnsupportedCharacter {
                scalar: 6,
                codepoint: 64,
            },
        ),
        ("a".repeat(33), TextRefusal::WordBound),
        ("x ".repeat(100), TextRefusal::EventBound),
        (" ".repeat(513), TextRefusal::InputBound),
    ] {
        let mut storage = [VoiceEvent::boundary(B::phrase); MAXIMUM_EVENTS];
        assert!(matches!(pronounce(&source, &mut storage), Err(error) if error == expected));
        assert!(storage
            .iter()
            .all(|value| *value == VoiceEvent::boundary(B::phrase)));
    }
    let mut short = [VoiceEvent::boundary(B::phrase); 1];
    assert!(matches!(
        pronounce("hello", &mut short),
        Err(TextRefusal::OutputSpace)
    ));
    assert_eq!(short, [VoiceEvent::boundary(B::phrase)]);
}
#[test]
fn punctuation_decisions_and_empty_text_remain_explicit() {
    let mut storage = [VoiceEvent::boundary(B::word); MAXIMUM_EVENTS];
    assert!(pronounce("", &mut storage).unwrap().events().is_empty());
    let batch = pronounce("   hello   world!!", &mut storage).unwrap();
    let boundaries = batch
        .events()
        .iter()
        .filter_map(|event| {
            if let VoiceEvent::boundary(value) = event {
                Some(*value)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(boundaries, [B::word, B::turn]);
}
#[test]
fn source_metadata_does_not_change_the_realization_or_block_invariance() {
    let mut storage = [VoiceEvent::boundary(B::word); MAXIMUM_EVENTS];
    let batch = pronounce("The quick fox can chat and sing.", &mut storage).unwrap();
    let explicit = batch
        .events()
        .iter()
        .map(|event| match event {
            VoiceEvent::pronounced(value) => VoiceEvent::segment(value.realization),
            event => *event,
        })
        .collect::<Vec<_>>();
    let mut left = Renderer::prepare(batch.events()).unwrap();
    let mut right = Renderer::prepare(&explicit).unwrap();
    let mut block = [0_i16; 128];
    while !left.is_complete() {
        let count = left.render(&mut block).unwrap();
        for expected in &block[..count] {
            let mut sample = [0_i16; 1];
            assert_eq!(right.render(&mut sample).unwrap(), 1);
            assert_eq!(sample[0], *expected);
        }
    }
    assert!(right.is_complete());
}
