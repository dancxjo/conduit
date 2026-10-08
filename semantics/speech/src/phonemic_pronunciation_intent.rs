//! Mechanical common intent composition from an original checked phonemic pronunciation.
//! Policy and phoneme selection already ran in the owning Language/Speech Plots.
use crate::{phonemic_pronunciation::PreparedPhonemicPronunciation, semantic::*};
use conduit_plot::rust_binding::{BoundedSequence, NativeBindingRefusal};
#[derive(Debug)]
pub enum PhonemicPronunciationIntentRefusal {
    Language,
    Origin,
    Source,
    Native(NativeBindingRefusal),
}
pub struct PreparedPhonemicPronunciationIntent<'a, 'basis> {
    pronunciation: &'a PreparedPhonemicPronunciation<'basis>,
    intent: SpeechUtteranceIntent,
}
impl<'a, 'basis> PreparedPhonemicPronunciationIntent<'a, 'basis> {
    pub fn pronunciation(&self) -> &'a PreparedPhonemicPronunciation<'basis> {
        self.pronunciation
    }
    pub fn intent(&self) -> &SpeechUtteranceIntent {
        &self.intent
    }
}
/// One prepared word becomes one bounded immutable phoneme sequence. The supplied
/// phonetic occurrence names a fresh sequence at ordinal zero. It does not grant
/// playback authority or claim any queued/played frames.
pub fn prepare_phonemic_pronunciation_intent<'a, 'basis>(
    pronunciation: &'a PreparedPhonemicPronunciation<'basis>,
    origin: &LanguageSpeechTokenRef,
    prosody: &SpeechSegmentProsodyIntent,
    provenance: &SpeechEvidenceProvenance,
) -> Result<PreparedPhonemicPronunciationIntent<'a, 'basis>, PhonemicPronunciationIntentRefusal> {
    use PhonemicPronunciationIntentRefusal::*;
    let request = pronunciation.selection().request();
    if origin.language() != request.source().material().language() {
        return Err(Language);
    }
    if *origin.ordinal() != 0 {
        return Err(Origin);
    }
    let reference = conduit_language::language_source_occurrence(
        request.source().material(),
        request.token().span(),
        LanguageTextSegmentKind::Word,
    )
    .map_err(|_| Source)?;
    let sources = BoundedSequence::try_from_iter([LanguageSegmentRef::text(
        *reference.kind(),
        reference.language().clone(),
        reference.range().clone(),
        reference.revision_id().clone(),
        reference.text_id().clone(),
    )
    .map_err(Native)?])
    .map_err(|_| Source)?;
    let events = pronunciation
        .result()
        .phonemes()
        .iter()
        .enumerate()
        .map(|(ordinal, phoneme)| {
            let occurrence = LanguageSpeechTokenRef::new(
                origin.inventory_id().clone(),
                origin.language().clone(),
                ordinal as u32,
                origin.revision_id().clone(),
                origin.sequence_id().clone(),
                origin.utterance_id().clone(),
            )
            .map_err(Native)?;
            SpeechUtteranceIntentEvent::segment(
                occurrence,
                PhoneSpecification::unspecified(),
                PhonemeSpecification::known(phoneme.phoneme().clone()).map_err(Native)?,
                prosody.clone(),
                provenance.clone(),
                sources.clone(),
                StressSpecification::known(*phoneme.stress()).map_err(Native)?,
                SpeechPositionSpecification::unspecified(),
            )
            .map_err(Native)
        })
        .collect::<Result<alloc::vec::Vec<_>, _>>()?;
    let intent = SpeechUtteranceIntent::new(
        BoundedSequence::try_from_iter(events).map_err(|_| Source)?,
        origin.inventory_id().clone(),
        origin.language().clone(),
        provenance.clone(),
        origin.revision_id().clone(),
        origin.utterance_id().clone(),
    )
    .map_err(Native)?;
    Ok(PreparedPhonemicPronunciationIntent {
        pronunciation,
        intent,
    })
}
