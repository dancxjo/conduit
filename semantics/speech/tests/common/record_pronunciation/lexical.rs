//! Explicit finite lexical candidates for both prefixes and record ambiguity.
#[path = "../../../../language/tests/common/pronunciation_fixture.rs"]
mod base;
pub use base::{analysis, arc, profile};
use conduit_language::{lexical::*, *};
use conduit_plot::rust_binding::BoundedSequence;
pub fn lexical(text: &str) -> PreparedLexicalTape {
    let source = LanguageTextRevision::new(
        LanguageTextFinality::Final,
        LanguageText::new(
            LanguageTextId::new(format!("record-pair/{text}")).unwrap(),
            LanguageId::new("language/en".into()).unwrap(),
            LanguageTextRevisionId::new("record-pair/source@1".into()).unwrap(),
            text.into(),
        )
        .unwrap(),
        None,
        base::provenance(),
        0,
        Some(text.chars().count() as u32),
    )
    .unwrap();
    let simple = |lemma: &str, pos| {
        LanguageLexicalCandidate::new(lemma.into(), BoundedSequence::new(), pos).unwrap()
    };
    let entries = [
        LanguageLexicalEntry::new(
            BoundedSequence::try_from_iter([simple("a", LanguageLexicalPos::Determiner)]).unwrap(),
            "a".into(),
        )
        .unwrap(),
        LanguageLexicalEntry::new(
            BoundedSequence::try_from_iter([simple("we", LanguageLexicalPos::Pronoun)]).unwrap(),
            "we".into(),
        )
        .unwrap(),
        LanguageLexicalEntry::new(
            BoundedSequence::try_from_iter([
                base::candidate(LanguageLexicalPos::Noun),
                base::candidate(LanguageLexicalPos::Verb),
            ])
            .unwrap(),
            "record".into(),
        )
        .unwrap(),
    ];
    let profile = LanguageLexicalProfile::new(
        BoundedSequence::try_from_iter(entries).unwrap(),
        "reviewed/record-pair/lexical@1".into(),
        source.material().language().clone(),
        base::provenance(),
    )
    .unwrap();
    prepare_lexical_tape(&source, &profile, None).unwrap()
}
