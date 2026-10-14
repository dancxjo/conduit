#![allow(dead_code)]
#[path = "parser_fixture.rs"]
mod symbolic;
use conduit_language::*;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
pub fn provenance() -> LinguisticDerivationProvenance {
    LinguisticDerivationProvenance::deterministic_rule("fixture/revision".into(), "1".into())
        .unwrap()
}
pub fn revision(
    text: &str,
    old: Option<&LanguageTextRevision>,
    stable: u32,
) -> LanguageTextRevision {
    let sequence = old.map_or(0, |s| s.sequence() + 1);
    LanguageTextRevision::new(
        LanguageTextFinality::Partial,
        LanguageText::new(
            LanguageTextId::new("utterance".into()).unwrap(),
            LanguageId::new("language/en".into()).unwrap(),
            LanguageTextRevisionId::new(format!("source/{}", sequence + 1)).unwrap(),
            text.into(),
        )
        .unwrap(),
        old.map(|s| {
            LanguageTextPriorRevision::new(s.material().revision().clone(), *s.sequence()).unwrap()
        }),
        provenance(),
        sequence,
        Some(stable),
    )
    .unwrap()
}
pub fn profile() -> LanguageLexicalProfile {
    let entries = ["answer", "record"].into_iter().map(|surface| {
        LanguageLexicalEntry::new(
            BoundedSequence::try_from_iter([
                LanguageLexicalCandidate::new(
                    surface.into(),
                    BoundedSequence::new(),
                    LanguageLexicalPos::Noun,
                )
                .unwrap(),
                LanguageLexicalCandidate::new(
                    surface.into(),
                    BoundedSequence::new(),
                    LanguageLexicalPos::Verb,
                )
                .unwrap(),
            ])
            .unwrap(),
            surface.into(),
        )
        .unwrap()
    });
    LanguageLexicalProfile::new(
        BoundedSequence::try_from_iter(entries).unwrap(),
        "fixture/revision-lexical".into(),
        LanguageId::new("language/en".into()).unwrap(),
        provenance(),
    )
    .unwrap()
}
pub fn state(committed: u64) -> LanguageParserState {
    state_with_relation(committed, "obj")
}
pub fn state_with_relation(committed: u64, relation: &str) -> LanguageParserState {
    let mut f = symbolic::Fixture::new();
    let initial = f.initial(2);
    let first = f.step(&initial, "right_arc", "root", "analysis/1");
    assert!(symbolic::accepted(&first));
    let second = f.step(
        symbolic::field(&first, "state"),
        "right_arc",
        relation,
        "analysis/1",
    );
    assert!(symbolic::accepted(&second));
    let raw =
        LanguageParserNumericState::from_structured(symbolic::field(&second, "state").clone())
            .unwrap();
    LanguageParserState::new(
        raw.basis().clone(),
        committed,
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
pub fn dep() -> LanguageParserRelation {
    LanguageParserRelation::new(
        LanguageUniversalDependencyRelation::Dep,
        LanguageParserSubtype::new(String::new()).unwrap(),
    )
    .unwrap()
}
pub fn source() -> String {
    [
        include_str!("../../types.conduit"),
        include_str!("../../identity.conduit"),
        include_str!("../../coverage.conduit"),
        include_str!("../../syntax.conduit"),
        include_str!("../../text_revision.conduit"),
        include_str!("../../revision_lineage.conduit"),
        include_str!("../../lexical.conduit"),
        include_str!("../../parser.conduit"),
        include_str!("../../parser_beam.conduit"),
        include_str!("../../parser_available.conduit"),
        PARSER_REVISION_REBASE_SOURCE,
        PARSER_BEAM_COMMIT_SOURCE,
        RANK_AND_PRUNE,
    ]
    .join("\n")
}

/// Admit the exact Source-produced numeric state without changing its fields.
pub fn native_state(raw: &LanguageParserNumericState) -> LanguageParserState {
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

/// Authored composition of the existing five-comparator network and pruning law.
pub const RANK_AND_PRUNE: &str = r#"
plot fixture-parser-beam-view (
 input: LanguageParserAgreementQuery...| >> output: LanguageParserBeam...|
) = .beam
plot fixture-parser-prune-request (
 input: LanguageParserBeam...| >> output: LanguageParserPruneRequest...|
) = { beam: ., maximum_survivors: 1 }
plot fixture-parser-rank-and-prune (
 input: LanguageParserAdmittedAgreementQuery...| >> output: LanguageParserBeam...|
) {
 input >> language-parser-admitted-agreement-query() >> fixture-parser-beam-view() >> language-parser-rank-0-1() >> language-parser-rank-2-3() >> language-parser-rank-0-2() >> language-parser-rank-1-3() >> language-parser-rank-1-2() >> fixture-parser-prune-request() >> language-parser-retain() >> output
}
"#;
