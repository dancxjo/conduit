#![cfg(all(feature = "semantic-bindings", feature = "kernel"))]
#[path = "common/carried_formant_fixture.rs"]
mod fixture;
use conduit_language::{
    lexical::prepare_lexical_tape, LanguageParserBeamCommitResult, LanguageText,
    LanguageTextRevision, LanguageTextRevisionId,
};
use conduit_speech::{complete_carrier::*, semantic::*};

#[test]
fn malformed_word_relations_foreign_revision_and_forged_commitment_cannot_enter_complete_custody() {
    let f = fixture::Fixture::new();
    let inventory = f.inventory().unwrap();
    let original = f.correspondence(&inventory).unwrap();
    let structure = f.structure();
    let evidence = f.complete_evidence();
    let prepare = |words: &[TranslationCorrespondence],
                   lexical: &conduit_language::lexical::PreparedLexicalTape,
                   result: &LanguageParserBeamCommitResult| {
        PreparedCompleteSpeechCarrier::prepare(
            &original,
            &structure,
            lexical,
            words,
            &evidence.request,
            result,
            CompleteCarrierAcoustics::not_supplied(),
        )
        .is_err()
    };
    assert!(!prepare(
        &f.word_phonemes,
        &evidence.lexical,
        &evidence.result
    ));
    assert!(prepare(
        &f.word_phonemes[..1],
        &evidence.lexical,
        &evidence.result
    ));
    let mut duplicates = f.word_phonemes.clone();
    duplicates.push(duplicates[0].clone());
    assert!(prepare(&duplicates, &evidence.lexical, &evidence.result));
    let text = f.source.material();
    let foreign = LanguageTextRevision::new(
        *f.source.finality(),
        LanguageText::new(
            text.identity().clone(),
            text.language().clone(),
            LanguageTextRevisionId::new("foreign/source-revision".into()).unwrap(),
            text.text().clone(),
        )
        .unwrap(),
        f.source.prior().clone(),
        f.source.provenance().clone(),
        *f.source.sequence(),
        *f.source.stable_prefix(),
    )
    .unwrap();
    let foreign_lexical = prepare_lexical_tape(&foreign, &f.lexical_profile, None).unwrap();
    assert!(prepare(
        &f.word_phonemes,
        &foreign_lexical,
        &evidence.result
    ));
    let r = &evidence.result;
    let forged = LanguageParserBeamCommitResult::new(
        false,
        r.candidate0().clone(),
        r.candidate1().clone(),
        r.candidate2().clone(),
        r.candidate3().clone(),
        r.frontier().clone(),
        r.refusal().clone(),
    )
    .unwrap();
    assert!(prepare(&f.word_phonemes, &evidence.lexical, &forged));
}

#[test]
fn inactive_beam_slots_do_not_acquire_active_token_coverage_requirements() {
    use conduit_language::{
        LanguageParserAdmittedBeam, LanguageParserBeamCommitRequest, LanguageParserHypothesis,
        LanguageParserState,
    };
    let f = fixture::Fixture::new();
    let inventory = f.inventory().unwrap();
    let original = f.correspondence(&inventory).unwrap();
    let structure = f.structure();
    let evidence = f.complete_evidence();
    let beam = evidence.request.beam();
    let state = beam.candidate1().state();
    let empty = LanguageParserState::new(
        state.basis().clone(),
        0,
        0,
        *state.heads(),
        state.relation0().clone(),
        state.relation1().clone(),
        state.relation2().clone(),
        state.relation3().clone(),
        [0; 5],
        0,
        0,
    )
    .unwrap();
    let inactive = LanguageParserHypothesis::new(false, 2, 0, empty).unwrap();
    let request = LanguageParserBeamCommitRequest::new(
        LanguageParserAdmittedBeam::new(
            beam.basis().clone(),
            beam.candidate0().clone(),
            inactive,
            beam.candidate2().clone(),
            beam.candidate3().clone(),
        )
        .unwrap(),
        false,
        evidence.request.frontier().clone(),
        false,
        *evidence.request.through(),
    )
    .unwrap();
    let result = fixture::execute_commitment(&request);
    assert!(*result.accepted());
    PreparedCompleteSpeechCarrier::prepare(
        &original,
        &structure,
        &evidence.lexical,
        &f.word_phonemes,
        &request,
        &result,
        CompleteCarrierAcoustics::not_supplied(),
    )
    .unwrap();
}
