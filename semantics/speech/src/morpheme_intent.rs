//! An explicit morphology component retaining original text and pronunciation.
use crate::semantic::*;
use alloc::vec::Vec;
use conduit_plot::rust_binding::NativeBindingRefusal;
#[derive(Debug)]
pub enum MorphemeIntentRefusal {
    Native(NativeBindingRefusal),
    Text(crate::text_admission::TextReferenceRefusal),
    Local(crate::admission::LocalSemanticRefusal),
    SourceMaterialMismatch,
    Representation,
}
pub struct PreparedMorphemeIntent<'a> {
    intent: &'a SpeechUtteranceIntent,
    phonemes: &'a SpeechPhonemeSequence,
    morpheme: &'a SpeechPlannedMorphemeIntent,
    text: Option<&'a LanguageText>,
    basis: SpeechMorphemePronunciationBasis,
    members: Vec<SpeechMorphemePronunciationMember>,
    references: Vec<SpeechSequenceReferenceMatch>,
    order: Vec<SpeechMorphemePronunciationOrder>,
    source_match: Option<LanguageTextReferenceMatch>,
    surface_match: Option<SpeechMorphemeSurfaceMatch>,
}
impl<'a> PreparedMorphemeIntent<'a> {
    pub fn intent(&self) -> &'a SpeechUtteranceIntent {
        self.intent
    }
    pub fn phonemes(&self) -> &'a SpeechPhonemeSequence {
        self.phonemes
    }
    pub fn morpheme(&self) -> &'a SpeechPlannedMorphemeIntent {
        self.morpheme
    }
    pub fn text(&self) -> Option<&'a LanguageText> {
        self.text
    }
    pub fn basis(&self) -> &SpeechMorphemePronunciationBasis {
        &self.basis
    }
    pub fn members(&self) -> &[SpeechMorphemePronunciationMember] {
        &self.members
    }
    pub fn references(&self) -> &[SpeechSequenceReferenceMatch] {
        &self.references
    }
    pub fn order(&self) -> &[SpeechMorphemePronunciationOrder] {
        &self.order
    }
    pub fn source_match(&self) -> Option<&LanguageTextReferenceMatch> {
        self.source_match.as_ref()
    }
    pub fn surface_match(&self) -> Option<&SpeechMorphemeSurfaceMatch> {
        self.surface_match.as_ref()
    }
    pub fn prepare(
        intent: &'a SpeechUtteranceIntent,
        phonemes: &'a SpeechPhonemeSequence,
        morpheme: &'a SpeechPlannedMorphemeIntent,
        text: Option<&'a LanguageText>,
    ) -> Result<Self, MorphemeIntentRefusal> {
        use MorphemeIntentRefusal::*;
        crate::admission::validate_feature_bundle(morpheme.features()).map_err(Local)?;
        let basis = SpeechMorphemePronunciationBasis::new(
            intent.clone(),
            morpheme.clone(),
            phonemes.clone(),
        )
        .map_err(Native)?;
        let count = u32::try_from(phonemes.tokens().len()).map_err(|_| Representation)?;
        let mut members = Vec::new();
        let mut references = Vec::new();
        let mut order = Vec::new();
        for (index, occurrence) in morpheme.pronunciation().as_slice().iter().enumerate() {
            members.push(
                SpeechMorphemePronunciationMember::new(
                    index as u64,
                    morpheme.clone(),
                    occurrence.clone(),
                )
                .map_err(Native)?,
            );
            let matched =
                SpeechSequenceBasisMatch::new(phonemes.basis().clone(), occurrence.clone())
                    .map_err(Native)?;
            references.push(SpeechSequenceReferenceMatch::new(matched, count).map_err(Native)?);
            if index > 0 {
                order.push(
                    SpeechMorphemePronunciationOrder::new(
                        morpheme.pronunciation()[index - 1].clone(),
                        occurrence.clone(),
                    )
                    .map_err(Native)?,
                );
            }
        }
        let (source_match, surface_match) = match (morpheme.source(), text) {
            (None, None) => (None, None),
            (Some(source), Some(material)) => {
                let original = source.occurrence();
                let reference = LanguageSegmentRef::text(
                    *original.kind(),
                    original.language().clone(),
                    original.range().clone(),
                    original.revision_id().clone(),
                    original.text_id().clone(),
                )
                .map_err(Native)?;
                let resolved =
                    crate::text_admission::resolve_text(&reference, material).map_err(Text)?;
                let surface = SpeechMorphemeSurfaceMatch::new(
                    resolved.text().into(),
                    morpheme.surface().clone(),
                )
                .map_err(Native)?;
                (Some(resolved.checked().clone()), Some(surface))
            }
            _ => return Err(SourceMaterialMismatch),
        };
        Ok(Self {
            intent,
            phonemes,
            morpheme,
            text,
            basis,
            members,
            references,
            order,
            source_match,
            surface_match,
        })
    }
}
