//! Source-selected step curves preserve uncertainty; independent controls overlap.
use crate::common_acoustic_programs::*;
use crate::common_acoustic_quantities::*;
use crate::semantic::*;
use alloc::vec::Vec;
use conduit_audio::{
    AudioTimeFraction, AudioTrajectoryAnchor, AudioTrajectoryInterpolation, AudioTrajectoryQuery,
};
use conduit_core::{StructuredInfoValue, StructuredInfoValueShape};
use conduit_plot::rust_binding::{record_field_value, NativeRustBinding};
use SpeechCommonAcousticRefusal as Refusal;

pub struct PreparedSpeechTypedStepCurve<C> {
    original: C,
    original_canonical: Vec<u8>,
    anchor: AudioTrajectoryAnchor,
    segments: Vec<StructuredInfoValue>,
    preparation: Vec<SpeechCommonAcousticExecution>,
    value_admit: fn(StructuredInfoValue) -> Result<StructuredInfoValue, Refusal>,
}
pub struct SpeechTypedStepCurveReceipt<C> {
    original: C,
    original_canonical: Vec<u8>,
    query: AudioTrajectoryQuery,
    query_canonical: Vec<u8>,
    selected_segment: Vec<u8>,
    executions: Vec<SpeechCommonAcousticExecution>,
    result: StructuredInfoValue,
    result_canonical: Vec<u8>,
}
impl<C> SpeechTypedStepCurveReceipt<C> {
    pub fn original(&self) -> &C {
        &self.original
    }
    pub fn original_canonical(&self) -> &[u8] {
        &self.original_canonical
    }
    pub fn query(&self) -> &AudioTrajectoryQuery {
        &self.query
    }
    pub fn query_canonical(&self) -> &[u8] {
        &self.query_canonical
    }
    pub fn selected_segment_canonical(&self) -> &[u8] {
        &self.selected_segment
    }
    pub fn executions(&self) -> &[SpeechCommonAcousticExecution] {
        &self.executions
    }
    pub fn result(&self) -> &StructuredInfoValue {
        &self.result
    }
    pub fn result_canonical(&self) -> &[u8] {
        &self.result_canonical
    }
}
fn time(segment: &StructuredInfoValue, name: &str) -> Result<AudioTimeFraction, Refusal> {
    Ok(AudioTimeFraction::from_structured(record_field_value(
        segment, name,
    )?)?)
}
fn before(
    left: AudioTimeFraction,
    right: AudioTimeFraction,
    evidence: &mut Vec<SpeechCommonAcousticExecution>,
) -> Result<bool, Refusal> {
    boolean(
        BEFORE,
        SpeechAcousticTimeComparison::new(left, right)?,
        evidence,
    )
}
impl<C: NativeRustBinding + Clone> PreparedSpeechTypedStepCurve<C> {
    // Private construction prevents unrelated Native Types becoming curve proof.
    fn prepare(
        canonical: &[u8],
        value_admit: fn(StructuredInfoValue) -> Result<StructuredInfoValue, Refusal>,
    ) -> Result<Self, Refusal> {
        let original = C::decode(canonical)?;
        let original_canonical = original.clone().encode()?;
        let structured = original.clone().into_structured()?;
        let anchor =
            AudioTrajectoryAnchor::from_structured(record_field_value(&structured, "anchor")?)?;
        let collection = record_field_value(&structured, "segments")?;
        let StructuredInfoValueShape::Collection(segments) = collection.shape() else {
            return Err(Refusal::InvalidOutput);
        };
        let segments = segments.to_vec();
        let mut preparation = Vec::new();
        let mut previous = None;
        for segment in &segments {
            value_admit(record_field_value(segment, "value")?)?;
            let start = time(segment, "start")?;
            let end = time(segment, "end")?;
            if !before(start.clone(), end.clone(), &mut preparation)? {
                return Err(Refusal::InvalidSpan);
            }
            if let Some(prior) = previous {
                if before(start, prior, &mut preparation)? {
                    return Err(Refusal::UnorderedOrOverlapping);
                }
            }
            let interpolation = AudioTrajectoryInterpolation::from_structured(record_field_value(
                segment,
                "interpolation",
            )?)?;
            if !boolean(STEP, interpolation, &mut preparation)? {
                return Err(Refusal::UnsupportedLinearProfile);
            }
            previous = Some(end);
        }
        Ok(Self {
            original,
            original_canonical,
            anchor,
            segments,
            preparation,
            value_admit,
        })
    }
    pub fn original(&self) -> &C {
        &self.original
    }
    pub fn original_canonical(&self) -> &[u8] {
        &self.original_canonical
    }
    pub fn preparation_executions(&self) -> &[SpeechCommonAcousticExecution] {
        &self.preparation
    }
    pub fn query(&self, canonical: &[u8]) -> Result<SpeechTypedStepCurveReceipt<C>, Refusal> {
        let query = AudioTrajectoryQuery::decode(canonical)?;
        let query_canonical = query.clone().encode()?;
        let mut executions = self.preparation.clone();
        if !boolean(
            ANCHOR,
            SpeechAcousticAnchorComparison::new(self.anchor.clone(), query.anchor().clone())?,
            &mut executions,
        )? {
            return Err(Refusal::ForeignAnchor);
        }
        for (index, segment) in self.segments.iter().enumerate() {
            let input = SpeechStepCoverageInput::new(
                time(segment, "end")?,
                index + 1 == self.segments.len(),
                AudioTimeFraction::new(
                    *query.time().denominator(),
                    *query.time().numerator_seconds(),
                )?,
                time(segment, "start")?,
            )?;
            if boolean(COVERS, input, &mut executions)? {
                let selected_segment = segment
                    .canonical_bytes()
                    .map_err(|_| Refusal::InvalidOutput)?;
                let result = (self.value_admit)(record_field_value(segment, "value")?)?;
                let result_canonical = result
                    .canonical_bytes()
                    .map_err(|_| Refusal::InvalidOutput)?;
                return Ok(SpeechTypedStepCurveReceipt {
                    original: self.original.clone(),
                    original_canonical: self.original_canonical.clone(),
                    query,
                    query_canonical,
                    selected_segment,
                    executions,
                    result,
                    result_canonical,
                });
            }
        }
        Err(Refusal::MissingCoverage)
    }
}
pub fn prepare_speech_rate_step_curve(
    canonical: &[u8],
) -> Result<PreparedSpeechTypedStepCurve<SpeechSyllabicRateCurve>, Refusal> {
    PreparedSpeechTypedStepCurve::prepare(canonical, admit_rate)
}
pub fn prepare_speech_probability_step_curve(
    canonical: &[u8],
) -> Result<PreparedSpeechTypedStepCurve<SpeechProbabilityCurve>, Refusal> {
    PreparedSpeechTypedStepCurve::prepare(canonical, admit_probability)
}
pub fn prepare_speech_tilt_step_curve(
    canonical: &[u8],
) -> Result<PreparedSpeechTypedStepCurve<SpeechSpectralTiltCurve>, Refusal> {
    PreparedSpeechTypedStepCurve::prepare(canonical, admit_tilt)
}

