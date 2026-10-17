//! Test-only native admission of pinned lexical data and revisioned values.
//! These helpers never select an action, attachment, lexical choice or frontier.
#![allow(dead_code)]
use conduit_core::semantic_digest;
use conduit_language::{parser_model::ENGLISH_PARSER_LEXICAL_PROFILE, *};
use conduit_plot::rust_binding::BoundedSequence;
use std::collections::BTreeMap;

fn hex(bytes: [u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
fn provenance() -> LinguisticDerivationProvenance {
    LinguisticDerivationProvenance::deterministic_rule(
        "language/parser-source-admission@1".into(),
        "source/1".into(),
    )
    .unwrap()
}
/// Exact seventeen-code UPOS order retained in the accepted artifact schema.
pub fn pos(code: u64) -> LanguageLexicalPos {
    match code {
        0 => LanguageLexicalPos::Adjective,
        1 => LanguageLexicalPos::Adposition,
        2 => LanguageLexicalPos::Adverb,
        3 => LanguageLexicalPos::Auxiliary,
        4 => LanguageLexicalPos::CoordinatingConjunction,
        5 => LanguageLexicalPos::Determiner,
        6 => LanguageLexicalPos::Interjection,
        7 => LanguageLexicalPos::Noun,
        8 => LanguageLexicalPos::Numeral,
        9 => LanguageLexicalPos::Particle,
        10 => LanguageLexicalPos::Pronoun,
        11 => LanguageLexicalPos::ProperNoun,
        12 => LanguageLexicalPos::Punctuation,
        13 => LanguageLexicalPos::SubordinatingConjunction,
        14 => LanguageLexicalPos::Symbol,
        15 => LanguageLexicalPos::Verb,
        16 => LanguageLexicalPos::Other,
        _ => panic!("UPOS code outside accepted schema"),
    }
}
/// Every lexical alternative comes from the exact retained profile JSON.
pub fn profile() -> LanguageLexicalProfile {
    let inventory: BTreeMap<String, Vec<u64>> =
        serde_json::from_str(ENGLISH_PARSER_LEXICAL_PROFILE).unwrap();
    let entries = inventory.into_iter().map(|(surface, codes)| {
        let candidates = codes.into_iter().map(|code| {
            LanguageLexicalCandidate::new(surface.clone(), BoundedSequence::new(), pos(code))
                .unwrap()
        });
        LanguageLexicalEntry::new(BoundedSequence::try_from_iter(candidates).unwrap(), surface)
            .unwrap()
    });
    let identity = hex(semantic_digest(
        "language/parser-lexical-profile@1",
        ENGLISH_PARSER_LEXICAL_PROFILE.as_bytes(),
    ));
    LanguageLexicalProfile::new(
        BoundedSequence::try_from_iter(entries).unwrap(),
        identity.clone(),
        LanguageId::new("language/en".into()).unwrap(),
        LinguisticDerivationProvenance::library(
            "language/parser-pinned-lexical-profile@1".into(),
            identity,
        )
        .unwrap(),
    )
    .unwrap()
}
/// `id` is the exact source revision identity; all revisions share one text ID.
pub fn revision(
    text: &str,
    id: &str,
    sequence: u64,
    previous: Option<&LanguageTextRevision>,
    stable: Option<u32>,
    finality: LanguageTextFinality,
) -> LanguageTextRevision {
    revision_with_text(text, "utterance", id, sequence, previous, stable, finality)
}
pub fn revision_with_text(
    text: &str,
    text_id: &str,
    id: &str,
    sequence: u64,
    previous: Option<&LanguageTextRevision>,
    stable: Option<u32>,
    finality: LanguageTextFinality,
) -> LanguageTextRevision {
    LanguageTextRevision::new(
        finality,
        LanguageText::new(
            LanguageTextId::new(text_id.into()).unwrap(),
            LanguageId::new("language/en".into()).unwrap(),
            LanguageTextRevisionId::new(id.into()).unwrap(),
            text.into(),
        )
        .unwrap(),
        previous.map(|old| {
            LanguageTextPriorRevision::new(old.material().revision().clone(), *old.sequence())
                .unwrap()
        }),
        provenance(),
        sequence,
        stable,
    )
    .unwrap()
}
pub fn dep() -> LanguageParserRelation {
    LanguageParserRelation::new(
        LanguageUniversalDependencyRelation::Dep,
        LanguageParserSubtype::new(String::new()).unwrap(),
    )
    .unwrap()
}
pub fn admit_state(raw: LanguageParserNumericState) -> LanguageParserState {
    LanguageParserState::new(
        raw.basis().clone(),
        *raw.committed(),
        *raw.depth(),
        *raw.heads(),
        raw.relation0().clone(),
        raw.relation1().clone(),
        raw.relation2().clone(),
        raw.relation3().clone(),
        *raw.stack(),
        *raw.token_count(),
        *raw.unread(),
    )
    .unwrap()
}
pub fn admit_hyp(raw: LanguageParserScorerRawHypothesis) -> LanguageParserScorerHypothesis {
    let parser = raw.parser();
    let admitted = LanguageParserHypothesis::new(
        *parser.active(),
        *parser.identity(),
        *parser.score(),
        admit_state(parser.state().clone()),
    )
    .unwrap();
    LanguageParserScorerHypothesis::new(*raw.choices(), admitted, *raw.selected()).unwrap()
}
