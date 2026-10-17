//! Exact-power-of-ten capability within broad explicit-reference level domains.
use crate::generated::*;
use crate::source_execution::{AudioSourceExecution, AudioSourceExecutionRefusal, Program};
use crate::source_programs::*;
use alloc::vec::Vec;
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};

/// Admitted original and Source recognition proof for a supported-domain value
/// outside this numeric capability. No approximate logarithm is substituted.
pub struct AudioUnsupportedDecibelProfile {
    original: Vec<u8>,
    executions: Vec<AudioSourceExecution>,
}
impl AudioUnsupportedDecibelProfile {
    pub fn original_canonical(&self) -> &[u8] {
        &self.original
    }
    pub fn executions(&self) -> &[AudioSourceExecution] {
        &self.executions
    }
}
pub enum AudioDecibelRefusal {
    Admission(NativeBindingRefusal),
    Source(AudioSourceExecutionRefusal),
    UnsupportedExactPowerOfTen(AudioUnsupportedDecibelProfile),
}
impl core::fmt::Debug for AudioDecibelRefusal {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Admission(e) => f.debug_tuple("Admission").field(e).finish(),
            Self::Source(e) => f.debug_tuple("Source").field(e).finish(),
            Self::UnsupportedExactPowerOfTen(_) => f.write_str("UnsupportedExactPowerOfTen"),
        }
    }
}
impl From<NativeBindingRefusal> for AudioDecibelRefusal {
    fn from(e: NativeBindingRefusal) -> Self {
        Self::Admission(e)
    }
}
impl From<AudioSourceExecutionRefusal> for AudioDecibelRefusal {
    fn from(e: AudioSourceExecutionRefusal) -> Self {
        Self::Source(e)
    }
}
/// Original full semantic request and exact Source programs/inputs/raw outputs,
/// followed by semantic Native admission. Construction is private.
pub struct AudioDecibelReceipt<I, O> {
    original: I,
    original_canonical: Vec<u8>,
    executions: Vec<AudioSourceExecution>,
    result: O,
    admitted: Vec<u8>,
}
impl<I, O> AudioDecibelReceipt<I, O> {
    pub fn original(&self) -> &I {
        &self.original
    }
    pub fn original_canonical(&self) -> &[u8] {
        &self.original_canonical
    }
    pub fn executions(&self) -> &[AudioSourceExecution] {
        &self.executions
    }
    pub fn result(&self) -> &O {
        &self.result
    }
    pub fn admitted_canonical(&self) -> &[u8] {
        &self.admitted
    }
}
pub type AudioRatioDecibelReceipt =
    AudioDecibelReceipt<AudioReferencedLevelRatio, AudioDecibelLevel>;
pub type AudioDecibelRatioReceipt =
    AudioDecibelReceipt<AudioDecibelLevel, AudioReferencedLevelRatio>;

