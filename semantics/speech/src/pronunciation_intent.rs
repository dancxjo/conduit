//! Mechanical native intent composition from an immutable checked pronunciation.
//! Policy and phone selection already ran in the owning Language/Speech Plots.
use crate::{lexical_pronunciation::PreparedPronunciation, semantic::*};
use conduit_plot::rust_binding::{BoundedSequence, NativeBindingRefusal};
#[derive(Debug)]
pub enum PronunciationIntentRefusal {
    Language,
    Origin,
    Source,
    Native(NativeBindingRefusal),
}
pub struct PreparedPronunciationIntent<'a, 'basis> {
    pronunciation: &'a PreparedPronunciation<'basis>,
    intent: SpeechUtteranceIntent,
}
impl<'a, 'basis> PreparedPronunciationIntent<'a, 'basis> {
    pub fn pronunciation(&self) -> &'a PreparedPronunciation<'basis> {
        self.pronunciation
    }
    pub fn intent(&self) -> &SpeechUtteranceIntent {
        &self.intent
    }
}
/// One prepared word becomes one bounded immutable phone sequence. The supplied
/// phonetic occurrence names a fresh sequence at ordinal zero. It does not grant
/// playback authority or claim any queued/played frames.
pub fn prepare_pronunciation_intent<'a, 'basis>(
    pronunciation: &'a PreparedPronunciation<'basis>,
    origin: &LanguageSpeechTokenRef,
    prosody: &SpeechSegmentProsodyIntent,
    provenance: &SpeechEvidenceProvenance,
) -> Result<PreparedPronunciationIntent<'a, 'basis>, PronunciationIntentRefusal> {
    use PronunciationIntentRefusal::*;
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
        .phones()
        .iter()
        .enumerate()
        .map(|(ordinal, phone)| {
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
                PhoneSpecification::known(phone.phone().clone()).map_err(Native)?,
                PhonemeSpecification::unspecified(),
                prosody.clone(),
                provenance.clone(),
                sources.clone(),
                StressSpecification::known(*phone.stress()).map_err(Native)?,
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
    Ok(PreparedPronunciationIntent {
        pronunciation,
        intent,
    })
}
