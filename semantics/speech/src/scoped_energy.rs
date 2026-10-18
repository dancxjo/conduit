//! Original broad linguistic scopes for overlapping amplitude/power controls.
//! Exact references are retained; generic Audio owns coverage and priority.
use crate::{
    correspondence::PreparedSpeechCorrespondence,
    intent_frequency_trajectory::{validate_scope, FrequencyGestureRefusal},
    semantic::*,
    syllable_structure::PreparedSyllableStructure,
};
use alloc::vec::Vec;
use conduit_audio::*;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};

pub struct PreparedScopedEnergy<'a, 'owner, 'inventory> {
    original: &'a PreparedSpeechCorrespondence<'owner, 'inventory>,
    structure: &'a PreparedSyllableStructure<'owner>,
    supplied: SpeechScopedEnergyGestureSet,
    audio: PreparedAudioControlTrajectories,
}
impl<'a, 'owner, 'inventory> PreparedScopedEnergy<'a, 'owner, 'inventory> {
    pub fn original(&self) -> &'a PreparedSpeechCorrespondence<'owner, 'inventory> {
        self.original
    }
    pub fn structure(&self) -> &'a PreparedSyllableStructure<'owner> {
        self.structure
    }
    pub fn supplied(&self) -> &SpeechScopedEnergyGestureSet {
        &self.supplied
    }
    pub fn audio(&self) -> &PreparedAudioControlTrajectories {
        &self.audio
    }
    pub fn evaluate(
        &self,
        query: &AudioTrajectoryQuery,
    ) -> Result<AudioControlEvaluation, FrequencyGestureRefusal> {
        self.audio
            .evaluate(&query.clone().encode()?)
            .map_err(FrequencyGestureRefusal::Audio)
    }
}

pub fn prepare_scoped_energy<'a, 'owner, 'inventory>(
    original: &'a PreparedSpeechCorrespondence<'owner, 'inventory>,
    structure: &'a PreparedSyllableStructure<'owner>,
    supplied: &SpeechScopedEnergyGestureSet,
) -> Result<PreparedScopedEnergy<'a, 'owner, 'inventory>, FrequencyGestureRefusal> {
    if !core::ptr::eq(original.intent(), structure.intent())
        || !core::ptr::eq(original.phones(), structure.phones())
        || !core::ptr::eq(original.text(), structure.text())
    {
        return Err(FrequencyGestureRefusal::ForeignStructure);
    }
    let supplied = SpeechScopedEnergyGestureSet::decode(&supplied.clone().encode()?)?;
    let mut controls = Vec::with_capacity(supplied.targets().len());
    for (index, target) in supplied.targets().as_slice().iter().enumerate() {
        validate_scope(
            original,
            structure,
            target.scope(),
            target.control().trajectory().anchor(),
        )?;
        for earlier in &supplied.targets().as_slice()[..index] {
            SpeechScopedEnergyReferenceAgreement::new(earlier.clone(), target.clone())?;
        }
        controls.push(target.control().clone());
    }
    let audio = AudioControlTrajectorySet::new(
        *supplied.conflict(),
        BoundedSequence::try_from_iter(controls).map_err(|_| FrequencyGestureRefusal::Capacity)?,
        *supplied.gap(),
    )?;
    let audio = PreparedAudioControlTrajectories::new(&audio.encode()?)
        .map_err(FrequencyGestureRefusal::Audio)?;
    Ok(PreparedScopedEnergy {
        original,
        structure,
        supplied,
        audio,
    })
}
