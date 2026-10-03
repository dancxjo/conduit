use conduit_speech::{
    pronounce, EnglishPosition as W, EnglishPronunciationOrigin as O, Renderer, TextRefusal,
    VoiceBoundary as B, VoiceEvent, MAXIMUM_EVENTS,
};

#[test]
fn digit_names_keep_each_original_scalar_and_lexical_realization() {
    let mut digits = [VoiceEvent::boundary(B::word); MAXIMUM_EVENTS];
    let input = "0123456789";
    let prepared = pronounce(input, &mut digits).unwrap();
    assert_eq!(prepared.source(), input);
    for (index, name) in [
        "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine",
    ]
    .iter()
    .enumerate()
    {
        let mut words = [VoiceEvent::boundary(B::word); MAXIMUM_EVENTS];
        let word = pronounce(name, &mut words).unwrap();
        let expected: Vec<_> = word
            .events()
            .iter()
            .filter_map(|event| match event {
                VoiceEvent::pronounced(value) => Some(value.realization),
                _ => None,
            })
            .collect();
        let actual: Vec<_> = prepared
            .events()
            .iter()
            .filter_map(|event| match event {
                VoiceEvent::pronounced(value) if value.source_scalar_start == index as u32 => {
                    Some(*value)
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            actual
                .iter()
                .map(|value| value.realization)
                .collect::<Vec<_>>(),
            expected
        );
        assert!(actual
            .iter()
            .all(|value| value.origin == O::digit_name
                && value.source_scalar_end == index as u32 + 1));
        assert_eq!(actual.first().unwrap().realization.position, W::initial);
        assert_eq!(actual.last().unwrap().realization.position, W::r#final);
    }
    let boundaries: Vec<_> = prepared
        .events()
        .iter()
        .filter_map(|event| match event {
            VoiceEvent::boundary(value) => Some(*value),
            _ => None,
        })
        .collect();
    assert_eq!(
        boundaries.iter().filter(|value| **value == B::word).count(),
        10
    );
    assert_eq!(boundaries.last(), Some(&B::turn));
}

#[test]
fn normalization_preflight_preserves_atomic_refusal() {
    for (text, expected) in [
        (
            "2é".to_owned(),
            TextRefusal::UnsupportedCharacter {
                scalar: 1,
                codepoint: 233,
            },
        ),
        ("0".repeat(100), TextRefusal::EventBound),
    ] {
        let mut storage = [VoiceEvent::boundary(B::phrase); MAXIMUM_EVENTS];
        assert!(matches!(pronounce(&text, &mut storage), Err(error) if error == expected));
        assert!(storage
            .iter()
            .all(|event| *event == VoiceEvent::boundary(B::phrase)));
    }
    let mut short = [VoiceEvent::boundary(B::phrase); 1];
    assert!(matches!(
        pronounce("1", &mut short),
        Err(TextRefusal::OutputSpace)
    ));
    assert_eq!(short, [VoiceEvent::boundary(B::phrase)]);
}

#[test]
fn normalized_digit_audio_is_block_invariant() {
    let mut storage = [VoiceEvent::boundary(B::word); MAXIMUM_EVENTS];
    let prepared = pronounce("a007b", &mut storage).unwrap();
    let render = |block| {
        let mut renderer = Renderer::prepare(prepared.events()).unwrap();
        let mut output = vec![];
        let mut samples = [0; 128];
        while !renderer.is_complete() {
            let count = renderer.render(&mut samples[..block]).unwrap();
            output.extend_from_slice(&samples[..count]);
        }
        output
    };
    let expected = render(128);
    assert_eq!(render(1), expected);
    assert_eq!(render(127), expected);
    let digit_spans: Vec<_> = prepared
        .events()
        .iter()
        .filter_map(|event| match event {
            VoiceEvent::pronounced(value) if value.origin == O::digit_name => {
                Some((value.source_scalar_start, value.source_scalar_end))
            }
            _ => None,
        })
        .collect();
    assert!(digit_spans
        .iter()
        .all(|span| [(1, 2), (2, 3), (3, 4)].contains(span)));
    assert!(
        digit_spans.contains(&(1, 2))
            && digit_spans.contains(&(2, 3))
            && digit_spans.contains(&(3, 4))
    );
}
