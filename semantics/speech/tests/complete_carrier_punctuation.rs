#![cfg(all(feature = "semantic-bindings", feature = "kernel"))]
//! Supplied symbolic analysis, not learned syntax or punctuation-driven timing.
#[path = "common/carried_formant_fixture.rs"]
mod fixture;
use conduit_language::{lexical::prepare_lexical_tape, *};
use conduit_speech::{complete_carrier::*, semantic::*};

fn reference(kind: LanguageTextSegmentKind, start: u32, end: u32) -> LanguageSegmentRef {
    let span = fixture::text_ref(kind, start, end);
    LanguageSegmentRef::text(
        *span.kind(),
        span.language().clone(),
        span.range().clone(),
        span.revision_id().clone(),
        span.text_id().clone(),
    )
    .unwrap()
}

fn request(source: &LanguageTextRevision) -> LanguageParserBeamCommitRequest {
    let basis = LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new("punctuation/analysis".into()).unwrap(),
        source.material().revision().clone(),
        source.material().identity().clone(),
    )
    .unwrap();
    let relation = |base| {
        LanguageParserRelation::new(base, LanguageParserSubtype::new("".into()).unwrap()).unwrap()
    };
    let state = LanguageParserState::new(
        basis.clone(),
        0,
        4,
        [4, 0, 0, 5, 4],
        relation(LanguageUniversalDependencyRelation::Root),
        relation(LanguageUniversalDependencyRelation::Punct),
        relation(LanguageUniversalDependencyRelation::Dep),
        relation(LanguageUniversalDependencyRelation::Dep),
        [4, 0, 1, 2, 4],
        3,
        3,
    )
    .unwrap();
    let hypothesis = |active, identity| {
        LanguageParserHypothesis::new(active, identity, 0, state.clone()).unwrap()
    };
    LanguageParserBeamCommitRequest::new(
        LanguageParserAdmittedBeam::new(
            basis.clone(),
            hypothesis(true, 1),
            hypothesis(false, 2),
            hypothesis(false, 3),
            hypothesis(false, 4),
        )
        .unwrap(),
        false,
        LanguageParserRawFrontier::new(basis, 0, 3).unwrap(),
        false,
        3,
    )
    .unwrap()
}

#[test]
fn punctuation_requires_explicit_source_only_relation_without_losing_parser_coverage() {
    let mut f = fixture::Fixture::new();
    let old = &f.source;
    let text = old.material();
    // Replacing one space preserves the exact word ranges; the lexical owner
    // still emits a third token for the comma, which must be committed too.
    f.source = LanguageTextRevision::new(
        *old.finality(),
        LanguageText::new(
            text.identity().clone(),
            text.language().clone(),
            text.revision().clone(),
            "you,would".into(),
        )
        .unwrap(),
        old.prior().clone(),
        old.provenance().clone(),
        *old.sequence(),
        *old.stable_prefix(),
    )
    .unwrap();
    let inventory = f.inventory().unwrap();
    let original = f.correspondence(&inventory).unwrap();
    let structure = f.structure();
    let lexical = prepare_lexical_tape(&f.source, &f.lexical_profile, None).unwrap();
    assert_eq!(lexical.tape().tokens().len(), 3);
    let request = request(&f.source);
    let result = fixture::execute_commitment(&request);
    let omitted = TranslationCorrespondence::omitted(
        "punctuation retained as symbolic source; no phoneme realization".into(),
        fixture::seq([reference(LanguageTextSegmentKind::Grapheme, 3, 4)]),
    )
    .unwrap();
    let mut relations = f.word_phonemes.clone();
    relations.push(omitted.clone());
    let prepare = |relations: &[TranslationCorrespondence]| {
        PreparedCompleteSpeechCarrier::prepare(
            &original,
            &structure,
            &lexical,
            relations,
            &request,
            &result,
            CompleteCarrierAcoustics::not_supplied(),
        )
        .is_ok()
    };
    assert!(prepare(&relations));
    assert!(
        !prepare(&f.word_phonemes),
        "punctuation cannot disappear implicitly"
    );
    relations.push(omitted);
    assert!(!prepare(&relations), "duplicate source coverage refuses");
    let word_omission = TranslationCorrespondence::omitted(
        "attempted unspoken word".into(),
        fixture::seq([reference(LanguageTextSegmentKind::Word, 0, 3)]),
    )
    .unwrap();
    relations[2] = word_omission;
    relations.pop();
    assert!(
        !prepare(&relations),
        "a word cannot acquire punctuation omission"
    );
}
