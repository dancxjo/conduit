#![cfg(feature = "parser-model-selection")]
use conduit_language::*;
#[path = "common/parser_fixture.rs"]
mod fixture;
#[path = "common/parser_joint_v2_fixture.rs"]
mod joint;
#[path = "common/scorer_model.rs"]
mod scorer_model;
#[test]
fn checked_initial_state_accepts_exact_generated_source_identity_basis() {
    let mut f = fixture::Fixture::new();
    let basis = LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new("initialization/analysis".into()).unwrap(),
        LanguageTextRevisionId::new("initialization/revision".into()).unwrap(),
        LanguageTextId::new("initialization/text".into()).unwrap(),
    )
    .unwrap();
    let state = joint::initial_from_basis(&mut f, 3, basis.clone());
    assert_eq!(state.basis(), &basis);
    assert_eq!(*state.token_count(), 3);
    assert_eq!(*state.committed(), 0);
}
