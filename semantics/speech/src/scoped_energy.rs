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
    /// Admits a supplied measured/inferred datum without substituting it for
    /// the intended trajectory or authenticating its provenance.
    pub fn observe(
        &self,
        index: usize,
        supplied: &SpeechScopedEnergyObservation,
    ) -> Result<AdmittedScopedEnergyObservation<'_, 'a, 'owner, 'inventory>, FrequencyGestureRefusal>
    {
        let target = self
            .supplied
            .targets()
            .as_slice()
            .get(index)
            .ok_or(FrequencyGestureRefusal::Target)?;
        let supplied = SpeechScopedEnergyObservation::decode(&supplied.clone().encode()?)?;
        SpeechScopedEnergyObservationMatch::new(supplied.clone(), target.clone())?;
        validate_scope(
            self.original,
            self.structure,
            supplied.scope(),
            target.control().trajectory().anchor(),
        )?;
        let check = |error| SpeechScopedEnergyErrorMatch::new(error, supplied.clone()).map(|_| ());
        match supplied.absolute_linear_error() {
            SpeechEnergyErrorSpecification::Known(error) => {
                check(AudioReferencedLevelRatio::new(
                    error.basis().clone(),
                    error.ratio().clone(),
                )?)?;
            }
            SpeechEnergyErrorSpecification::Variable(errors) => {
                for error in errors.as_slice() {
                    check(error.clone())?;
                }
            }
            SpeechEnergyErrorSpecification::Gradient(gradient) => check(gradient.value().clone())?,
            SpeechEnergyErrorSpecification::Unknown
            | SpeechEnergyErrorSpecification::Unspecified
            | SpeechEnergyErrorSpecification::NotApplicable => {}
        }
        Ok(AdmittedScopedEnergyObservation {
            targets: self,
            index,
            supplied,
        })
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

/// The owner receipt keeps intended controls and supplied evidence distinct.
pub struct AdmittedScopedEnergyObservation<'receipt, 'a, 'owner, 'inventory> {
    targets: &'receipt PreparedScopedEnergy<'a, 'owner, 'inventory>,
    index: usize,
    supplied: SpeechScopedEnergyObservation,
}
impl AdmittedScopedEnergyObservation<'_, '_, '_, '_> {
    pub fn original(&self) -> &PreparedSpeechCorrespondence<'_, '_> {
        self.targets.original()
    }
    pub fn target(&self) -> &SpeechScopedEnergyTarget {
        &self.targets.supplied().targets().as_slice()[self.index]
    }
    pub fn supplied(&self) -> &SpeechScopedEnergyObservation {
        &self.supplied
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
