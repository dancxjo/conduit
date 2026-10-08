//! Prepared checked Source conversion with native admission on both boundaries.
//! Preparation and native decoding allocate; this is not an admitted Play Back.
use crate::{AudioCycleDuration, AudioFrequencyHz};
use alloc::vec::Vec;
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};
use conduit_plot::{PortableExpressionEvaluationRefusal, PortableExpressionProgram};
include!(concat!(env!("OUT_DIR"), "/acoustic_programs.rs"));

#[derive(Debug)]
pub enum AcousticQuantityRefusal {
    InvalidProgram,
    Admission(NativeBindingRefusal),
    Evaluation(PortableExpressionEvaluationRefusal),
}

/// Exact admitted source, executed program and admitted result for audit.
pub struct AcousticConversion<Input, Output> {
    pub input: Input,
    pub result: Output,
    pub original_canonical: Vec<u8>,
    pub result_canonical: Vec<u8>,
    pub source_program_hex: &'static str,
}

pub struct PreparedAcousticReciprocal {
    frequency_to_cycle: PortableExpressionProgram,
    cycle_to_frequency: PortableExpressionProgram,
}
impl PreparedAcousticReciprocal {
    pub fn new() -> Result<Self, AcousticQuantityRefusal> {
        Ok(Self {
            frequency_to_cycle: PortableExpressionProgram::from_canonical_hex(FREQUENCY_TO_CYCLE)
                .map_err(|_| AcousticQuantityRefusal::InvalidProgram)?,
            cycle_to_frequency: PortableExpressionProgram::from_canonical_hex(CYCLE_TO_FREQUENCY)
                .map_err(|_| AcousticQuantityRefusal::InvalidProgram)?,
        })
    }
    pub fn frequency_to_cycle(
        &self,
        canonical: &[u8],
    ) -> Result<AcousticConversion<AudioFrequencyHz, AudioCycleDuration>, AcousticQuantityRefusal>
    {
        let input =
            AudioFrequencyHz::decode(canonical).map_err(AcousticQuantityRefusal::Admission)?;
        let output = self
            .frequency_to_cycle
            .evaluate(canonical)
            .map_err(AcousticQuantityRefusal::Evaluation)?;
        let result =
            AudioCycleDuration::decode(&output).map_err(AcousticQuantityRefusal::Admission)?;
        Ok(AcousticConversion {
            input,
            result,
            original_canonical: canonical.into(),
            result_canonical: output,
            source_program_hex: FREQUENCY_TO_CYCLE,
        })
    }
    pub fn cycle_to_frequency(
        &self,
        canonical: &[u8],
    ) -> Result<AcousticConversion<AudioCycleDuration, AudioFrequencyHz>, AcousticQuantityRefusal>
    {
        let input =
            AudioCycleDuration::decode(canonical).map_err(AcousticQuantityRefusal::Admission)?;
        let output = self
            .cycle_to_frequency
            .evaluate(canonical)
            .map_err(AcousticQuantityRefusal::Evaluation)?;
        let result =
            AudioFrequencyHz::decode(&output).map_err(AcousticQuantityRefusal::Admission)?;
        Ok(AcousticConversion {
            input,
            result,
            original_canonical: canonical.into(),
            result_canonical: output,
            source_program_hex: CYCLE_TO_FREQUENCY,
        })
    }
}
