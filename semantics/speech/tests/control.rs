#![cfg(feature = "semantic-bindings")]
use conduit_speech::{control::*, semantic::*, *};

fn events() -> [VoiceEvent; 4] {
    [
        EnglishPhoneme::iy,
        EnglishPhoneme::h,
        EnglishPhoneme::z,
        EnglishPhoneme::aa,
    ]
    .map(|phoneme| {
        VoiceEvent::segment(RealizationInput {
            phoneme,
            stress: EnglishStress::primary,
            position: EnglishPosition::medial,
        })
    })
}
fn render(mut renderer: Renderer<'_>, block: usize) -> Vec<i16> {
    let mut output = Vec::new();
    let mut samples = [0; 128];
    while !renderer.is_complete() {
        let count = renderer.render(&mut samples[..block]).unwrap();
        output.extend_from_slice(&samples[..count]);
    }
    output
}
#[test]
fn projections_keep_requested_units_and_quantization_remainders() {
    let cycle = SpeechFundamentalCycle::new(120, 1).unwrap();
    let request = SpeechCycleAtRateRequest::new(cycle.clone(), 8000).unwrap();
    let projected = cycle_q8(&request).unwrap();
    assert_eq!(*projected.whole_q8(), 17066);
    assert_eq!(*projected.remainder_numerator(), 80);
    assert_eq!(projected.request(), &request);
    let amp = SpeechRelativeIntensity::new(1000, 1).unwrap();
    let projected = amplitude_q15(&amp).unwrap();
    assert_eq!(*projected.whole_q15(), 32);
    assert_eq!(*projected.remainder_numerator(), 768);
    assert_eq!(projected.request(), &amp);
    let projected = prepare_voice_control(Some(&cycle), &amp).unwrap();
    assert_eq!(projected.compact().period_q8, 17066);
    assert_eq!(projected.compact().amplitude_q15, 32);
    assert_eq!(projected.cycle().unwrap().request().cycle(), &cycle);
}
#[test]
fn output_gain_scales_voicing_and_noise_and_zero_is_still_completed_speech() {
    let events = events();
    let unity = prepare_voice_control(None, &SpeechRelativeIntensity::new(1, 1).unwrap()).unwrap();
    let controls = [unity.compact(); 4];
    let original = render(Renderer::prepare(&events).unwrap(), 128);
    assert_eq!(
        render(
            Renderer::prepare_controlled(&events, &controls).unwrap(),
            63
        ),
        original
    );
    let half = prepare_voice_control(None, &SpeechRelativeIntensity::new(2, 1).unwrap()).unwrap();
    let controls = [half.compact(); 4];
    let quieter = render(
        Renderer::prepare_controlled(&events, &controls).unwrap(),
        128,
    );
    assert_eq!(
        quieter,
        original.iter().map(|sample| sample / 2).collect::<Vec<_>>()
    );
    let zero = prepare_voice_control(None, &SpeechRelativeIntensity::new(1, 0).unwrap()).unwrap();
    let controls = [zero.compact(); 4];
    let silent = render(
        Renderer::prepare_controlled(&events, &controls).unwrap(),
        128,
    );
    assert_eq!(silent.len(), original.len());
    assert!(silent.iter().all(|sample| *sample == 0));
}
#[test]
fn fractional_pitch_is_block_invariant_and_pressure_can_discard_the_entire_candidate() {
    let events = events();
    let cycle = SpeechFundamentalCycle::new(120, 1).unwrap();
    let control =
        prepare_voice_control(Some(&cycle), &SpeechRelativeIntensity::new(1, 1).unwrap()).unwrap();
    let controls = [control.compact(); 4];
    let original = Renderer::prepare_controlled(&events, &controls).unwrap();
    let expected = render(original, 128);
    for block in [1, 63, 127] {
        assert_eq!(render(original, block), expected);
    }
    assert_ne!(expected, render(Renderer::prepare(&events).unwrap(), 128));
    let mut candidate = original;
    let mut refused = [0; 128];
    candidate.render(&mut refused).unwrap();
    assert_eq!(original.rendered_frames(), 0);
    let mut committed = original;
    let mut accepted = [0; 128];
    committed.render(&mut accepted).unwrap();
    assert_eq!(refused, accepted);
}
#[test]
fn unsupported_controls_and_arithmetic_have_distinct_refusals() {
    let events = events();
    let bad = SpeechEventVoiceControl {
        cycle_mode: SpeechCycleControlMode::resolved,
        period_q8: 0,
        amplitude_q15: 32768,
    };
    assert!(matches!(
        Renderer::prepare_controlled(&events, &[bad; 4]),
        Err(RenderRefusal::ControlDomain)
    ));
    assert!(matches!(
        Renderer::prepare_controlled(&events, &[]),
        Err(RenderRefusal::ControlCount)
    ));
    assert!(matches!(
        prepare_voice_control(None, &SpeechRelativeIntensity::new(1, 3).unwrap()),
        Err(ControlRefusal::Unsupported)
    ));
    assert_eq!(
        amplitude_q15(&SpeechRelativeIntensity::new(1, u64::MAX).unwrap()),
        Err(ControlRefusal::Arithmetic)
    );
    let request =
        SpeechCycleAtRateRequest::new(SpeechFundamentalCycle::new(1, u64::MAX).unwrap(), 8000)
            .unwrap();
    assert_eq!(cycle_q8(&request), Err(ControlRefusal::Arithmetic));
}
