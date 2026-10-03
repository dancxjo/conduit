#![cfg(feature = "semantic-bindings")]
use conduit_speech::{duration::*, semantic::*};

#[test]
fn cumulative_floor_preserves_fractional_frames_without_drift() {
    let third = SpeechExactDuration::new(3, 1).unwrap();
    for (rate, counts) in [(8000, [2666, 2667, 2667]), (16000, [5333, 5333, 5334])] {
        let spans = duration_spans(&[third.clone(), third.clone(), third.clone()], rate).unwrap();
        let mut start = 0;
        for (span, count) in spans.iter().zip(counts) {
            assert_eq!(span.duration(), &third);
            assert_eq!(*span.start_frame(), start);
            assert_eq!(*span.frame_count(), count);
            assert_eq!(*span.cumulative_end().request().sample_rate_hz(), rate);
            start += count;
            assert_eq!(*span.cumulative_end().whole_frames(), start);
        }
        assert_eq!(start, rate);
        assert_eq!(*spans[2].cumulative_end().remainder_numerator(), 0);
    }
}

#[test]
fn mixed_bases_and_zero_time_retain_the_original_durations() {
    let durations = [
        SpeechExactDuration::new(3, 1).unwrap(),
        SpeechExactDuration::new(8000, 1).unwrap(),
        SpeechExactDuration::new(1000, 120).unwrap(),
        SpeechExactDuration::new(24000, 0).unwrap(),
    ];
    let spans = duration_spans(&durations, 8000).unwrap();
    assert_eq!(
        spans
            .iter()
            .map(|span| *span.frame_count())
            .collect::<Vec<_>>(),
        [2666, 1, 960, 0]
    );
    for (span, duration) in spans.iter().zip(&durations) {
        assert_eq!(span.duration(), duration);
    }
    assert_eq!(*spans[3].cumulative_end().whole_frames(), 3627);
    assert_eq!(
        *spans[3].cumulative_end().request().duration().denominator(),
        24000
    );
    assert_eq!(*spans[3].cumulative_end().remainder_numerator(), 16000);
}

#[test]
fn preparation_refuses_bounds_and_overflow_without_changing_inputs() {
    let tiny = SpeechExactDuration::new(8000, 1).unwrap();
    assert_eq!(
        duration_spans(&vec![tiny; 257], 8000),
        Err(DurationRefusal::EventBound)
    );
    assert_eq!(duration_spans(&[], 0), Err(DurationRefusal::Rate));
    assert_eq!(duration_spans(&[], 192001), Err(DurationRefusal::Rate));
    assert!(duration_spans(&[], 8000).unwrap().is_empty());
    let durations = [
        SpeechExactDuration::new(u64::MAX, 1).unwrap(),
        SpeechExactDuration::new(2, 1).unwrap(),
    ];
    let before = durations.clone();
    assert_eq!(
        duration_spans(&durations, 8000),
        Err(DurationRefusal::Arithmetic { event: 1 })
    );
    assert_eq!(durations, before);
    let huge = SpeechExactDuration::new(1, u64::MAX).unwrap();
    assert_eq!(
        duration_spans(&[huge], 8000),
        Err(DurationRefusal::Arithmetic { event: 0 })
    );
}

#[test]
fn native_span_law_refuses_an_inconsistent_frame_count() {
    let duration = SpeechExactDuration::new(8000, 960).unwrap();
    let span = duration_spans(core::slice::from_ref(&duration), 8000)
        .unwrap()
        .pop()
        .unwrap();
    assert!(SpeechEventFrameSpan::new(span.cumulative_end().clone(), duration, 959, 0).is_err());
}

fn event() -> conduit_speech::VoiceEvent {
    use conduit_speech::*;
    VoiceEvent::segment(RealizationInput {
        phoneme: EnglishPhoneme::iy,
        position: EnglishPosition::medial,
        stress: EnglishStress::primary,
    })
}
fn render(mut renderer: conduit_speech::Renderer<'_>, block: usize) -> Vec<i16> {
    let mut output = Vec::new();
    let mut storage = [0; 128];
    while !renderer.is_complete() {
        let count = renderer.render(&mut storage[..block]).unwrap();
        output.extend_from_slice(&storage[..count]);
    }
    assert_eq!(output.len() as u64, renderer.total_frames());
    output
}

#[test]
fn source_durations_drive_actual_pcm_length_and_pressure_safe_blocks() {
    let events = [event(); 3];
    let durations = vec![SpeechExactDuration::new(3, 1).unwrap(); 3];
    let mut storage = [-1; 4];
    let prepared = prepare_duration_render(&events, &durations, &mut storage).unwrap();
    assert_eq!(prepared.renderer().total_frames(), 8000);
    assert_eq!(
        prepared
            .spans()
            .iter()
            .map(|span| *span.frame_count())
            .collect::<Vec<_>>(),
        [2666, 2667, 2667]
    );
    let expected = render(prepared.renderer(), 128);
    for block in [1, 63, 127] {
        assert_eq!(render(prepared.renderer(), block), expected);
    }
    let original = prepared.renderer();
    let mut candidate = original;
    let mut rejected = [0; 128];
    candidate.render(&mut rejected).unwrap();
    assert_eq!(original.rendered_frames(), 0);
    let mut committed = original;
    let mut accepted = [0; 128];
    committed.render(&mut accepted).unwrap();
    assert_eq!(rejected, accepted);
}

