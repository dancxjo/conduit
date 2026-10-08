//! Known Audio controls retain their exact Speech role, sources and provenance.
use crate::common_acoustic_programs::AUDIO_DOMAIN;
use crate::common_acoustic_quantities::*;
use crate::semantic::*;
use alloc::vec::Vec;
use conduit_plot::rust_binding::NativeRustBinding;
#[derive(Debug)]
pub enum SpeechAudioTargetRefusal {
    Common(SpeechCommonAcousticRefusal),
    Audio(conduit_audio::AudioTrajectoryRefusal),
    WrongQuantityDomain,
}
impl From<SpeechCommonAcousticRefusal> for SpeechAudioTargetRefusal {
    fn from(e: SpeechCommonAcousticRefusal) -> Self {
        Self::Common(e)
    }
}
impl From<conduit_plot::rust_binding::NativeBindingRefusal> for SpeechAudioTargetRefusal {
    fn from(e: conduit_plot::rust_binding::NativeBindingRefusal) -> Self {
        Self::Common(SpeechCommonAcousticRefusal::Admission(e))
    }
}
pub struct PreparedSpeechAudioTarget {
    original: SpeechKnownAudioTarget,
    original_canonical: Vec<u8>,
    preparation: Vec<SpeechCommonAcousticExecution>,
    audio: conduit_audio::PreparedAudioQuantityTrajectory,
}
pub struct SpeechAudioTargetReceipt {
    original: SpeechKnownAudioTarget,
    original_canonical: Vec<u8>,
    domain_executions: Vec<SpeechCommonAcousticExecution>,
    audio: conduit_audio::AudioTrajectoryEvaluation,
}
impl SpeechAudioTargetReceipt {
    pub fn original(&self) -> &SpeechKnownAudioTarget {
        &self.original
    }
    pub fn original_canonical(&self) -> &[u8] {
        &self.original_canonical
    }
    pub fn domain_executions(&self) -> &[SpeechCommonAcousticExecution] {
        &self.domain_executions
    }
    pub fn audio(&self) -> &conduit_audio::AudioTrajectoryEvaluation {
        &self.audio
    }
}
impl PreparedSpeechAudioTarget {
    pub fn new(canonical: &[u8]) -> Result<Self, SpeechAudioTargetRefusal> {
        let original = SpeechKnownAudioTarget::decode(canonical)?;
        let original_canonical = original.clone().encode()?;
        let mut preparation = Vec::new();
        for segment in original.trajectory().segments().as_slice() {
            for value in [segment.left(), segment.right()] {
                if !boolean(
                    AUDIO_DOMAIN,
                    SpeechTargetAudioDomain::new(*original.role(), value.clone())?,
                    &mut preparation,
                )? {
                    return Err(SpeechAudioTargetRefusal::WrongQuantityDomain);
                }
            }
        }
        let audio = conduit_audio::PreparedAudioQuantityTrajectory::new(
            &original.trajectory().clone().encode()?,
        )
        .map_err(SpeechAudioTargetRefusal::Audio)?;
        Ok(Self {
            original,
            original_canonical,
            preparation,
            audio,
        })
    }
    pub fn query(
        &self,
        canonical: &[u8],
    ) -> Result<SpeechAudioTargetReceipt, SpeechAudioTargetRefusal> {
        let audio = self
            .audio
            .evaluate(canonical)
            .map_err(SpeechAudioTargetRefusal::Audio)?;
        Ok(SpeechAudioTargetReceipt {
            original: self.original.clone(),
            original_canonical: self.original_canonical.clone(),
            domain_executions: self.preparation.clone(),
            audio,
        })
    }
}
