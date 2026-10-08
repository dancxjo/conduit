//! Additive common-IR quantity preparation; no utterance or renderer ownership.
use crate::common_acoustic_programs::*;
use crate::semantic::*;
use alloc::vec::Vec;
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};
use conduit_plot::{PortableExpressionEvaluationRefusal, PortableExpressionProgram};
#[derive(Debug)]
pub enum SpeechCommonAcousticRefusal {
    Admission(NativeBindingRefusal),
    InvalidProgram,
    Evaluation(PortableExpressionEvaluationRefusal),
    InvalidOutput,
    InvalidSpan,
    UnorderedOrOverlapping,
    ForeignAnchor,
    MissingCoverage,
    UnsupportedLinearProfile,
    UnsupportedMeasurementProfile,
}
impl From<NativeBindingRefusal> for SpeechCommonAcousticRefusal {
    fn from(e: NativeBindingRefusal) -> Self {
        Self::Admission(e)
    }
}
#[derive(Clone)]
pub struct SpeechCommonAcousticExecution {
    program: &'static str,
    input: Vec<u8>,
    output: Vec<u8>,
}
impl SpeechCommonAcousticExecution {
    pub fn source_program_hex(&self) -> &'static str {
        self.program
    }
    pub fn input_canonical(&self) -> &[u8] {
        &self.input
    }
    pub fn output_canonical(&self) -> &[u8] {
        &self.output
    }
}
pub(crate) fn execute<T: NativeRustBinding>(
    program: &'static str,
    input: T,
    evidence: &mut Vec<SpeechCommonAcousticExecution>,
) -> Result<Vec<u8>, SpeechCommonAcousticRefusal> {
    let input = input.encode()?;
    execute_admitted(program, input, evidence)
}
// Used only after exact typed Native admission or after selecting a field of an
// admitted curve. This is not a refinement validator for untrusted raw bytes.
pub(crate) fn execute_admitted(
    program: &'static str,
    input: Vec<u8>,
    evidence: &mut Vec<SpeechCommonAcousticExecution>,
) -> Result<Vec<u8>, SpeechCommonAcousticRefusal> {
    let parsed = PortableExpressionProgram::from_canonical_hex(program)
        .map_err(|_| SpeechCommonAcousticRefusal::InvalidProgram)?;
    let output = parsed
        .evaluate(&input)
        .map_err(SpeechCommonAcousticRefusal::Evaluation)?;
    evidence.push(SpeechCommonAcousticExecution {
        program,
        input,
        output: output.clone(),
    });
    Ok(output)
}
pub(crate) fn boolean<T: NativeRustBinding>(
    program: &'static str,
    input: T,
    evidence: &mut Vec<SpeechCommonAcousticExecution>,
) -> Result<bool, SpeechCommonAcousticRefusal> {
    match execute(program, input, evidence)?.as_slice() {
        [0] => Ok(false),
        [1] => Ok(true),
        _ => Err(SpeechCommonAcousticRefusal::InvalidOutput),
    }
}
pub struct SpeechCommonQuantityReceipt<I, O> {
    original: I,
    original_canonical: Vec<u8>,
    executions: Vec<SpeechCommonAcousticExecution>,
    result: O,
    admitted: Vec<u8>,
}
impl<I, O> SpeechCommonQuantityReceipt<I, O> {
    pub fn original(&self) -> &I {
        &self.original
    }
    pub fn original_canonical(&self) -> &[u8] {
        &self.original_canonical
    }
    pub fn executions(&self) -> &[SpeechCommonAcousticExecution] {
        &self.executions
    }
    pub fn result(&self) -> &O {
        &self.result
    }
    pub fn admitted_canonical(&self) -> &[u8] {
        &self.admitted
    }
}
fn project<I: NativeRustBinding + Clone, O: NativeRustBinding + Clone>(
    program: &'static str,
    canonical: &[u8],
) -> Result<SpeechCommonQuantityReceipt<I, O>, SpeechCommonAcousticRefusal> {
    let original = I::decode(canonical)?;
    let original_canonical = original.clone().encode()?;
    let mut executions = Vec::new();
    let result = O::decode(&execute(program, original.clone(), &mut executions)?)?;
    let admitted = result.clone().encode()?;
    Ok(SpeechCommonQuantityReceipt {
        original,
        original_canonical,
        executions,
        result,
        admitted,
    })
}
pub fn speech_duration_to_audio(
    canonical: &[u8],
) -> Result<
    SpeechCommonQuantityReceipt<SpeechExactDuration, conduit_audio::AudioTimeFraction>,
    SpeechCommonAcousticRefusal,
> {
    project(DURATION, canonical)
}
pub fn speech_cycle_to_audio(
    canonical: &[u8],
) -> Result<
    SpeechCommonQuantityReceipt<SpeechFundamentalCycle, conduit_audio::AudioCycleDuration>,
    SpeechCommonAcousticRefusal,
> {
    project(CYCLE, canonical)
}
pub fn speech_intensity_to_audio(
    canonical: &[u8],
) -> Result<
    SpeechCommonQuantityReceipt<SpeechRelativeIntensity, conduit_audio::AudioRelativeAmplitude>,
    SpeechCommonAcousticRefusal,
> {
    project(INTENSITY, canonical)
}
pub fn speech_rate_to_syllable_period(
    canonical: &[u8],
) -> Result<
    SpeechCommonQuantityReceipt<SpeechSyllabicRate, SpeechSyllablePeriod>,
    SpeechCommonAcousticRefusal,
> {
    let original = SpeechSyllabicRate::decode(canonical)?;
    let original_canonical = original.clone().encode()?;
    let input = SpeechRateReciprocalInput::new(
        *original.denominator_seconds(),
        *original.numerator_syllables(),
    )?;
    let mut executions = Vec::new();
    let raw = SpeechSyllablePeriodRaw::decode(&execute(RATE_PERIOD, input, &mut executions)?)?;
    let result = SpeechSyllablePeriod::new(
        *raw.denominator_syllables(),
        *raw.numerator_seconds(),
        original.scope().clone(),
    )?;
    let admitted = result.clone().encode()?;
    Ok(SpeechCommonQuantityReceipt {
        original,
        original_canonical,
        executions,
        result,
        admitted,
    })
}
pub fn speech_tilt_single_octave_delta(
    canonical: &[u8],
) -> Result<
    SpeechCommonQuantityReceipt<SpeechTiltOctaveRequest, conduit_audio::AudioDecibelFraction>,
    SpeechCommonAcousticRefusal,
> {
    let original = SpeechTiltOctaveRequest::decode(canonical)?;
    let original_canonical = original.clone().encode()?;
    let input = SpeechTiltSingleOctaveEligible::new(original.clone())?;
    let mut executions = Vec::new();
    let raw = SpeechTiltDeltaRaw::decode(&execute(TILT, input, &mut executions)?)?;
    let result = conduit_audio::AudioDecibelFraction::new(*raw.denominator(), *raw.numerator_db())?;
    let admitted = result.clone().encode()?;
    Ok(SpeechCommonQuantityReceipt {
        original,
        original_canonical,
        executions,
        result,
        admitted,
    })
}
