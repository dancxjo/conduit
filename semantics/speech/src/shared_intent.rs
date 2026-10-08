//! One prepared owner over the original utterance and ordinary typed components.
//! This is allocating semantic preparation, not permission to commit or play.
use crate::{
    correspondence::*, intent_context::*, morpheme_intent::*, semantic::*, syllable_intent::*,
};
use alloc::vec::Vec;
use conduit_plot::rust_binding::NativeBindingRefusal;
pub struct SpeechIntentComponents<'a> {
    pub context: &'a SpeechUtteranceIntentContext,
    pub intended_text: Option<&'a LanguageText>,
    pub phonemes: &'a SpeechPhonemeSequence,
    pub phones: &'a SpeechPhoneSequence,
    pub morphemes: &'a [SpeechPlannedMorphemeIntent],
    pub morpheme_texts: &'a [Option<&'a LanguageText>],
    pub syllables: &'a [SpeechPlannedSyllableIntent],
    pub correspondences: &'a [SpeechPhonemePhoneCorrespondence],
}
#[derive(Debug)]
pub enum SharedIntentRefusal {
    Native(NativeBindingRefusal),
    Representation,
    Context(ContextRefusal),
    Morpheme(MorphemeIntentRefusal),
    Syllable(SyllableIntentRefusal),
    Correspondence(CorrespondenceRefusal),
}
pub struct PreparedSpeechUtteranceIntent<'a> {
    original: &'a SpeechUtteranceIntent,
    components: SpeechIntentComponents<'a>,
    counts: SpeechIntentComponentCounts,
    context: PreparedUtteranceIntentContext<'a>,
    sequence_basis: SpeechPhonemePhoneCorrespondenceBasis,
    morphemes: Vec<PreparedMorphemeIntent<'a>>,
    syllables: Vec<PreparedSyllableIntent<'a>>,
    correspondences: Vec<PreparedPhonemePhoneCorrespondence<'a>>,
}
impl<'a> PreparedSpeechUtteranceIntent<'a> {
    pub fn original(&self) -> &'a SpeechUtteranceIntent {
        self.original
    }
    pub fn components(&self) -> &SpeechIntentComponents<'a> {
        &self.components
    }
    pub fn counts(&self) -> &SpeechIntentComponentCounts {
        &self.counts
    }
    pub fn context(&self) -> &PreparedUtteranceIntentContext<'a> {
        &self.context
    }
    pub fn sequence_basis(&self) -> &SpeechPhonemePhoneCorrespondenceBasis {
        &self.sequence_basis
    }
    pub fn morphemes(&self) -> &[PreparedMorphemeIntent<'a>] {
        &self.morphemes
    }
    pub fn syllables(&self) -> &[PreparedSyllableIntent<'a>] {
        &self.syllables
    }
    pub fn correspondences(&self) -> &[PreparedPhonemePhoneCorrespondence<'a>] {
        &self.correspondences
    }
    pub fn prepare(
        original: &'a SpeechUtteranceIntent,
        components: SpeechIntentComponents<'a>,
    ) -> Result<Self, SharedIntentRefusal> {
        use SharedIntentRefusal::*;
        let count = |value: usize| u64::try_from(value).map_err(|_| Representation);
        let counts = SpeechIntentComponentCounts::new(
            count(components.correspondences.len())?,
            count(components.morpheme_texts.len())?,
            count(components.morphemes.len())?,
            count(components.syllables.len())?,
        )
        .map_err(Native)?;
        let context = PreparedUtteranceIntentContext::prepare(
            original,
            components.context,
            components.intended_text,
        )
        .map_err(Context)?;
        let sequence_basis = SpeechPhonemePhoneCorrespondenceBasis::new(
            original.clone(),
            components.phonemes.clone(),
            components.phones.clone(),
        )
        .map_err(Native)?;
        let mut morphemes = Vec::new();
        let mut syllables = Vec::new();
        let mut correspondences = Vec::new();
        for (morpheme, text) in components.morphemes.iter().zip(components.morpheme_texts) {
            morphemes.push(
                PreparedMorphemeIntent::prepare(original, components.phonemes, morpheme, *text)
                    .map_err(Morpheme)?,
            );
        }
        for syllable in components.syllables {
            syllables.push(
                PreparedSyllableIntent::prepare(original, components.phones, syllable)
                    .map_err(Syllable)?,
            );
        }
        for correspondence in components.correspondences {
            correspondences.push(
                PreparedPhonemePhoneCorrespondence::prepare(
                    original,
                    components.phonemes,
                    components.phones,
                    correspondence,
                )
                .map_err(Correspondence)?,
            );
        }
        Ok(Self {
            original,
            components,
            counts,
            context,
            sequence_basis,
            morphemes,
            syllables,
            correspondences,
        })
    }
}
