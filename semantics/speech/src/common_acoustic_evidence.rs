//! Typed observed acoustics remain distinct from authored target controls.
use crate::common_acoustic_curves::admit_probability;
use crate::common_acoustic_programs::{BEFORE, EQUAL};
use crate::common_acoustic_quantities::*;
use crate::semantic::*;
use alloc::vec::Vec;
use conduit_plot::rust_binding::NativeRustBinding;
pub struct SpeechCommonAcousticEvidenceReceipt {
    original: SpeechCommonAcousticEvidence,
    original_canonical: Vec<u8>,
    executions: Vec<SpeechCommonAcousticExecution>,
}
impl SpeechCommonAcousticEvidenceReceipt {
    pub fn original(&self) -> &SpeechCommonAcousticEvidence {
        &self.original
    }
    pub fn original_canonical(&self) -> &[u8] {
        &self.original_canonical
    }
    pub fn executions(&self) -> &[SpeechCommonAcousticExecution] {
        &self.executions
    }
    /// Retained raw harmonicity/vector material has no implemented numeric
    /// interpretation profile. Presence is not silently treated as zero.
    pub fn require_interpreted_measurements(&self) -> Result<(), SpeechCommonAcousticRefusal> {
        // No harmonicity/vector interpretation capability is currently offered.
        // Other fields remain available under their independently declared units.
        Err(SpeechCommonAcousticRefusal::UnsupportedMeasurementProfile)
    }
}
pub fn prepare_speech_acoustic_evidence(
    canonical: &[u8],
) -> Result<SpeechCommonAcousticEvidenceReceipt, SpeechCommonAcousticRefusal> {
    let original = SpeechCommonAcousticEvidence::decode(canonical)?;
    let original_canonical = original.clone().encode()?;
    let mut executions = Vec::new();
    admit_probability(original.voicing_probability().clone().into_structured()?)?;
    admit_probability(original.periodicity().clone().into_structured()?)?;
    let comparison = SpeechAcousticTimeComparison::new(
        original.span().start().clone(),
        original.span().end().clone(),
    )?;
    if !boolean(BEFORE, comparison.clone(), &mut executions)?
        && !boolean(EQUAL, comparison, &mut executions)?
    {
        return Err(SpeechCommonAcousticRefusal::InvalidSpan);
    }
    Ok(SpeechCommonAcousticEvidenceReceipt {
        original,
        original_canonical,
        executions,
    })
}
