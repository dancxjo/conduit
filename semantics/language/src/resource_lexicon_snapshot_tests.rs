//! Pure Source correlation tests; authored tiny rows below are NOT claims of
//! membership in the pinned lexicon. Only the opaque prepared owner admits that.
use super::*;
use crate::generated::LanguageResourceLexicalBinding;
fn fixture() -> LanguageResourceLexicalBinding {
    let source = LanguageTextRevision::new(
        LanguageTextFinality::Final,
        LanguageText::new(
            LanguageTextId::new("opaque-source".into()).unwrap(),
            LanguageId::new("language/en".into()).unwrap(),
            LanguageTextRevisionId::new("1".into()).unwrap(),
            "a".into(),
        )
        .unwrap(),
        None,
        LinguisticDerivationProvenance::deterministic_rule(
            "correlation-unit-fixture".into(),
            "1".into(),
        )
        .unwrap(),
        0,
        Some(1),
    )
    .unwrap();
    let candidates = BoundedSequence::try_from_iter(
        [LanguageLexicalPos::Determiner, LanguageLexicalPos::Noun].map(|pos| {
            LanguageLexicalCandidate::new("a".into(), BoundedSequence::new(), pos).unwrap()
        }),
    )
    .unwrap();
    let row = LanguageLexicalEntry::new(candidates, "a".into()).unwrap();
    let profile = LanguageLexicalProfile::new(
        BoundedSequence::try_from_iter([row.clone()]).unwrap(),
        "unit-fixture-not-adopted-resource".into(),
        source.material().language().clone(),
        source.provenance().clone(),
    )
    .unwrap();
    let lexical = prepare_lexical_tape(&source, &profile, None).unwrap();
    LanguageResourceLexicalBinding::new(
        0,
        pinned_train_lexicon_reference(),
        row,
        pinned_train_lexicon_reference(),
        source.material().clone(),
        lexical.tape().tokens()[0].clone(),
    )
    .unwrap()
}
fn accepted(query: LanguageResourceLexicalBinding) -> bool {
    let program = PreparedResourceSnapshotProgram::prepare().unwrap();
    program.program.evaluate(&query.encode().unwrap()).unwrap() == [1]
}
#[test]
fn source_correlates_exact_reference_occurrence_and_complete_ordered_candidates() {
    let q = fixture();
    assert!(accepted(q.clone()));
    let other = conduit_core::BoundedResourceRef {
        identity: conduit_core::ResourceSemanticIdentity::from_digest([1; 32]),
        ..q.row_resource().clone()
    };
    assert!(!accepted(
        LanguageResourceLexicalBinding::new(
            *q.ordinal(),
            q.resource().clone(),
            q.row().clone(),
            other,
            q.source().clone(),
            q.token().clone()
        )
        .unwrap()
    ));
    assert!(!accepted(
        LanguageResourceLexicalBinding::new(
            1,
            q.resource().clone(),
            q.row().clone(),
            q.row_resource().clone(),
            q.source().clone(),
            q.token().clone()
        )
        .unwrap()
    ));
    let stale = LanguageText::new(
        q.source().identity().clone(),
        q.source().language().clone(),
        LanguageTextRevisionId::new("stale".into()).unwrap(),
        q.source().text().clone(),
    )
    .unwrap();
    assert!(!accepted(
        LanguageResourceLexicalBinding::new(
            0,
            q.resource().clone(),
            q.row().clone(),
            q.row_resource().clone(),
            stale,
            q.token().clone()
        )
        .unwrap()
    ));
    let dropped = LanguageLexicalEntry::new(
        BoundedSequence::try_from_iter([q.row().candidates()[0].clone()]).unwrap(),
        q.row().surface().clone(),
    )
    .unwrap();
    assert!(!accepted(
        LanguageResourceLexicalBinding::new(
            0,
            q.resource().clone(),
            dropped,
            q.row_resource().clone(),
            q.source().clone(),
            q.token().clone()
        )
        .unwrap()
    ));
    let reversed = LanguageLexicalEntry::new(
        BoundedSequence::try_from_iter(q.row().candidates().iter().rev().cloned()).unwrap(),
        q.row().surface().clone(),
    )
    .unwrap();
    assert!(!accepted(
        LanguageResourceLexicalBinding::new(
            0,
            q.resource().clone(),
            reversed,
            q.row_resource().clone(),
            q.source().clone(),
            q.token().clone()
        )
        .unwrap()
    ));
    let changed = LanguageLexicalEntry::new(q.row().candidates().clone(), "A".into()).unwrap();
    assert!(!accepted(
        LanguageResourceLexicalBinding::new(
            0,
            q.resource().clone(),
            changed,
            q.row_resource().clone(),
            q.source().clone(),
            q.token().clone()
        )
        .unwrap()
    ));
}
