#![allow(dead_code)]
use conduit_language::{lexical::*, *};
use conduit_plot::rust_binding::BoundedSequence;
pub fn provenance() -> LinguisticDerivationProvenance {
    LinguisticDerivationProvenance::deterministic_rule(
        "supplied-graph/fixture".into(),
        "reviewed/1".into(),
    )
    .unwrap()
}
pub fn candidate(pos: LanguageLexicalPos) -> LanguageLexicalCandidate {
    LanguageLexicalCandidate::new("record".into(), BoundedSequence::new(), pos).unwrap()
}
pub fn lexical(text: &str) -> PreparedLexicalTape {
    let source = LanguageTextRevision::new(
        LanguageTextFinality::Final,
        LanguageText::new(
            LanguageTextId::new(format!("text/{text}")).unwrap(),
            LanguageId::new("language/en".into()).unwrap(),
            LanguageTextRevisionId::new("r/0".into()).unwrap(),
            text.into(),
        )
        .unwrap(),
        None,
        provenance(),
        0,
        None,
    )
    .unwrap();
    let profile = LanguageLexicalProfile::new(
        BoundedSequence::try_from_iter([LanguageLexicalEntry::new(
            BoundedSequence::try_from_iter([
                candidate(LanguageLexicalPos::Noun),
                candidate(LanguageLexicalPos::Verb),
            ])
            .unwrap(),
            "record".into(),
        )
        .unwrap()])
        .unwrap(),
        "lexical/record".into(),
        source.material().language().clone(),
        provenance(),
    )
    .unwrap();
    prepare_lexical_tape(&source, &profile, None).unwrap()
}
pub fn analysis() -> LanguageAnalysisRevisionId {
    LanguageAnalysisRevisionId::new("supplied-analysis/0".into()).unwrap()
}
pub fn relation(base: LanguageUniversalDependencyRelation) -> LanguageDependencyRelation {
    LanguageDependencyRelation::new(base, None).unwrap()
}
pub fn arc(
    lexical: &PreparedLexicalTape,
    base: LanguageUniversalDependencyRelation,
) -> LanguageDependencyArc {
    let tokens = lexical.tape().tokens();
    LanguageDependencyArc::new(
        LanguageAnalysisTokenRef::new(analysis(), tokens[0].identity().clone()).unwrap(),
        LanguageDependencyHead::token(analysis(), tokens[1].identity().clone()).unwrap(),
        relation(base),
    )
    .unwrap()
}
pub fn profile() -> LanguagePronunciationSelectionProfile {
    LanguagePronunciationSelectionProfile::new(
        "selection/record".into(),
        LanguageId::new("language/en".into()).unwrap(),
        provenance(),
        BoundedSequence::try_from_iter([
            LanguagePronunciationSelectionRule::new(
                LanguageLexicalPos::Noun,
                relation(LanguageUniversalDependencyRelation::Det),
            )
            .unwrap(),
            LanguagePronunciationSelectionRule::new(
                LanguageLexicalPos::Verb,
                relation(LanguageUniversalDependencyRelation::Nsubj),
            )
            .unwrap(),
        ])
        .unwrap(),
    )
    .unwrap()
}
