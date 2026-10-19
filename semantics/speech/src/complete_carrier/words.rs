//! Bounded word/phoneme relation admission over the existing owner contracts.
use super::*;
use crate::{
    reference_admission::{resolve_phoneme, ResolvedPhoneme},
    text_admission::{resolve_text, ResolvedText},
};
pub(super) struct WordAdmissions<'a> {
    pub text: Vec<ResolvedText<'a>>,
    pub phonemes: Vec<ResolvedPhoneme<'a>>,
    pub omitted: Vec<ResolvedText<'a>>,
}
pub(super) fn prepare<'a>(
    original: &'a PreparedSpeechCorrespondence<'_, '_>,
    lexical: &'a PreparedLexicalTape,
    structure: &'a PreparedSyllableStructure<'_>,
    relations: &'a [TranslationCorrespondence],
) -> Result<WordAdmissions<'a>, CompleteCarrierRefusal> {
    use CompleteCarrierRefusal::*;
    if relations.is_empty() || relations.len() > 64 {
        return Err(Capacity);
    }
    let mut source_words = alloc::vec![None;original.phonemes().tokens().len()];
    let mut lexical_coverage = alloc::vec![false; lexical.tape().tokens().len()];
    let mut text = Vec::new();
    let mut phonemes = Vec::new();
    let mut omitted = Vec::new();
    for relation in relations {
        if let TranslationCorrespondence::Omitted(group) = relation {
            // Punctuation remains in the exact lexical/parser basis. An
            // explicit source-only relation records why it has no phonemes;
            // it neither invents a pause nor removes a parser token.
            for source in group.sources().as_slice() {
                let resolved = resolve_text(source, original.text()).map_err(|_| WordRelation)?;
                let LanguageSegmentRef::Text(span) = source else {
                    return Err(WordRelation);
                };
                let index = lexical
                    .tape()
                    .tokens()
                    .iter()
                    .position(|token| {
                        matches!(
                            token.category(),
                            conduit_language::LinguisticTokenCategory::Punctuation
                        ) && *token.span().start() == u64::from(*span.range().start())
                            && *token.span().end() == u64::from(*span.range().end())
                    })
                    .ok_or(WordRelation)?;
                if !matches!(span.kind(), LanguageTextSegmentKind::Grapheme)
                    || core::mem::replace(&mut lexical_coverage[index], true)
                {
                    return Err(WordRelation);
                }
                omitted.push(resolved);
            }
            continue;
        }
        let TranslationCorrespondence::Aligned(group) = relation else {
            return Err(UnsupportedWordRelation);
        };
        if group.sources().len() != 1 {
            return Err(WordRelation);
        }
        let word = &group.sources()[0];
        text.push(resolve_text(word, original.text()).map_err(|_| WordRelation)?);
        let LanguageSegmentRef::Text(span) = word else {
            return Err(WordRelation);
        };
        if !matches!(span.kind(), LanguageTextSegmentKind::Word)
            || !lexical.tape().tokens().as_slice().iter().any(|token| {
                matches!(
                    token.category(),
                    conduit_language::LinguisticTokenCategory::Word
                ) && *token.span().start() == u64::from(*span.range().start())
                    && *token.span().end() == u64::from(*span.range().end())
            })
        {
            return Err(WordRelation);
        }
        for (index, token) in lexical.tape().tokens().as_slice().iter().enumerate() {
            if *token.span().start() == u64::from(*span.range().start())
                && *token.span().end() == u64::from(*span.range().end())
            {
                lexical_coverage[index] = true;
            }
        }
        for target in group.targets().as_slice() {
            let resolved =
                resolve_phoneme(target, original.phonemes()).map_err(|_| WordRelation)?;
            let ordinal = *resolved.checked().basis_match().reference().ordinal() as usize;
            let slot = source_words.get_mut(ordinal).ok_or(WordRelation)?;
            if slot.replace(word).is_some() {
                return Err(WordRelation);
            }
            phonemes.push(resolved);
        }
    }
    if source_words.iter().any(Option::is_none) || lexical_coverage.iter().any(|covered| !covered) {
        return Err(WordRelation);
    }
    for group in original.alignment().correspondences().as_slice() {
        let SpeechRealizationCorrespondence::Aligned(group) = group else {
            return Err(UnsupportedWordRelation);
        };
        let mut words = Vec::new();
        for phoneme in group.phonemes().as_slice() {
            let word = source_words
                .get(*phoneme.ordinal() as usize)
                .and_then(|w| *w)
                .ok_or(WordRelation)?;
            if !words.contains(word) {
                words.push(word.clone())
            }
        }
        for phone in group.phones().as_slice() {
            let event = original
                .intent()
                .events()
                .as_slice()
                .iter()
                .find_map(|e| match e {
                    SpeechUtteranceIntentEvent::Segment(v) if v.occurrence() == phone => Some(v),
                    _ => None,
                })
                .ok_or(WordRelation)?;
            if event.sources().as_slice() != words.as_slice() {
                return Err(WordRelation);
            }
        }
    }
    let mut phone_coverage = alloc::vec![false; original.phones().tokens().len()];
    for source in structure.sources() {
        for phone in source.syllable().phones().as_slice() {
            let ordinal = usize::try_from(*phone.ordinal()).map_err(|_| WordRelation)?;
            let covered = phone_coverage.get_mut(ordinal).ok_or(WordRelation)?;
            if core::mem::replace(covered, true) {
                return Err(WordRelation);
            }
            let event = original
                .intent()
                .events()
                .as_slice()
                .iter()
                .find_map(|e| match e {
                    SpeechUtteranceIntentEvent::Segment(v) if v.occurrence() == phone => Some(v),
                    _ => None,
                })
                .ok_or(WordRelation)?;
            let [LanguageSegmentRef::Text(event_word)] = event.sources().as_slice() else {
                return Err(WordRelation);
            };
            let word = source.word();
            if event_word.kind() != word.kind()
                || event_word.language() != word.language()
                || event_word.range() != word.range()
                || event_word.revision_id() != word.revision_id()
                || event_word.text_id() != word.text_id()
            {
                return Err(WordRelation);
            }
        }
    }
    if phone_coverage.iter().any(|covered| !covered) {
        return Err(WordRelation);
    }
    Ok(WordAdmissions {
        text,
        phonemes,
        omitted,
    })
}
