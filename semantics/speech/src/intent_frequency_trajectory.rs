//! Exact linguistic custody for overlapping generic Audio frequency gestures.
//! Preparation executes declared Audio policy; it never measures or infers a
//! frequency, substitutes an observation, or interprets a formant as F0.
//! Native values retain Core's finite canonical envelope: oversized repeated
//! full inventory bases refuse rather than being abbreviated or truncated.
//! Audio's admitted coverage policy requires every original control to cover a
//! query. Priority chooses a value per control identity; this is not additive
//! mixing. Observation allowances remain in Hz after reciprocal conversion;
//! this bridge makes no inferred period-uncertainty claim.
use crate::{
    correspondence::PreparedSpeechCorrespondence, semantic::*,
    syllable_structure::PreparedSyllableStructure,
};
use alloc::vec::Vec;
use conduit_audio::*;
use conduit_plot::rust_binding::{BoundedSequence, NativeBindingRefusal, NativeRustBinding};

#[derive(Debug)]
pub enum FrequencyGestureRefusal {
    Native(NativeBindingRefusal),
    ForeignStructure,
    InventoryBasis,
    Source,
    Anchor,
    Phone,
    Syllable,
    Word,
    Phrase,
    Role,
    Target,
    Capacity,
    UnselectedTarget { index: usize },
    Audio(AudioControlRefusal),
    Reciprocal(AcousticQuantityRefusal),
}
impl From<NativeBindingRefusal> for FrequencyGestureRefusal {
    fn from(value: NativeBindingRefusal) -> Self {
        Self::Native(value)
    }
}

