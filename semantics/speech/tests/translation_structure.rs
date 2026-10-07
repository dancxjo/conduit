#![cfg(feature = "semantic-bindings")]
use conduit_language::{discourse::*, lexical::*, *};
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::{semantic::*, translation_structure::*};
fn provenance() -> LinguisticDerivationProvenance {
    LinguisticDerivationProvenance::deterministic_rule(
        "supplied-translation-ud-fixture".into(),
        "fixture/1".into(),
    )
    .unwrap()
}
fn provider() -> ListeningProvenance {
    ListeningProvenance::new(
        BoundedSequence::new(),
        ListeningProvenanceKind::Derived,
        None,
        None,
        BoundedSequence::new(),
    )
    .unwrap()
}
fn revision(id: &str, language: &str, text: &str) -> LanguageTextRevision {
    LanguageTextRevision::new(
        LanguageTextFinality::Final,
        LanguageText::new(
            LanguageTextId::new(id.into()).unwrap(),
            LanguageId::new(language.into()).unwrap(),
            LanguageTextRevisionId::new("r1".into()).unwrap(),
            text.into(),
        )
        .unwrap(),
        None,
        provenance(),
        0,
        None,
    )
    .unwrap()
}
fn lexical(source: &LanguageTextRevision) -> PreparedLexicalTape {
    let profile = LanguageLexicalProfile::new(
        BoundedSequence::new(),
        "fixture/lexical".into(),
        source.material().language().clone(),
        provenance(),
    )
    .unwrap();
    prepare_lexical_tape(source, &profile, None).unwrap()
}
fn fact(tape: &PreparedLexicalTape, ordinal: usize) -> PreparedVocativeFact {
    let analysis = LanguageAnalysisRevisionId::new(format!(
        "{}/analysis",
        tape.tape().source().material().identity().get()
    ))
    .unwrap();
    let tokens = tape.tape().tokens().as_slice();
    let arc = LanguageDependencyArc::new(
        LanguageAnalysisTokenRef::new(analysis.clone(), tokens[ordinal].identity().clone())
            .unwrap(),
        LanguageDependencyHead::token(analysis.clone(), tokens[0].identity().clone()).unwrap(),
        LanguageDependencyRelation::new(LanguageUniversalDependencyRelation::Vocative, None)
            .unwrap(),
    )
    .unwrap();
    prepare_vocative_fact(
        "fixture/addressee".into(),
        tape.tape().source(),
        &analysis,
        &arc,
        provenance(),
        tokens.len() as u64,
    )
    .unwrap()
}
fn reference(tape: &PreparedLexicalTape, ordinal: usize) -> LanguageTextSegmentRef {
    language_source_occurrence(
        tape.tape().source().material(),
        tape.tape().tokens().as_slice()[ordinal].span(),
        LanguageTextSegmentKind::Word,
    )
    .unwrap()
}
fn segment(reference: &LanguageTextSegmentRef) -> LanguageSegmentRef {
    LanguageSegmentRef::text(
        *reference.kind(),
        reference.language().clone(),
        reference.range().clone(),
        reference.revision_id().clone(),
        reference.text_id().clone(),
    )
    .unwrap()
}
#[test]
fn accepted_reordered_alignment_preserves_addressee_without_target_comma() {
    let source = revision("english", "language/en", "Hello, Travis.");
    let target = revision("portuguese", "language/pt", "Olá Travis");
    let source_tape = lexical(&source);
    let target_tape = lexical(&target);
    let source_fact = fact(&source_tape, 2);
    let target_fact = fact(&target_tape, 1);
    let group = TranslationAlignedGroup::new(
        BoundedSequence::try_from_iter([
            segment(&reference(&source_tape, 0)),
            segment(&reference(&source_tape, 2)),
        ])
        .unwrap(),
        BoundedSequence::try_from_iter([
            segment(&reference(&target_tape, 1)),
            segment(&reference(&target_tape, 0)),
        ])
        .unwrap(),
    )
    .unwrap();
    let alignment = TranslationAlignment::new(
        None,
        TranslationCorrespondence::aligned(group.sources().clone(), group.targets().clone())
            .unwrap(),
        TranslationAlignmentId::new("alignment".into()).unwrap(),
        provider(),
    )
    .unwrap();
    let candidate = TranslationCandidate::new(
        BoundedSequence::try_from_iter([alignment]).unwrap(),
        TranslationCandidateId::new("candidate".into()).unwrap(),
        provider(),
        source.material().clone(),
        target.material().clone(),
    )
    .unwrap();
    let accepted = TranslationAccepted::new(
        provider(),
        candidate.identity().clone(),
        source.material().revision().clone(),
        source.material().identity().clone(),
        target.material().revision().clone(),
        target.material().identity().clone(),
    )
    .unwrap();
    let request = |source_slot, target_slot| TranslationVocativeRequest {
        candidate: &candidate,
        accepted: &accepted,
        source: &source_fact,
        target: &target_fact,
        source_lexical: &source_tape,
        target_lexical: &target_tape,
        alignment_index: 0,
        source_slot,
        target_slot,
    };
    let proof = prepare_translation_vocative(request(1, 0)).unwrap();
    assert_eq!(proof.alignment(), &candidate.alignments().as_slice()[0]);
    assert_eq!(proof.source_fact().fact(), source_fact.fact());
    assert_eq!(proof.target_fact().fact(), target_fact.fact());
    assert_eq!(
        proof.witness().source().analysis(),
        source_fact.fact().analysis()
    );
    assert_eq!(
        proof.witness().target().material().language(),
        target.material().language()
    );
    assert!(matches!(
        prepare_translation_vocative(request(0, 0)),
        Err(TranslationStructureRefusal::Unaligned)
    ));
    assert!(matches!(
        prepare_translation_vocative(request(1, 1)),
        Err(TranslationStructureRefusal::Unaligned)
    ));
    assert!(matches!(
        prepare_translation_vocative(request(9, 0)),
        Err(TranslationStructureRefusal::NonTextAlignment)
    ));
    let stale = TranslationAccepted::new(
        provider(),
        candidate.identity().clone(),
        source.material().revision().clone(),
        source.material().identity().clone(),
        LanguageTextRevisionId::new("r0".into()).unwrap(),
        target.material().identity().clone(),
    )
    .unwrap();
    let mut stale_request = request(1, 0);
    stale_request.accepted = &stale;
    assert!(matches!(
        prepare_translation_vocative(stale_request),
        Err(TranslationStructureRefusal::Native(_))
    ));
    let changed_source = revision("english", "language/en", "Hello different words");
    let changed_lexical = lexical(&changed_source);
    let mut foreign = request(1, 0);
    foreign.source_lexical = &changed_lexical;
    assert!(matches!(
        prepare_translation_vocative(foreign),
        Err(TranslationStructureRefusal::LexicalBasis)
    ));
}
