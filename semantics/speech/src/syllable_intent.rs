//! Explicit target grouping bound to the original utterance and phone sequence.
//! This allocating preparation does not infer syllables or admit playback.
use crate::semantic::*;
use alloc::vec::Vec;
use conduit_plot::rust_binding::NativeBindingRefusal;

#[derive(Debug)]
pub enum SyllableIntentRefusal {
    Native(NativeBindingRefusal),
    Representation,
    Local(crate::admission::LocalSemanticRefusal),
}
/// Retains the whole original carriers and every executed Native law receipt.
/// This is one component of the existing intent, not a second utterance owner.
pub struct PreparedSyllableIntent<'a> {
    intent: &'a SpeechUtteranceIntent,
    phones: &'a SpeechPhoneSequence,
    syllable: &'a SpeechPlannedSyllableIntent,
    intent_basis: SpeechSyllableIntentBasis,
    sequence_basis: SpeechSyllableSequenceBasis,
    members: Vec<SpeechSyllableMemberMatch>,
    references: Vec<SpeechSequenceReferenceMatch>,
    order: Vec<SpeechSyllableMemberOrder>,
}
impl<'a> PreparedSyllableIntent<'a> {
    pub fn intent(&self) -> &'a SpeechUtteranceIntent {
        self.intent
    }
    pub fn phones(&self) -> &'a SpeechPhoneSequence {
        self.phones
    }
    pub fn syllable(&self) -> &'a SpeechPlannedSyllableIntent {
        self.syllable
    }
    pub fn intent_basis(&self) -> &SpeechSyllableIntentBasis {
        &self.intent_basis
    }
    pub fn sequence_basis(&self) -> &SpeechSyllableSequenceBasis {
        &self.sequence_basis
    }
    pub fn members(&self) -> &[SpeechSyllableMemberMatch] {
        &self.members
    }
    pub fn references(&self) -> &[SpeechSequenceReferenceMatch] {
        &self.references
    }
    pub fn order(&self) -> &[SpeechSyllableMemberOrder] {
        &self.order
    }
    pub fn prepare(
        intent: &'a SpeechUtteranceIntent,
        phones: &'a SpeechPhoneSequence,
        syllable: &'a SpeechPlannedSyllableIntent,
    ) -> Result<Self, SyllableIntentRefusal> {
        use SyllableIntentRefusal::*;
        if let Some(span) = syllable.span() {
            crate::admission::validate_segment_span(span).map_err(Local)?;
        }
        let intent_basis =
            SpeechSyllableIntentBasis::new(intent.clone(), syllable.clone()).map_err(Native)?;
        let sequence_basis =
            SpeechSyllableSequenceBasis::new(phones.clone(), syllable.clone()).map_err(Native)?;
        let count = u32::try_from(phones.tokens().len()).map_err(|_| Representation)?;
        let mut members = Vec::new();
        let mut references = Vec::new();
        let mut order = Vec::new();
        for (index, occurrence) in syllable.phones().as_slice().iter().enumerate() {
            members.push(
                SpeechSyllableMemberMatch::new(index as u64, occurrence.clone(), syllable.clone())
                    .map_err(Native)?,
            );
            let basis = SpeechSequenceBasisMatch::new(phones.basis().clone(), occurrence.clone())
                .map_err(Native)?;
            references.push(SpeechSequenceReferenceMatch::new(basis, count).map_err(Native)?);
            if index > 0 {
                order.push(
                    SpeechSyllableMemberOrder::new(
                        syllable.phones()[index - 1].clone(),
                        occurrence.clone(),
                    )
                    .map_err(Native)?,
                );
            }
        }
        Ok(Self {
            intent,
            phones,
            syllable,
            intent_basis,
            sequence_basis,
            members,
            references,
            order,
        })
    }
}
