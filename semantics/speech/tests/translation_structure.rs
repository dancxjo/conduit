#![cfg(feature = "semantic-bindings")]
use conduit_language::{discourse::*, lexical::*, *};
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduit_speech::{semantic::*, translation_structure::*};
#[path = "common/asr_graph_sources.rs"]
#[allow(dead_code)]
mod asr_sources;
#[path = "common/learned_graph_receipt.rs"]
#[allow(dead_code)]
mod receipt;
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
    let source_tape = lexical(&source);
    let source_fact = fact(&source_tape, 2);
    check_alignment(source_tape, source_fact);
}
fn check_alignment(source_tape: PreparedLexicalTape, source_fact: PreparedVocativeFact) {
    let source = source_tape.tape().source().clone();
    let target = revision("portuguese", "language/pt", "Olá Travis");
    let target_tape = lexical(&target);
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

#[test]
#[ignore = "requires actual exact punctuated learned source graph; target and alignment remain explicit supplied fixtures"]
fn learned_source_graph_retains_addressee_through_explicit_translation_alignment() {
    let path = std::env::var("CONDUIT_LEARNED_GRAPH_RECEIPTS").unwrap();
    assert!(std::fs::metadata(&path).unwrap().len() <= 8 * 1024 * 1024);
    let rows: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["text"], "Hello, Travis.");
    let admitted = receipt::admit(&rows[0]).unwrap();
    assert_eq!(admitted.vocative, 2);
    let source = admitted.lexical.tape().source();
    let fact = prepare_vocative_fact(
        "learned/source/addressee".into(),
        source,
        admitted.basis.analysis_revision(),
        &admitted.arcs[admitted.vocative],
        source.provenance().clone(),
        admitted.lexical.tape().tokens().len() as u64,
    )
    .unwrap();
    check_alignment(admitted.lexical, fact);
}

#[test]
#[ignore = "requires whole actual independent protection via CONDUIT_INDEPENDENT_RECEIPT_PATH; target/alignment are supplied fixtures"]
fn protected_partial_vocative_enters_explicit_translation_alignment() {
    let bytes = std::fs::read(std::env::var("CONDUIT_INDEPENDENT_RECEIPT_PATH").unwrap()).unwrap();
    let protected = LanguageParserIndependentProtectedAdmission::decode(&bytes).unwrap();
    let admission = protected.admission();
    let query = admission.fact().query();
    let native_tape = query.beam().lexical().tape();
    assert_eq!(native_tape.source().material().text(), "Hello, Travis ");
    assert_eq!(
        *native_tape.source().finality(),
        LanguageTextFinality::Partial
    );
    assert_eq!(*query.dependent(), 2);
    assert_eq!(*admission.head(), 0);
    assert_eq!(*query.beam().candidate0().parser().state().committed(), 0);
    let lexical = prepare_lexical_tape(native_tape.source(), native_tape.profile(), None).unwrap();
    assert_eq!(lexical.tape(), native_tape);
    let fact = prepare_vocative_fact(
        "actual-independent/addressee-translation".into(),
        native_tape.source(),
        query.beam().basis().analysis_revision(),
        admission.arc(),
        native_tape.source().provenance().clone(),
        native_tape.tokens().len() as u64,
    )
    .unwrap();
    assert_eq!(fact.fact().basis(), admission.arc());
    assert_eq!(fact.fact().source(), native_tape.source());
    // Native admission owns the source arc. The helper supplies only the target
    // graph and accepted alignment; it does not infer them from punctuation.
    check_alignment(lexical, fact);
    assert_eq!(protected.clone().encode().unwrap(), bytes);
}