/// Every independent original scope remains paired by index with its Audio
/// control. Generic priority selects indices without rewriting these originals.
pub struct PreparedFrequencyGestures<'a, 'owner, 'inventory> {
    original: &'a PreparedSpeechCorrespondence<'owner, 'inventory>,
    structure: &'a PreparedSyllableStructure<'owner>,
    set: SpeechFrequencyGestureSet,
    audio: PreparedAudioControlTrajectories,
}
impl<'a, 'owner, 'inventory> PreparedFrequencyGestures<'a, 'owner, 'inventory> {
    pub fn original(&self) -> &'a PreparedSpeechCorrespondence<'owner, 'inventory> {
        self.original
    }
    pub fn structure(&self) -> &'a PreparedSyllableStructure<'owner> {
        self.structure
    }
    pub fn targets(&self) -> &SpeechFrequencyGestureSet {
        &self.set
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
    /// Convert only a declared fundamental target, evaluated through its original
    /// Audio set at the supplied query. All competing evaluations remain in it.
    pub fn fundamental_cycle(
        &self,
        index: usize,
        query: &AudioTrajectoryQuery,
    ) -> Result<FrequencyGestureCycleReceipt, FrequencyGestureRefusal> {
        let target = self
            .set
            .targets()
            .as_slice()
            .get(index)
            .ok_or(FrequencyGestureRefusal::Target)?;
        if !matches!(target.role(), SpeechFrequencyRole::Fundamental) {
            return Err(FrequencyGestureRefusal::Role);
        }
        let evaluation = self.evaluate(query)?;
        if !evaluation.selected_indices().contains(&index) {
            return Err(FrequencyGestureRefusal::UnselectedTarget { index });
        }
        let AudioTrajectoryQuantity::Frequency(value) = evaluation.evaluations()[index].result()
        else {
            return Err(FrequencyGestureRefusal::Role);
        };
        let conversion = reciprocal(value)?;
        Ok(FrequencyGestureCycleReceipt {
            evaluation,
            target_index: index,
            conversion,
        })
    }
    /// Measurement and inference keep separate Native cases. This admits only
    /// supplied evidence and does not authenticate a sensor or model.
    pub fn observe(
        &self,
        index: usize,
        supplied: &SpeechFrequencyObservation,
    ) -> Result<AdmittedFrequencyObservation<'_, 'a, 'owner, 'inventory>, FrequencyGestureRefusal>
    {
        let target = self
            .set
            .targets()
            .as_slice()
            .get(index)
            .ok_or(FrequencyGestureRefusal::Target)?;
        let supplied = SpeechFrequencyObservation::decode(&supplied.clone().encode()?)?;
        let evidence = evidence(&supplied);
        SpeechFrequencyObservationMatch::new(evidence.clone(), target.clone())?;
        validate_scope(
            self.original,
            self.structure,
            evidence.scope(),
            evidence.at().anchor(),
        )?;
        Ok(AdmittedFrequencyObservation {
            targets: self,
            target_index: index,
            supplied,
        })
    }
}
pub struct FrequencyGestureCycleReceipt {
    evaluation: AudioControlEvaluation,
    target_index: usize,
    conversion: AcousticConversion<AudioFrequencyHz, AudioCycleDuration>,
}
impl FrequencyGestureCycleReceipt {
    pub fn evaluation(&self) -> &AudioControlEvaluation {
        &self.evaluation
    }
    pub fn target_index(&self) -> usize {
        self.target_index
    }
    pub fn conversion(&self) -> &AcousticConversion<AudioFrequencyHz, AudioCycleDuration> {
        &self.conversion
    }
}
pub struct AdmittedFrequencyObservation<'receipt, 'a, 'owner, 'inventory> {
    targets: &'receipt PreparedFrequencyGestures<'a, 'owner, 'inventory>,
    target_index: usize,
    supplied: SpeechFrequencyObservation,
}
impl AdmittedFrequencyObservation<'_, '_, '_, '_> {
    pub fn original(&self) -> &PreparedSpeechCorrespondence<'_, '_> {
        self.targets.original()
    }
    pub fn target_index(&self) -> usize {
        self.target_index
    }
    pub fn supplied(&self) -> &SpeechFrequencyObservation {
        &self.supplied
    }
    pub fn evidence(&self) -> &SpeechFrequencyObservationEvidence {
        evidence(&self.supplied)
    }
    pub fn fundamental_cycle(
        &self,
    ) -> Result<AcousticConversion<AudioFrequencyHz, AudioCycleDuration>, FrequencyGestureRefusal>
    {
        if !matches!(self.evidence().role(), SpeechFrequencyRole::Fundamental) {
            return Err(FrequencyGestureRefusal::Role);
        }
        reciprocal(self.evidence().frequency())
    }
}
fn reciprocal(
    value: &AudioFrequencyHz,
) -> Result<AcousticConversion<AudioFrequencyHz, AudioCycleDuration>, FrequencyGestureRefusal> {
    PreparedAcousticReciprocal::new()
        .and_then(|p| {
            p.frequency_to_cycle(&value.encode().map_err(AcousticQuantityRefusal::Admission)?)
        })
        .map_err(FrequencyGestureRefusal::Reciprocal)
}
fn evidence(observation: &SpeechFrequencyObservation) -> &SpeechFrequencyObservationEvidence {
    match observation {
        SpeechFrequencyObservation::Measured(value) => value.evidence(),
        SpeechFrequencyObservation::Inferred(value) => value.evidence(),
    }
}

pub fn prepare_frequency_gestures<'a, 'owner, 'inventory>(
    original: &'a PreparedSpeechCorrespondence<'owner, 'inventory>,
    structure: &'a PreparedSyllableStructure<'owner>,
    supplied: &SpeechFrequencyGestureSet,
) -> Result<PreparedFrequencyGestures<'a, 'owner, 'inventory>, FrequencyGestureRefusal> {
    if !core::ptr::eq(original.intent(), structure.intent())
        || !core::ptr::eq(original.phones(), structure.phones())
        || !core::ptr::eq(original.text(), structure.text())
    {
        return Err(FrequencyGestureRefusal::ForeignStructure);
    }
    let set = SpeechFrequencyGestureSet::decode(&supplied.clone().encode()?)?;
    let mut controls = Vec::with_capacity(set.targets().len());
    for (index, target) in set.targets().as_slice().iter().enumerate() {
        validate_scope(
            original,
            structure,
            target.scope(),
            target.control().trajectory().anchor(),
        )?;
        for earlier in &set.targets().as_slice()[..index] {
            SpeechFrequencyRoleAgreement::new(earlier.clone(), target.clone())?;
        }
        controls.push(target.control().clone());
    }
    let audio_set = AudioControlTrajectorySet::new(
        *set.conflict(),
        BoundedSequence::try_from_iter(controls).map_err(|_| FrequencyGestureRefusal::Capacity)?,
        *set.gap(),
    )?;
    let audio = PreparedAudioControlTrajectories::new(&audio_set.encode()?)
        .map_err(FrequencyGestureRefusal::Audio)?;
    Ok(PreparedFrequencyGestures {
        original,
        structure,
        set,
        audio,
    })
}

