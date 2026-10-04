use conduit_speech::*;
fn render(mut renderer: Renderer<'_>, block: usize) -> Vec<i16> {
    let mut pcm = Vec::new();
    let mut output = [0; MAXIMUM_BLOCK_FRAMES];
    while !renderer.is_complete() {
        let frames = renderer.render(&mut output[..block]).unwrap();
        pcm.extend_from_slice(&output[..frames]);
    }
    pcm
}
#[test]
fn directly_supplied_phones_preserve_waveforms_without_inventing_phonemes() {
    let mut storage = [VoiceEvent::boundary(VoiceBoundary::word); MAXIMUM_EVENTS];
    let pronounced = pronounce(
        "Hello, world! The quick fox can chat and sing.",
        &mut storage,
    )
    .unwrap();
    let direct: Vec<_> = pronounced
        .events()
        .iter()
        .map(|event| {
            if let Some(input) = event.realization() {
                let selected = realize(input).unwrap();
                let value = VoiceEvent::phone(SpeechPhoneInput {
                    phone: selected.phone,
                    stress: input.stress,
                });
                assert_eq!(value.realization(), None);
                value
            } else {
                *event
            }
        })
        .collect();
    assert_eq!(
        render(Renderer::prepare(&direct).unwrap(), 63),
        render(Renderer::prepare(pronounced.events()).unwrap(), 128)
    );
}
#[test]
fn direct_allophones_are_not_selected_again_and_pressure_preserves_the_cursor() {
    let events = [
        VoiceEvent::phone(SpeechPhoneInput {
            phone: EnglishPhone::alveolar_flap,
            stress: EnglishStress::unstressed,
        }),
        VoiceEvent::phone(SpeechPhoneInput {
            phone: EnglishPhone::iy,
            stress: EnglishStress::primary,
        }),
        VoiceEvent::phone(SpeechPhoneInput {
            phone: EnglishPhone::t_aspirated,
            stress: EnglishStress::unknown,
        }),
    ];
    let original = Renderer::prepare(&events).unwrap();
    let expected = render(original, 128);
    assert_eq!(render(original, 1), expected);
    let mut staged = original;
    let mut refused = [0; 128];
    staged.render(&mut refused).unwrap();
    assert_eq!(original.rendered_frames(), 0);
    let mut accepted = original;
    let mut retried = [0; 128];
    accepted.render(&mut retried).unwrap();
    assert_eq!(refused, retried);
}
#[test]
fn exact_timing_and_voice_controls_apply_to_direct_phone_events() {
    let events = [
        VoiceEvent::phone(SpeechPhoneInput {
            phone: EnglishPhone::iy,
            stress: EnglishStress::primary,
        }),
        VoiceEvent::phone(SpeechPhoneInput {
            phone: EnglishPhone::h,
            stress: EnglishStress::unspecified,
        }),
    ];
    let counts = [1001, 511];
    let controls = [SpeechEventVoiceControl {
        cycle_mode: SpeechCycleControlMode::resolved,
        period_q8: 17066,
        amplitude_q15: 16384,
    }; 2];
    let renderer = Renderer::prepare_timed(&events, &counts)
        .unwrap()
        .with_controls(&controls)
        .unwrap();
    assert_eq!(renderer.total_frames(), 1512);
    assert_eq!(render(renderer, 1), render(renderer, 128));
}
