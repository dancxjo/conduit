//! Bounded traversal of the checked English text and pronunciation plots.
//! This decodes UTF-8 and assembles typed values; it contains no spelling rules.
use crate::{generated::*, MAXIMUM_EVENTS};

pub const MAXIMUM_TEXT_BYTES: usize = RENDER_PROFILE.maximum_text_bytes as usize;
pub const MAXIMUM_WORD_BYTES: usize = RENDER_PROFILE.maximum_word_bytes as usize;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextRefusal {
    InputBound,
    WordBound,
    EventBound,
    OutputSpace,
    UnsupportedCharacter { scalar: u32, codepoint: u32 },
    Arithmetic,
}
/// Source-scoped occurrences. The borrowed source is part of this preparation;
/// retained-artifact identities must be resolved before transporting references.
pub struct PronouncedText<'a> {
    source: &'a str,
    events: &'a [VoiceEvent],
}
impl PronouncedText<'_> {
    pub fn source(&self) -> &str {
        self.source
    }
    pub fn events(&self) -> &[VoiceEvent] {
        self.events
    }
}
pub fn pronounce<'a>(
    source: &'a str,
    storage: &'a mut [VoiceEvent],
) -> Result<PronouncedText<'a>, TextRefusal> {
    if source.len() > MAXIMUM_TEXT_BYTES {
        return Err(TextRefusal::InputBound);
    }
    let mut count = 0;
    traverse(source, |_| {
        count += 1;
        if count > MAXIMUM_EVENTS {
            Err(TextRefusal::EventBound)
        } else {
            Ok(())
        }
    })?;
    if storage.len() < count {
        return Err(TextRefusal::OutputSpace);
    }
    let mut index = 0;
    traverse(source, |event| {
        storage[index] = event;
        index += 1;
        Ok(())
    })?;
    Ok(PronouncedText {
        source,
        events: &storage[..count],
    })
}
fn slots(value: EnglishWordPhonemes) -> [EnglishPhonemeSlot; 12] {
    [
        value.phoneme1,
        value.phoneme2,
        value.phoneme3,
        value.phoneme4,
        value.phoneme5,
        value.phoneme6,
        value.phoneme7,
        value.phoneme8,
        value.phoneme9,
        value.phoneme10,
        value.phoneme11,
        value.phoneme12,
    ]
}
fn walk_word(
    word: &[u8],
    mut emit: impl FnMut(
        EnglishPronouncedPhoneme,
        usize,
        usize,
        EnglishPronunciationOrigin,
    ) -> Result<(), TextRefusal>,
) -> Result<(), TextRefusal> {
    let text = core::str::from_utf8(word).map_err(|_| TextRefusal::Arithmetic)?;
    match speech_english_lexicon(text).ok_or(TextRefusal::Arithmetic)? {
        EnglishLexiconDecision::matched(phones) => {
            for slot in slots(phones) {
                if let EnglishPhonemeSlot::present(phone) = slot {
                    emit(phone, 0, word.len(), EnglishPronunciationOrigin::dictionary)?;
                }
            }
        }
        EnglishLexiconDecision::unknown => {
            let mut index = 0;
            while index < word.len() {
                let context = EnglishSpellingContext {
                    previous: index
                        .checked_sub(1)
                        .and_then(|i| word.get(i))
                        .copied()
                        .unwrap_or(0) as i32,
                    current: word[index] as i32,
                    next: word.get(index + 1).copied().unwrap_or(0) as i32,
                    after: word.get(index + 2).copied().unwrap_or(0) as i32,
                    index: index as u32,
                    length: word.len() as u32,
                };
                let EnglishSpellingResult::supported(decision) =
                    speech_english_spelling(context).ok_or(TextRefusal::Arithmetic)?
                else {
                    return Err(TextRefusal::Arithmetic);
                };
                let consumed =
                    usize::try_from(decision.consumed).map_err(|_| TextRefusal::Arithmetic)?;
                let end = index
                    .checked_add(consumed)
                    .filter(|end| *end > index && *end <= word.len())
                    .ok_or(TextRefusal::Arithmetic)?;
                for slot in slots(decision.phonemes) {
                    if let EnglishPhonemeSlot::present(phone) = slot {
                        emit(phone, index, end, EnglishPronunciationOrigin::spelling_rule)?;
                    }
                }
                index = end;
            }
        }
    }
    Ok(())
}
fn word_events(
    word: &[u8],
    start: u32,
    mut emit: impl FnMut(VoiceEvent) -> Result<(), TextRefusal>,
) -> Result<bool, TextRefusal> {
    let mut count = 0_u32;
    walk_word(word, |_, _, _, _| {
        count += 1;
        Ok(())
    })?;
    let mut ordinal = 0;
    walk_word(word, |pronunciation, from, to, origin| {
        let realization = speech_text_realization(EnglishTextSegmentContext {
            pronunciation,
            ordinal,
            count,
        })
        .ok_or(TextRefusal::Arithmetic)?;
        emit(VoiceEvent::pronounced(TextSpeechSegment {
            realization,
            source_scalar_start: start + from as u32,
            source_scalar_end: start + to as u32,
            origin,
        }))?;
        ordinal += 1;
        Ok(())
    })?;
    Ok(count != 0)
}
fn boundary(
    class: EnglishTextClass,
    tail: &mut EnglishTextTail,
    emit: &mut impl FnMut(VoiceEvent) -> Result<(), TextRefusal>,
) -> Result<(), TextRefusal> {
    if let EnglishTextBoundaryDecision::emitted(value) =
        speech_text_boundary(EnglishTextBoundaryContext {
            class,
            prior: *tail,
        })
        .ok_or(TextRefusal::Arithmetic)?
    {
        emit(VoiceEvent::boundary(value.boundary))?;
        *tail = value.tail;
    }
    Ok(())
}
fn traverse(
    source: &str,
    mut emit: impl FnMut(VoiceEvent) -> Result<(), TextRefusal>,
) -> Result<(), TextRefusal> {
    let mut word = [0_u8; MAXIMUM_WORD_BYTES];
    let mut length = 0;
    let mut start = 0;
    let mut tail = EnglishTextTail::empty;
    for (index, character) in source.chars().enumerate() {
        let normalized = speech_lowercase(character as i32).ok_or(TextRefusal::Arithmetic)?;
        let class = speech_text_class(normalized).ok_or(TextRefusal::Arithmetic)?;
        match class {
            EnglishTextClass::letter | EnglishTextClass::apostrophe => {
                if length == MAXIMUM_WORD_BYTES {
                    return Err(TextRefusal::WordBound);
                }
                if length == 0 {
                    start = index as u32;
                }
                word[length] = u8::try_from(normalized).map_err(|_| TextRefusal::Arithmetic)?;
                length += 1;
            }
            EnglishTextClass::unsupported => {
                return Err(TextRefusal::UnsupportedCharacter {
                    scalar: index as u32,
                    codepoint: character as u32,
                })
            }
            class => {
                if length != 0 {
                    if word_events(&word[..length], start, &mut emit)? {
                        tail = EnglishTextTail::segment;
                    }
                    length = 0;
                }
                boundary(class, &mut tail, &mut emit)?;
            }
        }
    }
    if length != 0 && word_events(&word[..length], start, &mut emit)? {
        tail = EnglishTextTail::segment;
    }
    boundary(EnglishTextClass::turn_gap, &mut tail, &mut emit)
}
