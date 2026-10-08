//! Contiguous commit custody is checked separately from stable arc admission.
use conduit_language::*;
use conduit_plot::rust_binding::{
    validate_native_contracts, validate_native_invariants, NativeRustBinding,
};
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};
#[path = "common/parser_fixture.rs"]
mod fixture;
#[path = "common/parser_joint_v2_fixture.rs"]
mod joint;
#[path = "common/scorer_model.rs"]
mod scorer_model;

#[test]
fn actual_contiguous_commit_correlates_the_complete_retained_fact_and_runtime() {
    let source = [
        joint::source(),
        include_str!("../syntax.conduit").into(),
        include_str!("../parser_session.conduit").into(),
        include_str!("../parser_session_facts.conduit").into(),
        include_str!("../parser_session_dependency.conduit").into(),
        include_str!("../parser_session_commit.conduit").into(),
        include_str!("../parser_session_committed_dependency.conduit").into(),
    ]
    .join("\n");
    let checked =
        check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new()).unwrap();
    let native = checked
        .native_types
        .iter()
        .find(|t| t.name == "LanguageParserCommittedDependencyAdmission")
        .unwrap();
    let events: Vec<serde_json::Value> =
        include_str!("../training/ewt_joint_v2/native_stream_protected_events.jsonl")
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
    let snapshot = events
        .iter()
        .find(|row| row["event"] == "snapshot" && row["committed"] == 1)
        .unwrap();
    let bytes: Vec<u8> = serde_json::from_value(snapshot["stable_fact_bytes"][0].clone()).unwrap();
    let fact = LanguageParserJointStableFact::decode(&bytes).unwrap();
    assert_eq!(
        fact.query().beam().lexical().tape().source().finality(),
        &LanguageTextFinality::Partial
    );
    let dependent = *fact.query().dependent() as usize;
    assert_eq!(dependent, 0);
    let state = fact.query().beam().candidate0().parser().state();
    let head = state.heads()[dependent];
    assert!(head < 4);
    let token = |i: usize| {
        LanguageAnalysisTokenRef::new(
            fact.query().beam().basis().analysis_revision().clone(),
            fact.query().beam().lexical().tape().tokens()[i]
                .identity()
                .clone(),
        )
        .unwrap()
    };
    let governor = token(head as usize);
    let arc = LanguageDependencyArc::new(
        token(dependent),
        LanguageDependencyHead::token(governor.revision().clone(), governor.token().clone())
            .unwrap(),
        LanguageDependencyRelation::new(*state.relation0().base(), None).unwrap(),
    )
    .unwrap();
    assert_eq!(state.relation0().subtype().get(), "");
    let admission = LanguageParserStableDependencyAdmission::new(
        arc,
        fact.clone(),
        head,
        state.relation0().subtype().clone(),
    )
    .unwrap();
    let commit = LanguageParserJointCommitQuery::new(fact.clone()).unwrap();
    let row = events
        .iter()
        .find(|row| {
            row["event"] == "dependency-commit"
                && row["source_revision"] == snapshot["source_revision"]
        })
        .unwrap();
    let bytes: Vec<u8> = serde_json::from_value(row["native_runtime_bytes"].clone()).unwrap();
    let runtime = LanguageParserJointRuntimeBeam::decode(&bytes).unwrap();
    let propose = |runtime: LanguageParserJointRuntimeBeam| {
        fixture::record(
            &native.value_type,
            vec![
                ("admission", admission.clone().into_structured().unwrap()),
                ("commit", commit.clone().into_structured().unwrap()),
                ("runtime", runtime.into_structured().unwrap()),
            ],
        )
    };
    let validate = |runtime| {
        let value = propose(runtime);
        validate_native_contracts(&value, &native.value_contracts)
            .and_then(|()| validate_native_invariants(&value, &native.invariants))
    };
    validate(runtime.clone()).unwrap();
    // A structurally valid uncommitted snapshot cannot be substituted for the
    // Source operation's actual committed output.
    let uncommitted =
        LanguageParserJointRuntimeBeam::new(fact.query().beam().clone(), *runtime.selected())
            .unwrap();
    assert!(validate(uncommitted).is_err());
}
