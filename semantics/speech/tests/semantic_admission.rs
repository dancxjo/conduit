#![cfg(feature = "semantic-bindings")]
use conduit_core::IeeeF64;
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_speech::{admission::*, semantic::*};

#[test]
fn seconds_ordering_is_checked_after_native_construction_or_decode() {
    for (start, end) in [(-1.25, -0.5), (0.125, 0.125), (-0.0, 0.0)] {
        let span =
            SpeechTimeSpan::new(IeeeF64::from_value(end), IeeeF64::from_value(start)).unwrap();
        assert_eq!(validate_time_span(&span), Ok(()));
        assert_eq!(span.start_s().bits(), start.to_bits());
        let span = SpeechTimeSpan::from_structured(span.into_structured().unwrap()).unwrap();
        assert_eq!(validate_time_span(&span), Ok(()));
    }
    let reversed = SpeechTimeSpan::new(IeeeF64::from_value(0.1), IeeeF64::from_value(0.2)).unwrap();
    assert_eq!(
        validate_time_span(&reversed),
        Err(LocalSemanticRefusal::ReversedSeconds)
    );
    let reversed = SpeechTimeSpan::from_structured(reversed.into_structured().unwrap()).unwrap();
    assert_eq!(
        validate_time_span(&reversed),
        Err(LocalSemanticRefusal::ReversedSeconds)
    );
    let reversed =
        SpeechSegmentSpan::seconds(IeeeF64::from_value(0.1), IeeeF64::from_value(0.2)).unwrap();
    assert_eq!(
        validate_segment_span(&reversed),
        Err(LocalSemanticRefusal::ReversedSeconds)
    );
    for nonfinite in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(
            SpeechTimeSpan::new(IeeeF64::from_value(nonfinite), IeeeF64::from_value(0.0)).is_err()
        );
        assert!(SpeechSegmentSpan::seconds(
            IeeeF64::from_value(0.0),
            IeeeF64::from_value(nonfinite)
        )
        .is_err());
    }
}

#[test]
fn probability_checks_do_not_reinterpret_provider_or_log_scores() {
    let confidence =
        |scale, value| ListeningConfidence::new(None, scale, IeeeF64::from_value(value)).unwrap();
    for value in [-0.0, 0.0, 0.125, 1.0] {
        let score = confidence(ListeningConfidenceScale::Probability, value);
        assert_eq!(validate_listening_confidence(&score), Ok(()));
        assert_eq!(score.value().bits(), value.to_bits());
    }
    for value in [-0.001, 1.001, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let score = confidence(ListeningConfidenceScale::Probability, value);
        let score = ListeningConfidence::from_structured(score.into_structured().unwrap()).unwrap();
        assert_eq!(
            validate_listening_confidence(&score),
            Err(LocalSemanticRefusal::InvalidProbability)
        );
    }
    for value in [-123.456789012345, 7.125] {
        let score = confidence(ListeningConfidenceScale::LogProbability, value);
        assert_eq!(validate_listening_confidence(&score), Ok(()));
        assert_eq!(score.value().bits(), value.to_bits());
        let scale = ListeningConfidenceScale::provider_native(
            "vendor-score".into(),
            "provider".into(),
            None,
        )
        .unwrap();
        let score = confidence(scale, value);
        assert_eq!(validate_listening_confidence(&score), Ok(()));
        assert_eq!(score.value().bits(), value.to_bits());
    }
}
