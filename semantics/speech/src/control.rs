//! Admitted native quantities to explicit compact realization controls.
//! Plots own projection, precision, supported domains and waveform application.
use crate::{generated, semantic, SAMPLE_RATE_HZ};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlRefusal {
    Arithmetic,
    Domain,
    Unsupported,
}

pub fn cycle_q8(
    request: &semantic::SpeechCycleAtRateRequest,
) -> Result<semantic::SpeechCycleQ8AtRate, ControlRefusal> {
    let result = generated::speech_cycle_q8(generated::SpeechCycleQ8Input {
        numerator_seconds: *request.cycle().numerator_seconds(),
        denominator: *request.cycle().denominator(),
        sample_rate_hz: *request.sample_rate_hz(),
    })
    .ok_or(ControlRefusal::Arithmetic)?;
    semantic::SpeechCycleQ8AtRate::new(result.remainder_numerator, request.clone(), result.whole_q8)
        .map_err(|_| ControlRefusal::Domain)
}

pub fn amplitude_q15(
    request: &semantic::SpeechRelativeIntensity,
) -> Result<semantic::SpeechAmplitudeQ15, ControlRefusal> {
    let result = generated::speech_amplitude_q15(generated::SpeechAmplitudeQ15Input {
        numerator: *request.numerator(),
        denominator: *request.denominator(),
    })
    .ok_or(ControlRefusal::Arithmetic)?;
    semantic::SpeechAmplitudeQ15::new(
        result.remainder_numerator,
        request.clone(),
        result.whole_q15,
    )
    .map_err(|_| ControlRefusal::Domain)
}

/// Profile is an explicit realization choice, not an unknown/unspecified cycle.
/// Receipts preserve the source quantity and its declared quantization remainder.
pub struct PreparedVoiceControl {
    compact: generated::SpeechEventVoiceControl,
    cycle: Option<semantic::SpeechCycleQ8AtRate>,
    amplitude: semantic::SpeechAmplitudeQ15,
}
impl PreparedVoiceControl {
    pub fn compact(&self) -> generated::SpeechEventVoiceControl {
        self.compact
    }
    pub fn cycle(&self) -> Option<&semantic::SpeechCycleQ8AtRate> {
        self.cycle.as_ref()
    }
    pub fn amplitude(&self) -> &semantic::SpeechAmplitudeQ15 {
        &self.amplitude
    }
}

pub fn prepare_voice_control(
    cycle: Option<&semantic::SpeechFundamentalCycle>,
    amplitude: &semantic::SpeechRelativeIntensity,
) -> Result<PreparedVoiceControl, ControlRefusal> {
    let cycle = cycle
        .map(|cycle| {
            semantic::SpeechCycleAtRateRequest::new(cycle.clone(), u64::from(SAMPLE_RATE_HZ))
                .map_err(|_| ControlRefusal::Domain)
                .and_then(|request| cycle_q8(&request))
        })
        .transpose()?;
    let amplitude = amplitude_q15(amplitude)?;
    let compact = generated::SpeechEventVoiceControl {
        cycle_mode: if cycle.is_some() {
            generated::SpeechCycleControlMode::resolved
        } else {
            generated::SpeechCycleControlMode::profile
        },
        period_q8: cycle.as_ref().map_or(Ok(0), |value| {
            i32::try_from(*value.whole_q8()).map_err(|_| ControlRefusal::Unsupported)
        })?,
        amplitude_q15: i32::try_from(*amplitude.whole_q15())
            .map_err(|_| ControlRefusal::Unsupported)?,
    };
    if !generated::speech_voice_control_admitted(compact).ok_or(ControlRefusal::Arithmetic)? {
        return Err(ControlRefusal::Unsupported);
    }
    Ok(PreparedVoiceControl {
        compact,
        cycle,
        amplitude,
    })
}
