//! Finite deterministic scalar token reconstruction with reviewed lexical data.
//! This prepares inspectable material; it does not parse syntax or claim accuracy.
//! Unicode alphanumeric runs are lexical units under this reconstruction profile,
//! including contiguous CJK runs. This is not language-specific word segmentation.
//! Unlisted whole surfaces retain empty alternatives; their morphology is unknown.
use crate::*;
use alloc::{string::String, vec::Vec};
use conduit_plot::rust_binding::{BoundedSequence, NativeBindingRefusal};

#[derive(Debug)]
pub enum LexicalRefusal {
    Language,
    Profile,
    Prior,
    DuplicateEntry,
    TokenBound,
    TokenBytes,
    Revision(TextRevisionRefusal),
    Native(NativeBindingRefusal),
}

/// Fields remain private so a caller cannot manufacture previous correspondence.
pub struct PreparedLexicalTape {
    tape: LanguageLexicalTape,
}
impl PreparedLexicalTape {
    pub fn tape(&self) -> &LanguageLexicalTape {
        &self.tape
    }
}

/// Borrow the original producer's exact correspondence rule. Surface/span
/// equality is required under that producer's declared stable scalar prefix.
fn prior_occurrence<'a>(
    previous: Option<&'a PreparedLexicalTape>,
    start: u64,
    end: u64,
    surface: &str,
) -> Option<&'a LinguisticTokenIdentity> {
    let old = previous?;
    let stable = u64::from(old.tape.source().stable_prefix().unwrap_or(0));
    old.tape
        .tokens()
        .iter()
        .find(|token| {
            *token.span().start() == start
                && *token.span().end() == end
                && end <= stable
                && token.surface() == surface
                && matches!(token.completeness(), LanguageLexicalCompleteness::Complete)
        })
        .map(|token| token.identity())
}
/// Allocation-free revalidation against the Session's actual previous producer.
/// A separately produced tape with equal prior IDs is insufficient: every
/// original text frontier and full token correspondence must match this owner.
pub fn validate_prepared_lexical_successor(
    previous: Option<&PreparedLexicalTape>,
    candidate: &PreparedLexicalTape,
) -> Result<(), LexicalRefusal> {
    if previous.is_some_and(|old| old.tape.profile() != candidate.tape.profile()) {
        return Err(LexicalRefusal::Profile);
    }
    validate_text_revision(
        previous.map(|old| old.tape.source()),
        candidate.tape.source(),
        0,
        4096,
    )
    .map_err(LexicalRefusal::Revision)?;
    for token in candidate.tape.tokens().iter() {
        let expected = prior_occurrence(
            previous,
            *token.span().start(),
            *token.span().end(),
            token.surface(),
        );
        if token.prior_occurrence().as_ref() != expected {
            return Err(LexicalRefusal::Prior);
        }
    }
    Ok(())
}

pub fn prepare_lexical_tape(
    source: &LanguageTextRevision,
    profile: &LanguageLexicalProfile,
    previous: Option<&PreparedLexicalTape>,
) -> Result<PreparedLexicalTape, LexicalRefusal> {
    use LexicalRefusal::*;
    if source.material().language() != profile.language() {
        return Err(Language);
    }
    for (index, entry) in profile.entries().iter().enumerate() {
        if profile
            .entries()
            .iter()
            .take(index)
            .any(|other| other.surface() == entry.surface())
        {
            return Err(DuplicateEntry);
        }
    }
    if let Some(old) = previous {
        if old.tape.profile() != profile {
            return Err(Profile);
        }
    } else if source.prior().is_some() {
        return Err(Prior);
    }
    validate_text_revision(previous.map(|old| old.tape.source()), source, 0, 4096)
        .map_err(Revision)?;
    let chars: Vec<_> = source.material().text().chars().collect();
    let mut tokens = Vec::new();
    let mut cursor = 0;
    while cursor < chars.len() {
        if chars[cursor].is_whitespace() {
            cursor += 1;
            continue;
        }
        if tokens.len() == 128 {
            return Err(TokenBound);
        }
        let start = cursor;
        let word = chars[cursor].is_alphanumeric();
        cursor += 1;
        if word {
            while cursor < chars.len()
                && (chars[cursor].is_alphanumeric()
                    || (chars[cursor] == '\''
                        && cursor + 1 < chars.len()
                        && chars[cursor + 1].is_alphanumeric()))
            {
                cursor += 1;
            }
        }
        let surface: String = chars[start..cursor].iter().collect();
        if surface.len() > 256 {
            return Err(TokenBytes);
        }
        let partial = word
            && cursor == chars.len()
            && matches!(source.finality(), LanguageTextFinality::Partial);
        let candidates = if partial {
            BoundedSequence::new()
        } else {
            profile
                .entries()
                .iter()
                .find(|entry| entry.surface() == &surface)
                .map(|entry| entry.candidates().clone())
                .unwrap_or_default()
        };
        let prior_occurrence =
            prior_occurrence(previous, start as u64, cursor as u64, &surface).cloned();
        let identity = LinguisticTokenIdentity::new(
            tokens.len() as u64,
            source.material().identity().clone(),
            source.material().revision().clone(),
        )
        .map_err(Native)?;
        let span = TextSpan::new(
            LinguisticOffsetBasis::UnicodeScalar,
            cursor as u64,
            start as u64,
            source.material().identity().clone(),
            source.material().revision().clone(),
        )
        .map_err(Native)?;
        let token = LanguageLexicalToken::new(
            candidates,
            if word {
                LinguisticTokenCategory::Word
            } else {
                LinguisticTokenCategory::Punctuation
            },
            if partial {
                LanguageLexicalCompleteness::TrailingPartial
            } else {
                LanguageLexicalCompleteness::Complete
            },
            identity,
            prior_occurrence,
            span,
            surface,
        )
        .map_err(Native)?;
        tokens.push(token);
    }
    let tape = LanguageLexicalTape::new(
        profile.clone(),
        source.clone(),
        BoundedSequence::try_from_iter(tokens).map_err(|_| TokenBound)?,
    )
    .map_err(Native)?;
    Ok(PreparedLexicalTape { tape })
}

/// Lexical schemas are independent of shared identity imports.
pub fn lexical_types() -> Vec<(&'static str, conduit_core::StructuredInfoType)> {
    alloc::vec![
        (
            "LanguageLexicalPos",
            LanguageLexicalPos::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageLexicalCandidate",
            LanguageLexicalCandidate::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageLexicalEntry",
            LanguageLexicalEntry::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageLexicalProfile",
            LanguageLexicalProfile::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageLexicalCompleteness",
            LanguageLexicalCompleteness::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageLexicalToken",
            LanguageLexicalToken::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageLexicalTape",
            LanguageLexicalTape::semantic_type().expect("checked Language Type")
        ),
    ]
}
