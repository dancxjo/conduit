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
    let native_type = LanguageResourceLexicalBinding::semantic_type().unwrap();
    let encoded = query.encode().unwrap();
    let decoded =
        conduit_core::StructuredInfoValue::from_canonical_bytes(&encoded).unwrap_or_else(|error| {
            panic!(
                "Native query canonical validation failed: {error:?}; bytes={} Source maximum={:?}",
                encoded.len(),
                program.program.input_maximum_bytes,
            )
        });
    assert!(
        decoded.value_type() == &native_type,
        "decoded query Type differs from generated Native Type: decoded={:?} Native={:?}",
        decoded.value_type().semantic_digest(),
        native_type.semantic_digest()
    );
    assert!(
        program.program.input_type == native_type,
        "Source input Type differs from generated Native Type: Source={:?} Native={:?}",
        program.program.input_type.semantic_digest(),
        native_type.semantic_digest()
    );
    assert!(
        program
            .program
            .input_maximum_bytes
            .is_none_or(|maximum| encoded.len() <= maximum as usize),
        "Native query exceeds actual Source byte bound: bytes={} maximum={:?}",
        encoded.len(),
        program.program.input_maximum_bytes
    );
    program.program.evaluate(&encoded).unwrap() == [1]
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
    assert_eq!(q.token().candidates().len(), 2);
    assert_eq!(q.row().candidates().len(), 2);
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
    // The opposite mismatch must also return false without indexing either
    // sequence beyond its actual length.
    let extra =
        LanguageLexicalCandidate::new("a".into(), BoundedSequence::new(), LanguageLexicalPos::Verb)
            .unwrap();
    let extended = LanguageLexicalEntry::new(
        BoundedSequence::try_from_iter(q.row().candidates().iter().cloned().chain([extra]))
            .unwrap(),
        q.row().surface().clone(),
    )
    .unwrap();
    assert_eq!(extended.candidates().len(), 3);
    assert!(!accepted(
        LanguageResourceLexicalBinding::new(
            0,
            q.resource().clone(),
            extended,
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
