//! Bounded preparation of explicit linguistic structure over original carriers.
use crate::{ipa_phonetic::phonetic_from_ipa, semantic::*, syllable_intent::*};
use alloc::vec::Vec;
use conduit_plot::rust_binding::NativeBindingRefusal;

#[derive(Debug)]
pub enum StructureRefusal {
    Native(NativeBindingRefusal),
    Syllable(SyllableIntentRefusal),
    Ipa,
    Capacity,
    ContradictoryMembership,
    Position,
}
pub struct PreparedSyllableStructure<'a> {
    intent: &'a SpeechUtteranceIntent,
    phones: &'a SpeechPhoneSequence,
    text: &'a LanguageText,
    sources: &'a [SpeechSyllableSource],
    boundaries: &'a [SpeechPositionedBoundary],
    syllables: Vec<PreparedSyllableIntent<'a>>,
    text_receipts: Vec<LanguageTextReferenceMatch>,
    ipa_receipts: Vec<SpeechSyllableIpaMember>,
    boundary_order: Vec<SpeechBoundaryPositionOrder>,
}
impl<'a> PreparedSyllableStructure<'a> {
    pub fn intent(&self) -> &'a SpeechUtteranceIntent {
        self.intent
    }
    pub fn phones(&self) -> &'a SpeechPhoneSequence {
        self.phones
    }
    pub fn text(&self) -> &'a LanguageText {
        self.text
    }
    pub fn sources(&self) -> &'a [SpeechSyllableSource] {
        self.sources
    }
    pub fn boundaries(&self) -> &'a [SpeechPositionedBoundary] {
        self.boundaries
    }
    pub fn syllables(&self) -> &[PreparedSyllableIntent<'a>] {
        &self.syllables
    }
    pub fn text_receipts(&self) -> &[LanguageTextReferenceMatch] {
        &self.text_receipts
    }
    pub fn ipa_receipts(&self) -> &[SpeechSyllableIpaMember] {
        &self.ipa_receipts
    }
    pub fn boundary_order(&self) -> &[SpeechBoundaryPositionOrder] {
        &self.boundary_order
    }

    pub fn prepare(
        intent: &'a SpeechUtteranceIntent,
        phones: &'a SpeechPhoneSequence,
        text: &'a LanguageText,
        sources: &'a [SpeechSyllableSource],
        boundaries: &'a [SpeechPositionedBoundary],
    ) -> Result<Self, StructureRefusal> {
        use StructureRefusal::*;
        if sources.is_empty() || sources.len() > 64 || boundaries.len() > 256 {
            return Err(Capacity);
        }
        let scalar_count = u32::try_from(text.text().chars().count()).map_err(|_| Capacity)?;
        let mut prepared = Self {
            intent,
            phones,
            text,
            sources,
            boundaries,
            syllables: Vec::new(),
            text_receipts: Vec::new(),
            ipa_receipts: Vec::new(),
            boundary_order: Vec::new(),
        };
        for (index, source) in sources.iter().enumerate() {
            for earlier in &sources[..index] {
                if earlier.syllable().identity() == source.syllable().identity()
                    || earlier
                        .syllable()
                        .phones()
                        .as_slice()
                        .iter()
                        .any(|phone| source.syllable().phones().as_slice().contains(phone))
                {
                    return Err(ContradictoryMembership);
                }
            }
            let mut last_position = 0;
            let mut nuclei = 0;
            for role in source.syllable().phone_positions().as_slice() {
                let position = match role {
                    SpeechSyllablePosition::Onset => 0,
                    SpeechSyllablePosition::Nucleus => {
                        nuclei += 1;
                        1
                    }
                    SpeechSyllablePosition::Coda => 2,
                };
                if position < last_position {
                    return Err(Position);
                }
                last_position = position;
            }
            if nuclei != 1 || source.syllable().nucleus_index().is_none() {
                return Err(Position);
            }
            prepared.syllables.push(
                PreparedSyllableIntent::prepare(intent, phones, source.syllable())
                    .map_err(Syllable)?,
            );
            prepared.text_receipts.push(
                LanguageTextReferenceMatch::new(text.clone(), source.word().clone(), scalar_count)
                    .map_err(Native)?,
            );
            let reparsed = phonetic_from_ipa(
                source.ipa().original().clone(),
                source.ipa().provenance().clone(),
            )
            .map_err(|_| Ipa)?;
            if reparsed.transcription() != source.ipa() {
                return Err(Ipa);
            }
            let mut previous = None;
            for (member, position) in source.ipa_phone_indices().as_slice().iter().enumerate() {
                if previous.is_some_and(|last| last >= *position) {
                    return Err(Position);
                }
                previous = Some(*position);
                let occurrence = source
                    .ipa()
                    .units()
                    .as_slice()
                    .get(usize::try_from(*position).map_err(|_| Position)?)
                    .ok_or(Position)?;
                prepared.ipa_receipts.push(
                    SpeechSyllableIpaMember::new(member as u64, occurrence.clone(), source.clone())
                        .map_err(Native)?,
                );
            }
        }
        for (index, boundary) in boundaries.iter().enumerate() {
            if boundary.basis() != phones.basis()
                || *boundary.position() as usize > phones.tokens().len()
            {
                return Err(Position);
            }
            prepared.text_receipts.push(
                LanguageTextReferenceMatch::new(
                    text.clone(),
                    boundary.source().clone(),
                    scalar_count,
                )
                .map_err(Native)?,
            );
            if index > 0 {
                prepared.boundary_order.push(
                    SpeechBoundaryPositionOrder::new(
                        boundaries[index - 1].clone(),
                        boundary.clone(),
                    )
                    .map_err(Native)?,
                );
            }
        }
        Ok(prepared)
    }
}
