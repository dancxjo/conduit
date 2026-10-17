//! Prepared checked Source conversion with native admission on both boundaries.
//! Preparation and native decoding allocate; this is not an admitted Play Back.
use crate::source_programs::{AMPLITUDE_TO_POWER, CYCLE_TO_FREQUENCY, FREQUENCY_TO_CYCLE};
use crate::{AudioCycleDuration, AudioFrequencyHz};
use alloc::vec::Vec;
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};
use conduit_plot::{PortableExpressionEvaluationRefusal, PortableExpressionProgram};

#[derive(Debug)]
pub enum AcousticQuantityRefusal {
    InvalidProgram,
    Admission(NativeBindingRefusal),
    Evaluation(PortableExpressionEvaluationRefusal),
}

/// Exact admitted source, executed program and admitted result for audit.
pub struct AcousticConversion<Input, Output> {
    input: Input,
    result: Output,
    original_canonical: Vec<u8>,
    result_canonical: Vec<u8>,
    source_program_hex: &'static str,
}

impl<Input, Output> AcousticConversion<Input, Output> {
    pub fn input(&self) -> &Input {
        &self.input
    }
    pub fn result(&self) -> &Output {
        &self.result
    }
    pub fn original_canonical(&self) -> &[u8] {
        &self.original_canonical
    }
    pub fn result_canonical(&self) -> &[u8] {
        &self.result_canonical
    }
    pub fn source_program_hex(&self) -> &'static str {
        self.source_program_hex
    }
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

/// Original full-domain request plus its exact admitted execution projection.
pub struct AmplitudePowerConversion {
    original: crate::AudioAmplitudePowerRequest,
    original_canonical: Vec<u8>,
    executed: AcousticConversion<
        crate::AudioAmplitudePowerEligible,
        crate::generated::AudioAmplitudePowerSquared,
    >,
    result: crate::AudioPowerRatio,
    admitted_result_canonical: Vec<u8>,
}
impl AmplitudePowerConversion {
    pub fn original(&self) -> &crate::AudioAmplitudePowerRequest {
        &self.original
    }
    pub fn original_canonical(&self) -> &[u8] {
        &self.original_canonical
    }
    pub fn eligible_canonical(&self) -> &[u8] {
        self.executed.original_canonical()
    }
    pub fn raw_result_canonical(&self) -> &[u8] {
        self.executed.result_canonical()
    }
    pub fn source_program_hex(&self) -> &'static str {
        self.executed.source_program_hex()
    }
    pub fn result(&self) -> &crate::AudioPowerRatio {
        &self.result
    }
    pub fn admitted_result_canonical(&self) -> &[u8] {
        &self.admitted_result_canonical
    }
}

/// Allocating preparation/conformance seam, not an allocation-free Play Back.
pub struct PreparedAmplitudePower {
    program: PortableExpressionProgram,
}
impl PreparedAmplitudePower {
    pub fn new() -> Result<Self, AcousticQuantityRefusal> {
        Ok(Self {
            program: PortableExpressionProgram::from_canonical_hex(AMPLITUDE_TO_POWER)
                .map_err(|_| AcousticQuantityRefusal::InvalidProgram)?,
        })
    }
    pub fn convert(
        &self,
        canonical: &[u8],
    ) -> Result<AmplitudePowerConversion, AcousticQuantityRefusal> {
        let original = crate::AudioAmplitudePowerRequest::decode(canonical)
            .map_err(AcousticQuantityRefusal::Admission)?;
        // Source-owned eligibility validates full original request before square.
        let input = crate::AudioAmplitudePowerEligible::new(original)
            .map_err(AcousticQuantityRefusal::Admission)?;
        let executed_frame = input.encode().map_err(AcousticQuantityRefusal::Admission)?;
        let output = self
            .program
            .evaluate(&executed_frame)
            .map_err(AcousticQuantityRefusal::Evaluation)?;
        let raw = crate::generated::AudioAmplitudePowerSquared::decode(&output)
            .map_err(AcousticQuantityRefusal::Admission)?;
        let result = admit_squared(raw)?;
        let admitted_result_canonical = result
            .encode()
            .map_err(AcousticQuantityRefusal::Admission)?;
        Ok(AmplitudePowerConversion {
            original,
            result,
            admitted_result_canonical,
            original_canonical: canonical.into(),
            executed: AcousticConversion {
                input,
                result: raw,
                original_canonical: executed_frame,
                result_canonical: output,
                source_program_hex: AMPLITUDE_TO_POWER,
            },
        })
    }
}

fn admit_squared(
    raw: crate::generated::AudioAmplitudePowerSquared,
) -> Result<crate::AudioPowerRatio, AcousticQuantityRefusal> {
    crate::AudioPowerRatio::new(*raw.denominator(), *raw.numerator())
        .map_err(AcousticQuantityRefusal::Admission)
}

#[cfg(test)]
mod amplitude_admission_tests {
    use super::*;
    #[test]
    fn raw_square_is_not_semantic_admission() {
        let forged = crate::generated::AudioAmplitudePowerSquared::new(0, 4).unwrap();
        assert!(admit_squared(forged).is_err());
    }
}