#[test]
fn profile_duration_retains_exact_audio_and_zero_boundaries_add_no_samples() {
    use conduit_speech::{Renderer, VoiceBoundary, VoiceEvent};
    let events = [event()];
    let duration = [SpeechExactDuration::new(8000, 960).unwrap()];
    let mut storage = [0];
    let prepared = prepare_duration_render(&events, &duration, &mut storage).unwrap();
    assert_eq!(
        render(prepared.renderer(), 128),
        render(Renderer::prepare(&events).unwrap(), 128)
    );
    let boundaries = [VoiceEvent::boundary(VoiceBoundary::word); 256];
    let zero = SpeechExactDuration::new(1, 0).unwrap();
    let durations = vec![zero; 256];
    let mut storage = [99; 256];
    let prepared = prepare_duration_render(&boundaries, &durations, &mut storage).unwrap();
    assert!(prepared.renderer().is_complete());
    assert!(render(prepared.renderer(), 128).is_empty());
    let events = [
        boundaries[0],
        event(),
        boundaries[0],
        event(),
        boundaries[0],
    ];
    let mut renderer = Renderer::prepare_timed(&events, &[0, 960, 0, 640, 0]).unwrap();
    assert_eq!(renderer.total_frames(), 1600);
    assert_eq!(render(renderer, 1), render(renderer, 128));
    assert_eq!(renderer.render(&mut []).unwrap(), 0);
    assert_eq!(renderer.rendered_frames(), 0);
}

#[test]
fn duration_refusal_is_transactional_and_distinct_from_shape_and_storage() {
    use conduit_speech::{RenderRefusal, VoiceBoundary, VoiceEvent};
    let events = [event()];
    let mut storage = [77; 2];
    let tiny = [SpeechExactDuration::new(8000, 1).unwrap()];
    assert!(matches!(
        prepare_duration_render(&events, &tiny, &mut storage),
        Err(DurationRenderRefusal::Renderer(RenderRefusal::TimingDomain))
    ));
    assert_eq!(storage, [77; 2]);
    assert!(matches!(
        prepare_duration_render(&events, &[], &mut storage),
        Err(DurationRenderRefusal::TimingCount)
    ));
    assert!(matches!(
        prepare_duration_render(&events, &tiny, &mut []),
        Err(DurationRenderRefusal::OutputSpace)
    ));
    let too_long = [SpeechExactDuration::new(1, 31).unwrap()];
    assert!(matches!(
        prepare_duration_render(&events, &too_long, &mut storage),
        Err(DurationRenderRefusal::Renderer(RenderRefusal::TimingDomain))
    ));
    assert_eq!(storage, [77; 2]);
    let boundaries = [VoiceEvent::boundary(VoiceBoundary::word); 2];
    let long = vec![SpeechExactDuration::new(1, 20).unwrap(); 2];
    assert!(matches!(
        prepare_duration_render(&boundaries, &long, &mut storage),
        Err(DurationRenderRefusal::Renderer(
            RenderRefusal::DurationBound
        ))
    ));
    assert_eq!(storage, [77; 2]);
}

#[test]
fn timed_intake_preserves_an_explicit_phone_and_its_neighbor_model() {
    use conduit_speech::{
        realize, EnglishPhone, EnglishPhoneme, EnglishPosition, EnglishStress, RealizationInput,
        Renderer, VoiceEvent,
    };
    let input = RealizationInput {
        phoneme: EnglishPhoneme::t,
        position: EnglishPosition::initial,
        stress: EnglishStress::primary,
    };
    let mut selected = realize(input).unwrap();
    selected.phone = EnglishPhone::t;
    let events = [VoiceEvent::selected(selected), event()];
    let durations = [
        SpeechExactDuration::new(8000, 320).unwrap(),
        SpeechExactDuration::new(8000, 640).unwrap(),
    ];
    let mut storage = [0; 2];
    let prepared = prepare_duration_render(&events, &durations, &mut storage).unwrap();
    let actual = render(prepared.renderer(), 63);
    let plain = [
        VoiceEvent::segment(RealizationInput {
            position: EnglishPosition::medial,
            ..input
        }),
        events[1],
    ];
    assert_eq!(
        actual,
        render(Renderer::prepare_timed(&plain, &[320, 640]).unwrap(), 128)
    );
    let inferred = [VoiceEvent::segment(input), events[1]];
    assert_ne!(
        actual,
        render(
            Renderer::prepare_timed(&inferred, &[320, 640]).unwrap(),
            128
        )
    );
    assert_eq!(events[0], VoiceEvent::selected(selected));
    assert_eq!(prepared.spans()[0].duration(), &durations[0]);
}
