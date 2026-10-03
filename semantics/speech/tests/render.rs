use conduit_speech::{
    EnglishPhoneme as P, EnglishPosition as W, EnglishStress as S, RealizationInput, RenderRefusal,
    Renderer, VoiceEvent,
};
fn events() -> [VoiceEvent; 12] {
    [
        P::h,
        P::eh,
        P::l,
        P::p,
        P::ay,
        P::m,
        P::n,
        P::r,
        P::t,
        P::ow,
        P::k,
        P::oy,
    ]
    .map(|phoneme| {
        VoiceEvent::segment(RealizationInput {
            phoneme,
            position: W::medial,
            stress: S::primary,
        })
    })
}
fn render(events: &[VoiceEvent], block: usize) -> Vec<i16> {
    let mut renderer = Renderer::prepare(events).unwrap();
    let mut output = Vec::new();
    let mut samples = [0; 128];
    while !renderer.is_complete() {
        let count = renderer.render(&mut samples[..block]).unwrap();
        output.extend_from_slice(&samples[..count]);
    }
    assert_eq!(renderer.rendered_frames(), renderer.total_frames());
    output
}
#[test]
fn output_is_invariant_to_block_boundaries_and_pressure_can_discard_a_candidate() {
    let events = events();
    let expected = render(&events, 128);
    for block in [1, 63, 96, 127] {
        assert_eq!(render(&events, block), expected);
    }
    let mut committed = Renderer::prepare(&events).unwrap();
    let mut candidate = committed;
    let mut refused = [0; 128];
    candidate.render(&mut refused).unwrap();
    assert_eq!(committed.rendered_frames(), 0);
    let mut accepted = [0; 128];
    committed.render(&mut accepted).unwrap();
    assert_eq!(accepted, refused);
}
#[test]
fn bounds_refuse_before_rendering_and_empty_input_completes() {
    let event = events()[0];
    assert!(matches!(
        Renderer::prepare(&[event; 257]),
        Err(RenderRefusal::EventBound)
    ));
    assert!(matches!(
        Renderer::prepare(&[VoiceEvent::boundary(conduit_speech::VoiceBoundary::turn); 256]),
        Err(RenderRefusal::DurationBound)
    ));
    let bound = [event];
    let mut renderer = Renderer::prepare(&bound).unwrap();
    assert_eq!(
        renderer.render(&mut [0; 129]),
        Err(RenderRefusal::OutputBound)
    );
    assert_eq!(renderer.rendered_frames(), 0);
    let mut empty = Renderer::prepare(&[]).unwrap();
    assert!(empty.is_complete());
    assert_eq!(empty.render(&mut [0; 128]).unwrap(), 0);
}
#[test]
fn voiced_and_fricated_segments_are_not_silent_or_identical() {
    let make = |phoneme| {
        [VoiceEvent::segment(RealizationInput {
            phoneme,
            position: W::medial,
            stress: S::primary,
        })]
    };
    let vowel = render(&make(P::iy), 128);
    let fricative = render(&make(P::sh), 128);
    assert!(vowel.iter().any(|value| value.unsigned_abs() > 200));
    assert!(fricative.iter().any(|value| value.unsigned_abs() > 200));
    assert_ne!(&vowel[..640], fricative);
}

#[test]
fn realization_preserves_the_input_and_distinguishes_allophone_derivation() {
    use conduit_speech::{realize, EnglishDerivation as D, EnglishPhone};
    let initial = RealizationInput {
        phoneme: P::t,
        stress: S::primary,
        position: W::initial,
    };
    let result = realize(initial).unwrap();
    assert_eq!(result.input, initial);
    assert_eq!(result.phone, EnglishPhone::t_aspirated);
    assert_eq!(result.derivation, D::allophone_rule);
    let uncertain = RealizationInput {
        stress: S::unknown,
        ..initial
    };
    let result = realize(uncertain).unwrap();
    assert_eq!(result.input, uncertain);
    assert_eq!(result.phone, EnglishPhone::t);
    assert_eq!(result.derivation, D::voice_profile);
}

#[test]
fn selected_realizations_match_frontend_results_across_neighbors_and_blocks() {
    let input = events();
    let selected = input.map(|event| {
        VoiceEvent::selected(conduit_speech::realize(event.realization().unwrap()).unwrap())
    });
    let expected = render(&input, 128);
    for block in [1, 63, 128] {
        assert_eq!(render(&selected, block), expected);
    }
}

#[test]
fn explicit_phone_is_not_reselected_from_its_phoneme() {
    use conduit_speech::{realize, EnglishPhone};
    let input = RealizationInput {
        phoneme: P::t,
        stress: S::primary,
        position: W::initial,
    };
    let mut selected = realize(input).unwrap();
    assert_eq!(selected.phone, EnglishPhone::t_aspirated);
    selected.phone = EnglishPhone::t;
    let events = [VoiceEvent::selected(selected), events()[1]];
    // The selected plain stop must differ from the frontend's aspirated stop,
    // both in its own waveform and as the following segment's exact neighbor.
    let frontend = [VoiceEvent::segment(input), events[1]];
    let selected_pcm = render(&events, 63);
    let frontend_pcm = render(&frontend, 63);
    assert_ne!(selected_pcm, frontend_pcm);
    let plain = [
        VoiceEvent::segment(RealizationInput {
            position: W::medial,
            ..input
        }),
        events[1],
    ];
    assert_eq!(selected_pcm, render(&plain, 128));
    assert_eq!(events[0], VoiceEvent::selected(selected));
    assert_eq!(events[0].realization(), Some(input));
}
