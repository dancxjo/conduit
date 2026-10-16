//! Borrowed fidelity binding for an actual emitted observation and retained PCM.
//! Preparation rechecks the pair; reports do not attest execution or infer speech.
use crate::{
    AudioPitchObservation, PcmPitchProfile, PcmPitchRefusal, PcmPitchWorkspace, PCM_PITCH_PROFILE,
};
use alloc::vec::Vec;
use conduit_core::projection::*;
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};
#[derive(Debug)]
pub enum PcmPitchEvidenceRefusal {
    Analysis(PcmPitchRefusal),
    Native(NativeBindingRefusal),
    ObservationMismatch,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PcmPitchReduction {
    pub source_frames: u16,
    pub observed_frames: u16,
}
pub struct PcmPitchEvidence<'a> {
    source: &'a [u8],
    observation: &'a AudioPitchObservation,
    native: Vec<u8>,
    reduction: PcmPitchReduction,
}
fn text(value: &str) -> ProjectionText<'_> {
    ProjectionText::new(value).expect("bounded static pitch fidelity identity")
}
impl<'a> PcmPitchEvidence<'a> {
    /// Call outside Play with the actual emitted value and original input. This
    /// validates their complete relation, not merely matching caller-supplied IDs.
    pub fn bind(
        profile: PcmPitchProfile,
        source: &'a [u8],
        observation: &'a AudioPitchObservation,
    ) -> Result<Self, PcmPitchEvidenceRefusal> {
        let expected = profile
            .observe(&mut PcmPitchWorkspace::default(), source)
            .map_err(PcmPitchEvidenceRefusal::Analysis)?
            .prepare_native()
            .map_err(PcmPitchEvidenceRefusal::Native)?;
        if observation != &expected {
            return Err(PcmPitchEvidenceRefusal::ObservationMismatch);
        }
        let native = observation
            .clone()
            .encode()
            .map_err(PcmPitchEvidenceRefusal::Native)?;
        Ok(Self {
            source,
            observation,
            native,
            reduction: PcmPitchReduction {
                source_frames: profile.source_frames(),
                observed_frames: *observation.window_frames(),
            },
        })
    }
    pub fn source(&self) -> &&'a [u8] {
        &self.source
    }
    pub fn observation(&self) -> &AudioPitchObservation {
        self.observation
    }
    pub fn facts(&self) -> [ProjectionFact<'_, Self>; 3] {
        [
            ProjectionFact::Preserved {
                obligation: text("source-clock-window-custody"),
            },
            ProjectionFact::Transformed {
                obligation: text("observed-periodicity"),
                law: text(PCM_PITCH_PROFILE),
            },
            ProjectionFact::Lost {
                obligation: text("waveform"),
                class: ProjectionLoss::Cardinality,
                detail: &self.reduction,
                native_fact: None,
            },
        ]
    }
    pub fn native(&self) -> [ProjectionNativeFact<'_>; 1] {
        [ProjectionNativeFact {
            identity: text("pitch-observation"),
            contract: text("audio/observed-periodicity@1"),
            provider: text(PCM_PITCH_PROFILE),
            encoding: text("structured-native@1"),
            bytes: &self.native,
        }]
    }
    fn valid_facts(&self, facts: &[ProjectionFact<'_, Self>]) -> bool {
        facts.len() == 3
            && matches!(&facts[0], ProjectionFact::Preserved {obligation} if obligation.as_str()=="source-clock-window-custody")
            && matches!(&facts[1], ProjectionFact::Transformed {obligation,law} if obligation.as_str()=="observed-periodicity" && law.as_str()==PCM_PITCH_PROFILE)
            && matches!(&facts[2], ProjectionFact::Lost {obligation,class,detail,native_fact} if obligation.as_str()=="waveform" && *class==ProjectionLoss::Cardinality && **detail==self.reduction && native_fact.is_none())
    }
}
impl<'a> ProjectionDomain for PcmPitchEvidence<'a> {
    type Source = &'a [u8];
    type Target = AudioPitchObservation;
    type Detail = PcmPitchReduction;
    fn source_contract(&self) -> ProjectionText<'_> {
        text("audio/immutable-mono-pcm@1")
    }
    fn target_contract(&self) -> ProjectionText<'_> {
        text("audio/observed-periodicity@1")
    }
    fn validate(
        &self,
        source: &Self::Source,
        target: Option<&Self::Target>,
        facts: &[ProjectionFact<'_, Self>],
        native: &[ProjectionNativeFact<'_>],
        scores: &[ProjectionScore<'_>],
        mechanism: ProjectionMechanism,
    ) -> bool {
        if *source != self.source
            || target != Some(self.observation)
            || !self.valid_facts(facts)
            || native.len() != 1
            || !scores.is_empty()
            || mechanism != ProjectionMechanism::Completed
        {
            return false;
        }
        let expected = self.native()[0];
        let actual = native[0];
        actual.identity == expected.identity
            && actual.contract == expected.contract
            && actual.provider == expected.provider
            && actual.encoding == expected.encoding
            && actual.bytes == expected.bytes
    }
}
/// Explicitly permits only the declared reduction to this profile's observation.
/// Strict consumers still reject it; no waveform preservation is invented.
pub struct PcmPitchObservationPolicy;
impl<'a> ProjectionPolicy<PcmPitchEvidence<'a>> for PcmPitchObservationPolicy {
    fn identity(&self) -> ProjectionText<'_> {
        text("audio/permit-periodicity-observation-only@1")
    }
    fn permits(
        &self,
        _: &&'a [u8],
        _: &AudioPitchObservation,
        facts: &[ProjectionFact<'_, PcmPitchEvidence<'a>>],
    ) -> bool {
        facts.len() == 3
            && matches!(&facts[2], ProjectionFact::Lost {obligation,class,..}
            if obligation.as_str()=="waveform" && *class==ProjectionLoss::Cardinality)
    }
}
