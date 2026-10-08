//! Allocating, Source-authored bounded coefficient projection. No renderer or
//! clock authority is inferred; original acoustic quantities remain untouched.
use crate::{
    common_acoustic_quantities::{
        execute, SpeechCommonAcousticExecution, SpeechCommonAcousticRefusal,
    },
    resonator_programs::*,
    semantic::*,
};
use alloc::vec::Vec;
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};
#[derive(Debug)]
pub enum SpeechResonatorProjectionRefusal {
    Admission(NativeBindingRefusal),
    Source(SpeechCommonAcousticRefusal),
    NumericNarrowing,
    Quantization(alloc::boxed::Box<SpeechResonatorQuantizationRefusal>),
    DspArithmetic,
}
impl From<NativeBindingRefusal> for SpeechResonatorProjectionRefusal {
    fn from(v: NativeBindingRefusal) -> Self {
        Self::Admission(v)
    }
}
impl From<SpeechCommonAcousticRefusal> for SpeechResonatorProjectionRefusal {
    fn from(v: SpeechCommonAcousticRefusal) -> Self {
        Self::Source(v)
    }
}
pub struct SpeechResonatorQuantizationRefusal {
    original: SpeechResonatorProjectionRequest,
    original_frame: Vec<u8>,
    eligible_frame: Vec<u8>,
    executions: Vec<SpeechCommonAcousticExecution>,
    admitted_states: Vec<Vec<u8>>,
    raw_result: SpeechResonatorRawQ14,
    raw_frame: Vec<u8>,
    reason: NativeBindingRefusal,
}
impl core::fmt::Debug for SpeechResonatorQuantizationRefusal {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SpeechResonatorQuantizationRefusal")
            .field("raw_result", &self.raw_result)
            .field("reason", &self.reason)
            .finish()
    }
}
impl SpeechResonatorQuantizationRefusal {
    pub fn original(&self) -> &SpeechResonatorProjectionRequest {
        &self.original
    }
    pub fn original_canonical(&self) -> &[u8] {
        &self.original_frame
    }
    pub fn admitted_eligibility_canonical(&self) -> &[u8] {
        &self.eligible_frame
    }
    pub fn executions(&self) -> &[SpeechCommonAcousticExecution] {
        &self.executions
    }
    pub fn admitted_polynomial_state_frames(&self) -> &[Vec<u8>] {
        &self.admitted_states
    }
    pub fn raw_result(&self) -> &SpeechResonatorRawQ14 {
        &self.raw_result
    }
    pub fn raw_result_canonical(&self) -> &[u8] {
        &self.raw_frame
    }
    pub fn reason(&self) -> &NativeBindingRefusal {
        &self.reason
    }
}
pub struct PreparedSpeechResonatorQ14 {
    original: SpeechResonatorProjectionRequest,
    original_frame: Vec<u8>,
    eligible_frame: Vec<u8>,
    executions: Vec<SpeechCommonAcousticExecution>,
    admitted_states: Vec<Vec<u8>>,
    raw_result: SpeechResonatorRawQ14,
    raw_frame: Vec<u8>,
    result: SpeechStableResonatorQ14,
    admitted_frame: Vec<u8>,
}
impl PreparedSpeechResonatorQ14 {
    pub fn original(&self) -> &SpeechResonatorProjectionRequest {
        &self.original
    }
    pub fn original_canonical(&self) -> &[u8] {
        &self.original_frame
    }
    pub fn admitted_eligibility_canonical(&self) -> &[u8] {
        &self.eligible_frame
    }
    pub fn executions(&self) -> &[SpeechCommonAcousticExecution] {
        &self.executions
    }
    pub fn admitted_polynomial_state_frames(&self) -> &[Vec<u8>] {
        &self.admitted_states
    }
    pub fn raw_result(&self) -> &SpeechResonatorRawQ14 {
        &self.raw_result
    }
    pub fn raw_result_canonical(&self) -> &[u8] {
        &self.raw_frame
    }
    pub fn result(&self) -> &SpeechStableResonatorQ14 {
        &self.result
    }
    pub fn admitted_canonical(&self) -> &[u8] {
        &self.admitted_frame
    }
    /// One transition of the existing actual Source resonator. Checked integer
    /// arithmetic refuses overflow; caller owns finite state and drive bounds.
    pub fn filter_transition(
        &self,
        drive: i32,
        y1: i32,
        y2: i32,
    ) -> Result<i32, SpeechResonatorProjectionRefusal> {
        let (b, c) = self.dsp_coefficients()?;
        crate::generated::speech_resonator(crate::generated::ResonatorInput {
            drive,
            b,
            c,
            y1,
            y2,
        })
        .ok_or(SpeechResonatorProjectionRefusal::DspArithmetic)
    }
    /// Exact narrow copies for the existing I32 DSP ports, without arithmetic.
    pub fn dsp_coefficients(&self) -> Result<(i32, i32), SpeechResonatorProjectionRefusal> {
        Ok((
            i32::try_from(*self.result.b())
                .map_err(|_| SpeechResonatorProjectionRefusal::NumericNarrowing)?,
            i32::try_from(*self.result.c())
                .map_err(|_| SpeechResonatorProjectionRefusal::NumericNarrowing)?,
        ))
    }
}
fn state(
    raw: SpeechResonatorRawPolynomialState,
    frames: &mut Vec<Vec<u8>>,
) -> Result<SpeechResonatorPolynomialState, SpeechResonatorProjectionRefusal> {
    let s = SpeechResonatorPolynomialState::new(
        *raw.argument_q20(),
        *raw.index(),
        *raw.mode(),
        *raw.sum_q20(),
        *raw.term_q20(),
    )?;
    let frame = s.encode()?;
    let s = SpeechResonatorPolynomialState::decode(&frame)?;
    frames.push(frame);
    Ok(s)
}
pub fn prepare_speech_resonator_q14(
    canonical: &[u8],
) -> Result<PreparedSpeechResonatorQ14, SpeechResonatorProjectionRefusal> {
    let original = SpeechResonatorProjectionRequest::decode(canonical)?;
    let eligible = SpeechResonatorIntegerHzEligibility::new(original.clone())?;
    let eligible_frame = eligible.encode()?;
    let narrow =
        |v: u64| i64::try_from(v).map_err(|_| SpeechResonatorProjectionRefusal::NumericNarrowing);
    let input = SpeechResonatorNumericInput::new(
        narrow(*original.resonator().bandwidth().numerator_hz())?,
        narrow(*original.resonator().center().numerator_hz())?,
        narrow(*original.sample_rate_hz())?,
    )?;
    let mut executions = Vec::new();
    let mut admitted_states = Vec::new();
    let angles = SpeechResonatorAngles::decode(&execute(ANGLES, input, &mut executions)?)?;
    let mut results = Vec::new();
    for mode in [
        SpeechResonatorPolynomialMode::Cosine,
        SpeechResonatorPolynomialMode::Decay,
    ] {
        let seed = SpeechResonatorPolynomialSeed::new(angles.clone(), mode)?;
        let mut current = state(
            SpeechResonatorRawPolynomialState::decode(&execute(SEED, seed, &mut executions)?)?,
            &mut admitted_states,
        )?;
        // Finite traversal only; Source owns index, recurrence, arithmetic and seed.
        for _ in 0..8 {
            let step = SpeechResonatorPolynomialStep::new(current)?;
            current = state(
                SpeechResonatorRawPolynomialState::decode(&execute(STEP, step, &mut executions)?)?,
                &mut admitted_states,
            )?;
        }
        results.push(current);
    }
    let cosine = results.remove(0);
    let decay = results.remove(0);
    let combined = SpeechResonatorPolynomialResult::new(cosine, decay)?;
    let raw_q20 = SpeechResonatorRawQ20::decode(&execute(COMBINE, combined, &mut executions)?)?;
    let round = SpeechResonatorRoundQ20::new(raw_q20)?;
    let raw_frame = execute(ROUND, round, &mut executions)?;
    let raw_result = SpeechResonatorRawQ14::decode(&raw_frame)?;
    let result = match SpeechStableResonatorQ14::new(*raw_result.b(), *raw_result.c()) {
        Ok(value) => value,
        Err(reason) => {
            return Err(SpeechResonatorProjectionRefusal::Quantization(
                alloc::boxed::Box::new(SpeechResonatorQuantizationRefusal {
                    original,
                    original_frame: canonical.into(),
                    eligible_frame,
                    executions,
                    admitted_states,
                    raw_result,
                    raw_frame,
                    reason,
                }),
            ))
        }
    };
    let admitted_frame = result.encode()?;
    let result = SpeechStableResonatorQ14::decode(&admitted_frame)?;
    Ok(PreparedSpeechResonatorQ14 {
        original,
        original_frame: canonical.to_vec(),
        eligible_frame,
        executions,
        admitted_states,
        raw_result,
        raw_frame,
        result,
        admitted_frame,
    })
}
