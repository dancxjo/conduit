//! Real text traversal preserves stem/suffix evidence and native allomorphs.
use conduit_speech::{
    pronounce, EnglishPhoneme as P, EnglishPronunciationOrigin as O, EnglishStress as S, Renderer,
    TextRefusal, VoiceBoundary, VoiceEvent, MAXIMUM_EVENTS,
};
fn storage() -> [VoiceEvent; MAXIMUM_EVENTS] {
    [VoiceEvent::boundary(VoiceBoundary::word); MAXIMUM_EVENTS]
}
#[test]
fn regular_suffixes_follow_the_final_phoneme_and_preserve_stem_stress_and_source_spans() {
    for (word, stem_bytes, ending) in [
        ("conduits", 7, vec![P::s]),
        ("worlds", 5, vec![P::z]),
        ("voices", 5, vec![P::ih, P::z]),
        ("devices", 6, vec![P::ih, P::z]),
        ("speeches", 6, vec![P::ih, P::z]),
        ("thanked", 5, vec![P::t]),
        ("listened", 6, vec![P::d]),
        ("wanted", 4, vec![P::ih, P::d]),
        ("voiced", 5, vec![P::t]),
        ("pleased", 6, vec![P::d]),
    ] {
        let mut buffer = storage();
        let prepared = pronounce(word, &mut buffer).unwrap();
        let values: Vec<_> = prepared
            .events()
            .iter()
            .filter_map(|event| match event {
                VoiceEvent::pronounced(value) => Some(value),
                _ => None,
            })
            .collect();
        let derived: Vec<_> = values
            .iter()
            .filter(|value| value.origin == O::inflection_rule)
            .collect();
        assert_eq!(
            derived
                .iter()
                .map(|value| value.realization.phoneme)
                .collect::<Vec<_>>(),
            ending,
            "{word}"
        );
        assert!(derived
            .iter()
            .all(|value| value.source_scalar_start == stem_bytes
                && value.source_scalar_end == word.len() as u32));
        if derived.len() == 2 {
            assert_eq!(derived[0].realization.stress, S::unstressed);
        }
        let mut base_buffer = storage();
        let base = pronounce(&word[..stem_bytes as usize], &mut base_buffer).unwrap();
        let base: Vec<_> = base
            .events()
            .iter()
            .filter_map(|event| match event {
                VoiceEvent::pronounced(value) => Some(value),
                _ => None,
            })
            .collect();
        assert_eq!(values.len(), base.len() + ending.len());
        for (value, original) in values.iter().zip(base) {
            assert_eq!(value.origin, O::dictionary);
            assert_eq!(value.realization.phoneme, original.realization.phoneme);
            assert_eq!(value.realization.stress, original.realization.stress);
            assert_eq!(
                (value.source_scalar_start, value.source_scalar_end),
                (0, stem_bytes)
            );
        }
    }
}
#[test]
fn exact_words_keep_priority_and_undeclared_stems_keep_spelling_origin() {
    for word in ["thanks", "this", "is", "want"] {
        let mut buffer = storage();
        let prepared = pronounce(word, &mut buffer).unwrap();
        assert!(prepared.events().iter().all(
            |event| !matches!(event, VoiceEvent::pronounced(value) if value.origin != O::dictionary)
        ));
    }
    for word in ["as", "s", "ed", "readed", "speak ed", "stopped", "foxes"] {
        let mut buffer = storage();
        let prepared = pronounce(word, &mut buffer).unwrap();
        assert!(prepared.events().iter().all(|event| !matches!(event, VoiceEvent::pronounced(value) if value.origin == O::inflection_rule)), "{word}");
    }
}
#[test]
fn suffix_ranges_are_scalar_offsets_in_the_original_capitalized_sentence() {
    let mut buffer = storage();
    let prepared = pronounce("Voices, WANTED!", &mut buffer).unwrap();
    let ranges: Vec<_> = prepared
        .events()
        .iter()
        .filter_map(|event| match event {
            VoiceEvent::pronounced(value) if value.origin == O::inflection_rule => {
                Some((value.source_scalar_start, value.source_scalar_end))
            }
            _ => None,
        })
        .collect();
    assert_eq!(ranges, [(5, 6), (5, 6), (12, 14), (12, 14)]);
}
#[test]
fn expanded_endings_respect_atomic_capacity_and_pcm_chunk_invariance() {
    let original = VoiceEvent::boundary(VoiceBoundary::phrase);
    let mut short = [original; 1];
    assert!(matches!(
        pronounce("wanted", &mut short),
        Err(TextRefusal::OutputSpace)
    ));
    assert_eq!(short, [original]);
    let mut buffer = storage();
    let before = buffer;
    assert!(matches!(
        pronounce(&"voices ".repeat(60), &mut buffer),
        Err(TextRefusal::EventBound)
    ));
    assert_eq!(buffer, before);
    let prepared = pronounce("Conduits, voices. Thanked, wanted.", &mut buffer).unwrap();
    let pcm = |block| {
        let mut renderer = Renderer::prepare(prepared.events()).unwrap();
        let mut result = Vec::new();
        let mut frames = [0; 128];
        while !renderer.is_complete() {
            let count = renderer.render(&mut frames[..block]).unwrap();
            result.extend_from_slice(&frames[..count]);
        }
        result
    };
    assert_eq!(pcm(1), pcm(128));
}
