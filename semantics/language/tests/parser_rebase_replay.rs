//! Native preparation replay only; no installed runtime/currentness assertion.
#[path = "common/parser_revision_fixture.rs"]
mod fixture;
use conduit_language::{lexical::prepare_lexical_tape, parser_rebase_replay::*, *};
use conduit_plot::rust_binding::NativeRustBinding;
fn preparation() -> PreparedParserRevisionRebase {
    let old = fixture::revision("answer record ", None, 7);
    let lexical = prepare_lexical_tape(&old, &fixture::profile(), None).unwrap();
    let root = LanguageParserRelation::new(
        LanguageUniversalDependencyRelation::Root,
        LanguageParserSubtype::new("".into()).unwrap(),
    )
    .unwrap();
    let previous = LanguageParserState::new(
        LanguageParserBasis::new(
            LanguageAnalysisRevisionId::new("analysis/1".into()).unwrap(),
            old.material().revision().clone(),
            old.material().identity().clone(),
        )
        .unwrap(),
        1,
        3,
        [4, 0, 5, 5, 4],
        root,
        LanguageParserRelation::new(
            LanguageUniversalDependencyRelation::Obj,
            LanguageParserSubtype::new("".into()).unwrap(),
        )
        .unwrap(),
        fixture::dep(),
        fixture::dep(),
        [4, 0, 1, 4, 4],
        2,
        2,
    )
    .unwrap();
    PreparedParserRevisionRebase::prepare(
        &previous,
        &lexical,
        &fixture::revision("answer answer ", Some(&old), 7),
        LanguageAnalysisRevisionId::new("analysis/2".into()).unwrap(),
        fixture::dep(),
    )
    .unwrap()
}
fn expected(p: &PreparedParserRevisionRebase, unread: u64) -> LanguageParserNumericState {
    // Authored fixture expectation, never an installed execution receipt.
    LanguageParserNumericState::new(
        p.request().basis().clone(),
        1,
        2,
        [4, 5, 5, 5, 4],
        p.request().previous().relation0().clone(),
        fixture::dep(),
        fixture::dep(),
        fixture::dep(),
        [4, 0, 4, 4, 4],
        2,
        unread,
    )
    .unwrap()
}
#[test]
fn one_program_replay_preserves_committed_prefix_and_exact_native_custody() {
    let program = PreparedParserRebaseProgram::prepare().unwrap();
    let p = preparation();
    let observed = expected(&p, 1);
    let request = p.request().clone();
    let receipt = program.replay(p, &observed).unwrap();
    assert_eq!(receipt.preparation().request(), &request);
    assert_eq!(receipt.raw_result(), &observed);
    assert_eq!(*receipt.result().committed(), 1);
    assert_eq!(*receipt.result().unread(), 1);
    assert_eq!(receipt.result().heads(), &[4, 5, 5, 5, 4]);
    assert_eq!(receipt.input_canonical(), request.encode().unwrap());
    assert_eq!(receipt.output_canonical(), observed.encode().unwrap());
    assert!(!receipt.program_hex().is_empty());
    assert!(!receipt.source_id().is_empty());
    // Four independent survivor preparations share this exact immutable parse.
    for _ in 0..3 {
        let p = preparation();
        let observed = expected(&p, 1);
        let next = program.replay(p, &observed).unwrap();
        assert_eq!(next.output_canonical(), receipt.output_canonical());
    }
}
#[test]
fn stale_basis_and_wrong_state_observations_refuse_without_runtime_preparation() {
    let program = PreparedParserRebaseProgram::prepare().unwrap();
    let p = preparation();
    let wrong = expected(&p, 2);
    assert!(matches!(
        program.replay(p, &wrong),
        Err(ParserRebaseReplayRefusal::ObservationMismatch)
    ));
    let p = preparation();
    let e = expected(&p, 1);
    let stale = LanguageParserNumericState::new(
        p.request().previous().basis().clone(),
        *e.committed(),
        *e.depth(),
        *e.heads(),
        e.relation0().clone(),
        e.relation1().clone(),
        e.relation2().clone(),
        e.relation3().clone(),
        *e.stack(),
        *e.token_count(),
        *e.unread(),
    )
    .unwrap();
    assert!(matches!(
        program.replay(p, &stale),
        Err(ParserRebaseReplayRefusal::ObservationMismatch)
    ));
}