fn admit_rate(value: StructuredInfoValue) -> Result<StructuredInfoValue, Refusal> {
    Ok(SpeechSyllabicRateSpecification::from_structured(value)?.into_structured()?)
}
pub(crate) fn admit_probability(
    value: StructuredInfoValue,
) -> Result<StructuredInfoValue, Refusal> {
    let admitted = SpeechProbabilitySpecification::from_structured(value)?;
    if let SpeechProbabilitySpecification::Known(payload) = &admitted {
        SpeechUnitInterval::new(*payload.denominator(), *payload.numerator())?;
    }
    Ok(admitted.into_structured()?)
}
fn admit_tilt(value: StructuredInfoValue) -> Result<StructuredInfoValue, Refusal> {
    Ok(SpeechSpectralTiltSpecification::from_structured(value)?.into_structured()?)
}

pub fn prepare_speech_duration_step_curve(
    canonical: &[u8],
) -> Result<PreparedSpeechTypedStepCurve<SpeechDurationCurve>, Refusal> {
    PreparedSpeechTypedStepCurve::prepare(canonical, admit_duration)
}
pub fn prepare_speech_decibel_step_curve(
    canonical: &[u8],
) -> Result<PreparedSpeechTypedStepCurve<SpeechDecibelLevelCurve>, Refusal> {
    PreparedSpeechTypedStepCurve::prepare(canonical, admit_decibel)
}
fn admit_duration(value: StructuredInfoValue) -> Result<StructuredInfoValue, Refusal> {
    Ok(SpeechDurationSpecification::from_structured(value)?.into_structured()?)
}
fn admit_decibel(value: StructuredInfoValue) -> Result<StructuredInfoValue, Refusal> {
    Ok(SpeechDecibelLevelSpecification::from_structured(value)?.into_structured()?)
}
