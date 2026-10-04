#![cfg(feature = "semantic-bindings")]
use conduit_core::IeeeF32;
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::{
    control::prepare_voice_control, duration::duration_spans, intent_prosody::*, semantic::*, *,
};

fn duration() -> SpeechExactDuration {
    SpeechExactDuration::new(30, 1).unwrap()
}
fn cycle() -> SpeechFundamentalCycle {
    SpeechFundamentalCycle::new(120, 1).unwrap()
}
fn intensity() -> SpeechRelativeIntensity {
    SpeechRelativeIntensity::new(2, 1).unwrap()
}
fn intent() -> SpeechSegmentProsodyIntent {
    SpeechSegmentProsodyIntent::new(
        SpeechDurationSpecification::known(30, 1).unwrap(),
        SpeechCycleSpecification::known(120, 1).unwrap(),
        SpeechIntensitySpecification::known(2, 1).unwrap(),
    )
    .unwrap()
}
fn render(mut renderer: Renderer<'_>, block: usize) -> Vec<i16> {
    let mut samples = [0; 128];
    let mut pcm = Vec::new();
    while !renderer.is_complete() {
        let count = renderer.render(&mut samples[..block]).unwrap();
        pcm.extend_from_slice(&samples[..count]);
    }
    pcm
}
#[test]
fn rich_intent_renders_with_exact_existing_projection_and_block_invariance() {
    let intents = [intent(), intent(), intent()];
    let events = [EnglishPhone::iy, EnglishPhone::h, EnglishPhone::aa].map(|phone| {
        VoiceEvent::phone(SpeechPhoneInput {
            phone,
            stress: EnglishStress::primary,
        })
    });
    let prepared = prepare_segment_prosody(&intents).unwrap();
    assert!(core::ptr::eq(prepared.source(), &intents[..]));
    let spans = duration_spans(&[duration(), duration(), duration()], 8000).unwrap();
    assert_eq!(prepared.spans(), spans);
    let counts = spans
        .iter()
        .map(|s| *s.frame_count() as i32)
        .collect::<Vec<_>>();
    assert_eq!(counts, [266, 267, 267]);
    let control = prepare_voice_control(Some(&cycle()), &intensity()).unwrap();
    let controls = [control.compact(); 3];
    for receipt in prepared.controls() {
        assert_eq!(receipt.cycle().unwrap(), control.cycle().unwrap());
        assert_eq!(receipt.amplitude(), control.amplitude());
    }
    let expected = render(
        Renderer::prepare_timed(&events, &counts)
            .unwrap()
            .with_controls(&controls)
            .unwrap(),
        128,
    );
    assert_eq!(render(prepared.renderer(&events).unwrap(), 1), expected);
    assert_eq!(render(prepared.renderer(&events).unwrap(), 63), expected);
    assert_eq!(expected.len(), 800);
    assert!(expected.iter().any(|sample| *sample != 0));
}
#[test]
fn every_unresolved_quantity_retains_its_exact_state_and_event() {
    let confidence = SpeechConfidence::new(IeeeF32::from_value(0.5)).unwrap();
    let durations = [
        SpeechDurationSpecification::unknown(),
        SpeechDurationSpecification::unspecified(),
        SpeechDurationSpecification::not_applicable(),
        SpeechDurationSpecification::variable(
            BoundedSequence::try_from_iter([duration()]).unwrap(),
        )
        .unwrap(),
        SpeechDurationSpecification::gradient(confidence.clone(), duration()).unwrap(),
    ];
    let cycles = [
        SpeechCycleSpecification::unknown(),
        SpeechCycleSpecification::unspecified(),
        SpeechCycleSpecification::not_applicable(),
        SpeechCycleSpecification::variable(BoundedSequence::try_from_iter([cycle()]).unwrap())
            .unwrap(),
        SpeechCycleSpecification::gradient(confidence.clone(), cycle()).unwrap(),
    ];
    let intensities = [
        SpeechIntensitySpecification::unknown(),
        SpeechIntensitySpecification::unspecified(),
        SpeechIntensitySpecification::not_applicable(),
        SpeechIntensitySpecification::variable(
            BoundedSequence::try_from_iter([intensity()]).unwrap(),
        )
        .unwrap(),
        SpeechIntensitySpecification::gradient(confidence, intensity()).unwrap(),
    ];
    for specification in durations {
        let source = [
            intent(),
            SpeechSegmentProsodyIntent::new(
                specification.clone(),
                intent().fundamental_cycle().clone(),
                intent().relative_intensity().clone(),
            )
            .unwrap(),
        ];
        match prepare_segment_prosody(&source) {
            Err(IntentProsodyRefusal::Duration {
                event,
                specification: actual,
            }) => {
                assert_eq!(event, 1);
                assert_eq!(actual, specification);
            }
            _ => panic!("duration silently resolved"),
        }
    }
    for specification in cycles {
        let source = [
            intent(),
            SpeechSegmentProsodyIntent::new(
                intent().duration().clone(),
                specification.clone(),
                intent().relative_intensity().clone(),
            )
            .unwrap(),
        ];
        match prepare_segment_prosody(&source) {
            Err(IntentProsodyRefusal::Cycle {
                event,
                specification: actual,
            }) => {
                assert_eq!(event, 1);
                assert_eq!(actual, specification);
            }
            _ => panic!("cycle silently resolved"),
        }
    }
    for specification in intensities {
        let source = [
            intent(),
            SpeechSegmentProsodyIntent::new(
                intent().duration().clone(),
                intent().fundamental_cycle().clone(),
                specification.clone(),
            )
            .unwrap(),
        ];
        match prepare_segment_prosody(&source) {
            Err(IntentProsodyRefusal::Intensity {
                event,
                specification: actual,
            }) => {
                assert_eq!(event, 1);
                assert_eq!(actual, specification);
            }
            _ => panic!("intensity silently resolved"),
        }
    }
}
#[test]
fn finite_preparation_and_renderer_admission_remain_distinct() {
    assert!(matches!(
        prepare_segment_prosody(&vec![intent(); 257]),
        Err(IntentProsodyRefusal::EventBound)
    ));
    let empty = prepare_segment_prosody(&[]).unwrap();
    assert!(empty.renderer(&[]).unwrap().is_complete());
    let source = [intent()];
    let prepared = prepare_segment_prosody(&source).unwrap();
    assert!(matches!(
        prepared.renderer(&[]),
        Err(IntentProsodyRenderRefusal::Renderer(
            RenderRefusal::TimingCount
        ))
    ));
    assert!(matches!(
        prepared.renderer(&[VoiceEvent::boundary(VoiceBoundary::word)]),
        Err(IntentProsodyRenderRefusal::Boundary { event: 0 })
    ));
    let mut excessive = intent();
    excessive = SpeechSegmentProsodyIntent::new(
        excessive.duration().clone(),
        excessive.fundamental_cycle().clone(),
        SpeechIntensitySpecification::known(1, 3).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        prepare_segment_prosody(&[excessive]),
        Err(IntentProsodyRefusal::Control {
            event: 0,
            reason: conduit_speech::control::ControlRefusal::Unsupported
        })
    ));
}