pub struct PreparedExactPowerOfTenDecibels {
    working: Program,
    recognize: Program,
    from_power: Program,
    inverse: Program,
    to_power: Program,
}
impl PreparedExactPowerOfTenDecibels {
    pub fn new() -> Result<Self, AudioDecibelRefusal> {
        Ok(Self {
            working: Program::new(DECIBEL_WORKING)?,
            recognize: Program::new(DECIBEL_RECOGNIZE)?,
            from_power: Program::new(DECIBEL_FROM_POWER)?,
            inverse: Program::new(DECIBEL_INVERSE)?,
            to_power: Program::new(DECIBEL_TO_POWER)?,
        })
    }
    pub fn ratio_to_decibels(
        &self,
        canonical: &[u8],
    ) -> Result<AudioRatioDecibelReceipt, AudioDecibelRefusal> {
        let original = AudioReferencedLevelRatio::decode(canonical)?;
        let original_canonical = original.clone().encode()?;
        let mut executions = Vec::new();
        let working: AudioDecibelRatioWorking = self
            .working
            .native(original.ratio().clone(), &mut executions)?;
        let recognition: AudioExactPowerOfTen = self.recognize.native(working, &mut executions)?;
        let eligible = eligible(
            recognition,
            *original.basis().convention(),
            &original_canonical,
            &executions,
        )?;
        let value: AudioDecibelValue = self.from_power.native(eligible, &mut executions)?;
        let result = AudioDecibelLevel::new(original.basis().clone(), value)?;
        let admitted = result.clone().encode()?;
        Ok(AudioDecibelReceipt {
            original,
            original_canonical,
            executions,
            result,
            admitted,
        })
    }
    pub fn decibels_to_ratio(
        &self,
        canonical: &[u8],
    ) -> Result<AudioDecibelRatioReceipt, AudioDecibelRefusal> {
        let original = AudioDecibelLevel::decode(canonical)?;
        let original_canonical = original.clone().encode()?;
        let mut executions = Vec::new();
        let input = AudioDecibelInverseInput::new(
            *original.basis().convention(),
            original.value().clone(),
        )?;
        let recognition: AudioExactPowerOfTen = self.inverse.native(input, &mut executions)?;
        let eligible = eligible(
            recognition,
            *original.basis().convention(),
            &original_canonical,
            &executions,
        )?;
        let raw: AudioDecibelRatioRaw = self.to_power.native(eligible, &mut executions)?;
        let result = admit_ratio(original.basis().clone(), raw)?;
        let admitted = result.clone().encode()?;
        Ok(AudioDecibelReceipt {
            original,
            original_canonical,
            executions,
            result,
            admitted,
        })
    }
}
fn eligible(
    recognition: AudioExactPowerOfTen,
    convention: AudioDecibelConvention,
    original: &[u8],
    executions: &[AudioSourceExecution],
) -> Result<AudioExactPowerOfTenEligible, AudioDecibelRefusal> {
    if !recognition.supported() {
        return Err(AudioDecibelRefusal::UnsupportedExactPowerOfTen(
            AudioUnsupportedDecibelProfile {
                original: original.to_vec(),
                executions: executions.to_vec(),
            },
        ));
    }
    Ok(AudioExactPowerOfTenEligible::new(convention, recognition)?)
}
// Exact field-copy only: denominator positivity belongs to semantic admission.
fn admit_ratio(
    basis: AudioDecibelBasis,
    raw: AudioDecibelRatioRaw,
) -> Result<AudioReferencedLevelRatio, NativeBindingRefusal> {
    let ratio = match basis.convention() {
        AudioDecibelConvention::AmplitudeTwentyLog10 => {
            AudioLevelRatio::amplitude(*raw.denominator(), *raw.numerator())?
        }
        AudioDecibelConvention::PowerTenLog10 => {
            AudioLevelRatio::power(*raw.denominator(), *raw.numerator())?
        }
    };
    AudioReferencedLevelRatio::new(basis, ratio)
}
/// Declared reference pair under the same positive proportionality premise.
/// The premise is authored metadata, not measured reference authority.
pub struct AudioReferencedAmplitudePowerReceipt {
    original: AudioReferencedAmplitudePowerRequest,
    original_canonical: Vec<u8>,
    square: crate::AmplitudePowerConversion,
    result: AudioReferencedLevelRatio,
    admitted: Vec<u8>,
}
impl AudioReferencedAmplitudePowerReceipt {
    pub fn original(&self) -> &AudioReferencedAmplitudePowerRequest {
        &self.original
    }
    pub fn original_canonical(&self) -> &[u8] {
        &self.original_canonical
    }
    pub fn square(&self) -> &crate::AmplitudePowerConversion {
        &self.square
    }
    pub fn result(&self) -> &AudioReferencedLevelRatio {
        &self.result
    }
    pub fn admitted_canonical(&self) -> &[u8] {
        &self.admitted
    }
}
pub fn convert_referenced_amplitude_to_power(
    canonical: &[u8],
) -> Result<AudioReferencedAmplitudePowerReceipt, crate::AcousticQuantityRefusal> {
    use crate::AcousticQuantityRefusal as Refusal;
    let original =
        AudioReferencedAmplitudePowerRequest::decode(canonical).map_err(Refusal::Admission)?;
    let original_canonical = original.clone().encode().map_err(Refusal::Admission)?;
    let request = AudioAmplitudePowerRequest::new(
        *original.amplitude(),
        *original.references().relationship(),
    )
    .map_err(Refusal::Admission)?;
    let square = crate::PreparedAmplitudePower::new()?
        .convert(&request.encode().map_err(Refusal::Admission)?)?;
    let basis = AudioDecibelBasis::new(
        AudioDecibelConvention::PowerTenLog10,
        original.references().power().clone(),
    )
    .map_err(Refusal::Admission)?;
    let ratio =
        AudioLevelRatio::power(*square.result().denominator(), *square.result().numerator())
            .map_err(Refusal::Admission)?;
    let result = AudioReferencedLevelRatio::new(basis, ratio).map_err(Refusal::Admission)?;
    let admitted = result.clone().encode().map_err(Refusal::Admission)?;
    Ok(AudioReferencedAmplitudePowerReceipt {
        original,
        original_canonical,
        square,
        result,
        admitted,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn forged_zero_raw_denominator_refuses_semantic_admission() {
        let provenance = AudioTrajectoryProvenance::new(
            AudioTrajectoryProvenanceKind::Unknown,
            "test".into(),
            None,
        )
        .unwrap();
        let reference = AudioDecibelReference::new(
            1,
            "r".into(),
            1,
            provenance,
            AudioDecibelReferenceRole::Power,
            conduit_core::Unit::Second,
        )
        .unwrap();
        let basis =
            AudioDecibelBasis::new(AudioDecibelConvention::PowerTenLog10, reference).unwrap();
        let raw = AudioDecibelRatioRaw::new(0, 1).unwrap();
        assert!(admit_ratio(basis, raw).is_err());
    }
}
