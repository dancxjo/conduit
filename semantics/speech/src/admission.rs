//! Local preparation checks not expressible by the current native Type laws.
//!
//! These checks borrow exact values and never clamp, relabel, or resolve artifact
//! references. A successful check is not complete rich-to-compact admission.
use crate::semantic::{
    AsrTranscriptCandidate, ListeningConfidence, ListeningConfidenceScale, SpeechFeatureBundle,
    SpeechSegmentSpan, SpeechTimeSpan,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalSemanticRefusal {
    ReversedSeconds,
    InvalidProbability,
    DuplicateFeatureIdentity,
    StablePrefixOutOfRange,
    StablePrefixNotUtf8Boundary,
    InconsistentTranscriptSplit,
    InconsistentStableWords,
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

/// Preserve the pinned FeatureBundle map's unique-key meaning without sorting,
/// merging, or discarding specifications. At most 120 comparisons for 16 items.
/// Call after construction or decode; native sequence bounds alone are weaker.
pub fn validate_feature_bundle(bundle: &SpeechFeatureBundle) -> Result<(), LocalSemanticRefusal> {
    let features = bundle.get().as_slice();
    for (index, feature) in features.iter().enumerate() {
        if features[..index]
            .iter()
            .any(|previous| previous.identity() == feature.identity())
        {
            return Err(LocalSemanticRefusal::DuplicateFeatureIdentity);
        }
    }
    Ok(())
}

/// Validate a borrowed pinned Speaking stability snapshot, not chronological
/// stability or commitment. UTF-8 bytes never become scalar revision indices.
/// Unlike upstream from_parts, malformed input refuses rather than clamps.
/// Word-prefix/count semantics follow transcript.rs at b5d7535: whitespace-
/// delimited prefix, excluding terminal text without a word delimiter.
/// This check does not reinterpret the optional provider confidence.
pub fn validate_transcript_candidate(
    candidate: &AsrTranscriptCandidate,
) -> Result<(), LocalSemanticRefusal> {
    let text = candidate.text().as_str();
    let split = usize::try_from(*candidate.stable_prefix_utf8_bytes())
        .map_err(|_| LocalSemanticRefusal::StablePrefixOutOfRange)?;
    if split > text.len() {
        return Err(LocalSemanticRefusal::StablePrefixOutOfRange);
    }
    if !text.is_char_boundary(split) {
        return Err(LocalSemanticRefusal::StablePrefixNotUtf8Boundary);
    }
    let (stable, unstable) = text.split_at(split);
    if candidate.stable_text() != stable || candidate.unstable_text() != unstable {
        return Err(LocalSemanticRefusal::InconsistentTranscriptSplit);
    }
    let word_end = if stable.chars().next_back().is_some_and(char::is_whitespace) {
        stable.trim_end().len()
    } else {
        stable
            .char_indices()
            .rev()
            .find_map(|(index, ch)| ch.is_whitespace().then_some(index + ch.len_utf8()))
            .unwrap_or_default()
    };
    let words = stable[..word_end].trim_end();
    let prefix = (!words.is_empty()).then_some(words);
    if candidate.stable_word_prefix().as_deref() != prefix
        || usize::try_from(*candidate.stable_word_count()).ok()
            != Some(words.split_whitespace().count())
    {
        return Err(LocalSemanticRefusal::InconsistentStableWords);
    }
    Ok(())
}
