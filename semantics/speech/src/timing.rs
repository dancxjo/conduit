//! Preparation adapters for the checked exact-time projection plots.
//! No rounding, renderer selection, commitment or acoustic policy lives here.
use crate::{generated, semantic};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimingRefusal {
    Arithmetic,
    OutputDomain,
}

/// Retains the original request and the exact fractional-frame remainder.
/// The caller chooses whether its realization permits that remainder.
pub fn duration_at_rate(
    request: &semantic::SpeechDurationAtRateRequest,
) -> Result<semantic::SpeechDurationAtRate, TimingRefusal> {
    let result = generated::speech_time_at_rate(generated::SpeechTimeProjectionInput {
        numerator_seconds: *request.duration().numerator_seconds(),
        denominator: *request.duration().denominator(),
        sample_rate_hz: *request.sample_rate_hz(),
    })
    .ok_or(TimingRefusal::Arithmetic)?;
    semantic::SpeechDurationAtRate::new(
        result.remainder_numerator,
        request.clone(),
        result.whole_frames,
    )
    .map_err(|_| TimingRefusal::OutputDomain)
}

/// A cycle in seconds is independently projected into the requested clock.
pub fn cycle_at_rate(
    request: &semantic::SpeechCycleAtRateRequest,
) -> Result<semantic::SpeechCycleAtRate, TimingRefusal> {
    let result = generated::speech_time_at_rate(generated::SpeechTimeProjectionInput {
        numerator_seconds: *request.cycle().numerator_seconds(),
        denominator: *request.cycle().denominator(),
        sample_rate_hz: *request.sample_rate_hz(),
    })
    .ok_or(TimingRefusal::Arithmetic)?;
    semantic::SpeechCycleAtRate::new(
        result.remainder_numerator,
        request.clone(),
        result.whole_frames,
    )
    .map_err(|_| TimingRefusal::OutputDomain)
}
