//! Mechanical intent composition retaining an independent early lexical fact.
//! The supplied prosody is explicit; this preparation grants no playback authority.
use crate::pronunciation_intent::PronunciationIntentRefusal;
use crate::{semantic::*, stable_lexical_pronunciation::PreparedStableLexicalPronunciation};
use conduit_plot::rust_binding::BoundedSequence;
pub struct PreparedStableLexicalIntent<'a, 'basis, 'fact> {
    pronunciation: &'a PreparedStableLexicalPronunciation<'basis, 'fact>,
    intent: SpeechUtteranceIntent,
}
impl<'a, 'basis, 'fact> PreparedStableLexicalIntent<'a, 'basis, 'fact> {
    pub fn pronunciation(&self) -> &'a PreparedStableLexicalPronunciation<'basis, 'fact> {
        self.pronunciation
    }
    pub fn intent(&self) -> &SpeechUtteranceIntent {
        &self.intent
    }
}
/// One prepared word becomes one bounded immutable phone sequence. The supplied
/// phonetic occurrence names a fresh sequence at ordinal zero. It does not grant
/// playback authority or claim any queued/played frames.
pub fn prepare_stable_lexical_intent<'a, 'basis, 'fact>(
    pronunciation: &'a PreparedStableLexicalPronunciation<'basis, 'fact>,
    origin: &LanguageSpeechTokenRef,
    prosody: &SpeechSegmentProsodyIntent,
    provenance: &SpeechEvidenceProvenance,
) -> Result<PreparedStableLexicalIntent<'a, 'basis, 'fact>, PronunciationIntentRefusal> {
    use PronunciationIntentRefusal::*;
    let selection = pronunciation.selection();
    let source = selection.lexical().tape().source();
    let token = selection
        .lexical()
        .tape()
        .tokens()
        .iter()
        .nth(*selection.fact().query().dependent() as usize)
        .ok_or(Source)?;
    if origin.language() != source.material().language() {
        return Err(Language);
    }
    if *origin.ordinal() != 0 {
        return Err(Origin);
    }
    let reference = conduit_language::language_source_occurrence(
        source.material(),
        token.span(),
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
    Ok(PreparedStableLexicalIntent {
        pronunciation,
        intent,
    })
}
