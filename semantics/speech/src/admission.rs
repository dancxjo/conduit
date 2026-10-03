//! Local preparation checks not expressible by the current native Type laws.
//!
//! These checks borrow exact values and never clamp, relabel, or resolve artifact
//! references. A successful check is not complete rich-to-compact admission.
use crate::semantic::{
    ListeningConfidence, ListeningConfidenceScale, SpeechSegmentSpan, SpeechTimeSpan,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalSemanticRefusal {
    ReversedSeconds,
    InvalidProbability,
}

fn ordered_seconds(start: f64, end: f64) -> Result<(), LocalSemanticRefusal> {
    // Native seconds leaves already enforce finiteness at construction/decode.
    if start > end {
        return Err(LocalSemanticRefusal::ReversedSeconds);
    }
    Ok(())
}

/// Equal endpoints and negative relative offsets are valid. No duration or
/// sample clock is inferred from a seconds span.
pub fn validate_time_span(span: &SpeechTimeSpan) -> Result<(), LocalSemanticRefusal> {
    ordered_seconds(span.start_s().value(), span.end_s().value())
}

/// Frame ordering is already a native law; seconds ordering needs this check.
pub fn validate_segment_span(span: &SpeechSegmentSpan) -> Result<(), LocalSemanticRefusal> {
    match span {
        SpeechSegmentSpan::Seconds(span) => {
            ordered_seconds(span.start_s().value(), span.end_s().value())
        }
        SpeechSegmentSpan::Frames(_) => Ok(()),
    }
}

/// Only explicitly probability-scaled evidence has a unit-interval law.
/// Provider-native and log scores retain their original domain and precision.
pub fn validate_listening_confidence(
    confidence: &ListeningConfidence,
) -> Result<(), LocalSemanticRefusal> {
    if matches!(confidence.scale(), ListeningConfidenceScale::Probability) {
        let value = confidence.value().value();
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(LocalSemanticRefusal::InvalidProbability);
        }
    }
    Ok(())
}