fn carrier(
    original: &PreparedSpeechCorrespondence<'_, '_>,
    value: &SpeechAcousticCarrierBasis,
    anchor: &AudioTrajectoryAnchor,
) -> Result<(), FrequencyGestureRefusal> {
    if value.inventory_basis() != original.inventory().basis() {
        return Err(FrequencyGestureRefusal::InventoryBasis);
    }
    if value.source() != original.alignment().source() {
        return Err(FrequencyGestureRefusal::Source);
    }
    if value.anchor() != anchor {
        return Err(FrequencyGestureRefusal::Anchor);
    }
    Ok(())
}
fn phone(
    original: &PreparedSpeechCorrespondence<'_, '_>,
    value: &LanguageSpeechTokenRef,
) -> Result<(), FrequencyGestureRefusal> {
    SpeechSequenceBasisMatch::new(original.phones().basis().clone(), value.clone())?;
    if *value.ordinal() as usize >= original.phones().tokens().len() {
        return Err(FrequencyGestureRefusal::Phone);
    }
    Ok(())
}
fn validate_scope(
    original: &PreparedSpeechCorrespondence<'_, '_>,
    structure: &PreparedSyllableStructure<'_>,
    scope: &SpeechAcousticScope,
    anchor: &AudioTrajectoryAnchor,
) -> Result<(), FrequencyGestureRefusal> {
    match scope {
        SpeechAcousticScope::Phone(value) => {
            let value = value.value();
            if value.inventory_basis() != original.inventory().basis() {
                return Err(FrequencyGestureRefusal::InventoryBasis);
            }
            if value.source() != original.alignment().source() {
                return Err(FrequencyGestureRefusal::Source);
            }
            if value.anchor() != anchor {
                return Err(FrequencyGestureRefusal::Anchor);
            }
            phone(original, value.occurrence())?;
        }
        SpeechAcousticScope::Syllable(value) => {
            let value = value.value();
            carrier(original, value.carrier(), anchor)?;
            let source = structure
                .sources()
                .iter()
                .find(|s| s.syllable().identity() == value.syllable())
                .ok_or(FrequencyGestureRefusal::Syllable)?;
            if source.variety() != original.inventory().basis().variety() {
                return Err(FrequencyGestureRefusal::InventoryBasis);
            }
        }
        SpeechAcousticScope::Word(value) => {
            let value = value.value();
            carrier(original, value.carrier(), anchor)?;
            let mut found = false;
            for source in structure
                .sources()
                .iter()
                .filter(|s| s.word() == value.word())
            {
                if source.variety() != original.inventory().basis().variety() {
                    return Err(FrequencyGestureRefusal::InventoryBasis);
                }
                found = true;
            }
            if !found {
                return Err(FrequencyGestureRefusal::Word);
            }
            LanguageTextReferenceMatch::new(
                original.text().clone(),
                value.word().clone(),
                u32::try_from(original.text().text().chars().count())
                    .map_err(|_| FrequencyGestureRefusal::Capacity)?,
            )?;
        }
        SpeechAcousticScope::Phrase(value) => {
            let value = value.value();
            carrier(original, value.carrier(), anchor)?;
            phone(original, value.first())?;
            if value.boundary().basis() != original.phones().basis()
                || !structure.boundaries().contains(value.boundary())
                || *value.end_exclusive() as usize > original.phones().tokens().len()
            {
                return Err(FrequencyGestureRefusal::Phrase);
            }
        }
        SpeechAcousticScope::Utterance(value) => carrier(original, value.value(), anchor)?,
    }
    Ok(())
}
