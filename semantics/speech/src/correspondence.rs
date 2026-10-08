//! Explicit many-to-many grouping retaining all original token material.
use crate::semantic::*;
use alloc::vec::Vec;
use conduit_plot::rust_binding::{BoundedSequence, NativeBindingRefusal};
#[derive(Debug)]
pub enum CorrespondenceRefusal {
    Native(NativeBindingRefusal),
    Representation,
}
pub struct PreparedPhonemePhoneCorrespondence<'a> {
    intent: &'a SpeechUtteranceIntent,
    phonemes: &'a SpeechPhonemeSequence,
    phones: &'a SpeechPhoneSequence,
    correspondence: &'a SpeechPhonemePhoneCorrespondence,
    basis: SpeechPhonemePhoneCorrespondenceBasis,
    phoneme_references: Vec<SpeechSequenceReferenceMatch>,
    phone_references: Vec<SpeechSequenceReferenceMatch>,
    order: Vec<SpeechCorrespondenceOccurrenceOrder>,
    realization: Vec<SpeechCorrespondenceRealizationMatch>,
}
impl<'a> PreparedPhonemePhoneCorrespondence<'a> {
    pub fn intent(&self) -> &'a SpeechUtteranceIntent {
        self.intent
    }
    pub fn phonemes(&self) -> &'a SpeechPhonemeSequence {
        self.phonemes
    }
    pub fn phones(&self) -> &'a SpeechPhoneSequence {
        self.phones
    }
    pub fn correspondence(&self) -> &'a SpeechPhonemePhoneCorrespondence {
        self.correspondence
    }
    pub fn basis(&self) -> &SpeechPhonemePhoneCorrespondenceBasis {
        &self.basis
    }
    pub fn phoneme_references(&self) -> &[SpeechSequenceReferenceMatch] {
        &self.phoneme_references
    }
    pub fn phone_references(&self) -> &[SpeechSequenceReferenceMatch] {
        &self.phone_references
    }
    pub fn order(&self) -> &[SpeechCorrespondenceOccurrenceOrder] {
        &self.order
    }
    pub fn realization(&self) -> &[SpeechCorrespondenceRealizationMatch] {
        &self.realization
    }
    pub fn prepare(
        intent: &'a SpeechUtteranceIntent,
        phonemes: &'a SpeechPhonemeSequence,
        phones: &'a SpeechPhoneSequence,
        correspondence: &'a SpeechPhonemePhoneCorrespondence,
    ) -> Result<Self, CorrespondenceRefusal> {
        use CorrespondenceRefusal::*;
        let basis = SpeechPhonemePhoneCorrespondenceBasis::new(
            intent.clone(),
            phonemes.clone(),
            phones.clone(),
        )
        .map_err(Native)?;
        let mut phoneme_references = Vec::new();
        let mut phone_references = Vec::new();
        let mut order = Vec::new();
        let mut realization = Vec::new();
        for (refs, sequence_basis, count, receipts) in [
            (
                correspondence.phonemes().as_slice(),
                phonemes.basis(),
                phonemes.tokens().len(),
                &mut phoneme_references,
            ),
            (
                correspondence.phones().as_slice(),
                phones.basis(),
                phones.tokens().len(),
                &mut phone_references,
            ),
        ] {
            let count = u32::try_from(count).map_err(|_| Representation)?;
            for (index, occurrence) in refs.iter().enumerate() {
                let checked =
                    SpeechSequenceBasisMatch::new(sequence_basis.clone(), occurrence.clone())
                        .map_err(Native)?;
                receipts.push(SpeechSequenceReferenceMatch::new(checked, count).map_err(Native)?);
                if index > 0 {
                    order.push(
                        SpeechCorrespondenceOccurrenceOrder::new(
                            refs[index - 1].clone(),
                            occurrence.clone(),
                        )
                        .map_err(Native)?,
                    );
                }
            }
        }
        // Both supported resolved cases must retain and agree with the
        // original token's realized_as field, including explicit empty omission.
        // Unresolved intent is preserved without choosing a realization.
        if matches!(
            correspondence.kind(),
            SpeechRealizationCorrespondenceKind::Realized
                | SpeechRealizationCorrespondenceKind::Omitted
        ) {
            let selected = BoundedSequence::try_from_iter(
                correspondence
                    .phones()
                    .as_slice()
                    .iter()
                    .map(|r| phones.tokens()[*r.ordinal() as usize].clone()),
            )
            .map_err(|_| Representation)?;
            for occurrence in correspondence.phonemes().as_slice() {
                realization.push(
                    SpeechCorrespondenceRealizationMatch::new(
                        phonemes.tokens()[*occurrence.ordinal() as usize].clone(),
                        selected.clone(),
                    )
                    .map_err(Native)?,
                );
            }
        }
        Ok(Self {
            intent,
            phonemes,
            phones,
            correspondence,
            basis,
            phoneme_references,
            phone_references,
            order,
            realization,
        })
    }
}
