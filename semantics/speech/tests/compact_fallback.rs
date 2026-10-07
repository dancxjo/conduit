//! Compact Source pronunciation has no parser/model admission prerequisite.
//! These voice events carry spelling uncertainty and punctuation boundaries;
//! they provide no dependency graph, discourse fact, or language commitment.
use conduit_speech::{
    pronounce, EnglishPosition, EnglishPronunciationOrigin, EnglishStress, Renderer, VoiceBoundary,
    VoiceEvent, MAXIMUM_EVENTS,
};

#[test]
fn no_parser_punctuation_fallback_remains_executable_and_uncertain() {
    let mut storage = [VoiceEvent::boundary(VoiceBoundary::word); MAXIMUM_EVENTS];
    let prepared = pronounce("Hello, Travis.", &mut storage).unwrap();
    assert_eq!(prepared.source(), "Hello, Travis.");
    let boundaries = prepared
        .events()
        .iter()
        .filter_map(|event| match event {
            VoiceEvent::boundary(boundary) => Some(*boundary),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(boundaries, [VoiceBoundary::phrase, VoiceBoundary::turn]);
    let spelling = prepared
        .events()
        .iter()
        .filter_map(|event| match event {
            VoiceEvent::pronounced(segment)
                if segment.origin == EnglishPronunciationOrigin::spelling_rule =>
            {
                Some(segment)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(!spelling.is_empty());
    assert!(spelling.iter().all(|segment| segment.realization.stress
        == EnglishStress::unspecified
        && segment.source_scalar_start >= 7
        && segment.source_scalar_end <= 13));
    let mut renderer = Renderer::prepare(prepared.events()).unwrap();
    let mut block = [0i16; 80];
    let mut frames = 0;
    let mut audible = false;
    while !renderer.is_complete() {
        let count = renderer.render(&mut block).unwrap();
        frames += count;
        audible |= block[..count].iter().any(|sample| *sample != 0);
    }
    assert!(frames > 0 && audible);
}

#[test]
fn fallback_word_traversal_does_not_split_answer_into_dictionary_substrings() {
    let mut storage = [VoiceEvent::boundary(VoiceBoundary::word); MAXIMUM_EVENTS];
    let prepared = pronounce("answer", &mut storage).unwrap();
    assert_eq!(
        prepared.events().last(),
        Some(&VoiceEvent::boundary(VoiceBoundary::turn))
    );
    let segments = prepared.events()[..prepared.events().len() - 1]
        .iter()
        .map(|event| match event {
            VoiceEvent::pronounced(segment) => segment,
            _ => panic!("one word must not acquire an internal boundary"),
        })
        .collect::<Vec<_>>();
    assert_eq!(segments.first().unwrap().source_scalar_start, 0);
    assert_eq!(segments.last().unwrap().source_scalar_end, 6);
    assert_eq!(
        segments
            .iter()
            .filter(|segment| segment.realization.position == EnglishPosition::initial)
            .count(),
        1
    );
    assert_eq!(
        segments
            .iter()
            .filter(|segment| segment.realization.position == EnglishPosition::r#final)
            .count(),
        1
    );
}
