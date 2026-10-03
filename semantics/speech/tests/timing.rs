#![cfg(feature = "semantic-bindings")]
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_speech::{semantic::*, timing::*};

#[test]
fn exact_time_and_cycle_never_alias_renderer_frames_or_each_other() {
    let duration = SpeechExactDuration::new(8000, 61).unwrap();
    let cycle = SpeechFundamentalCycle::new(8000, 61).unwrap();
    assert_ne!(
        duration.clone().into_structured().unwrap().value_type(),
        cycle.clone().into_structured().unwrap().value_type()
    );
    for (rate, expected) in [(8000, 61), (16000, 122)] {
        let request = SpeechCycleAtRateRequest::new(cycle.clone(), rate).unwrap();
        let result = cycle_at_rate(&request).unwrap();
        assert_eq!(*result.whole_frames(), expected);
        assert_eq!(*result.remainder_numerator(), 0);
        assert_eq!(result.request(), &request);
    }
    assert!(SpeechExactDuration::new(0, 61).is_err());
    assert!(SpeechFundamentalCycle::new(8000, 0).is_err());
    assert!(SpeechCycleAtRateRequest::new(cycle, 0).is_err());
    assert!(SpeechExactDuration::new(1, 0).is_ok());
    assert!(SpeechRelativeIntensity::new(0, 1).is_err());
}

#[test]
fn projection_retains_fractional_frames_and_original_authored_ratio() {
    let duration = SpeechExactDuration::new(3, 1).unwrap();
    let request = SpeechDurationAtRateRequest::new(duration, 8000).unwrap();
    let projected = duration_at_rate(&request).unwrap();
    assert_eq!(*projected.whole_frames(), 2666);
    assert_eq!(*projected.remainder_numerator(), 2);
    assert_eq!(projected.request(), &request);
    assert!(SpeechDurationAtRate::new(3, request.clone(), 2666).is_err());
    assert!(SpeechDurationAtRate::new(2, request.clone(), 2667).is_err());
    let roundtrip =
        SpeechDurationAtRate::from_structured(projected.clone().into_structured().unwrap())
            .unwrap();
    assert_eq!(roundtrip, projected);
    let unreduced = SpeechExactDuration::new(1000, 120).unwrap();
    let request = SpeechDurationAtRateRequest::new(unreduced, 16000).unwrap();
    let projected = duration_at_rate(&request).unwrap();
    assert_eq!(*projected.whole_frames(), 1920);
    assert_eq!(*projected.request().duration().denominator(), 1000);
    assert_eq!(*projected.request().duration().numerator_seconds(), 120);
}

#[test]
fn checked_projection_overflow_refuses_without_mutating_the_basis() {
    let request =
        SpeechDurationAtRateRequest::new(SpeechExactDuration::new(1, u64::MAX).unwrap(), 192000)
            .unwrap();
    assert_eq!(duration_at_rate(&request), Err(TimingRefusal::Arithmetic));
    assert_eq!(*request.duration().numerator_seconds(), u64::MAX);
}
