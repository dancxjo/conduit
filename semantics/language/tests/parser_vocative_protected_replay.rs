//! Immutable actual learned arrivals replayed through ordinary protection owners.
#![cfg(all(feature = "parser-model-selection", target_has_atomic = "ptr"))]
use conduit_core::StructuredInfoValue;
use conduit_language::*;
use conduit_plot::rust_binding::{
    validate_native_contracts, validate_native_invariants, NativeBindingRefusal, NativeRustBinding,
};
#[path = "common/parser_fixture.rs"]
mod fixture;
#[path = "common/parser_joint_v2_fixture.rs"]
mod joint;
#[path = "common/parser_kernel.rs"]
mod parser_kernel;
#[path = "common/scorer_model.rs"]
mod scorer_model;
fn source() -> String {
    [
        joint::source(),
        include_str!("../parser_session.conduit").into(),
        include_str!("../parser_session_facts.conduit").into(),
        include_str!("../parser_session_branch.conduit").into(),
        include_str!("../parser_session_protection.conduit").into(),
        include_str!("../parser_session_protected_set.conduit").into(),
        include_str!("../parser_session_rebase.conduit").into(),
        include_str!("../parser_session_protected_rebase.conduit").into(),
        include_str!("../parser_session_protected_forest.conduit").into(),
    ]
    .join("\n")
}
fn bytes(value: &serde_json::Value) -> StructuredInfoValue {
    StructuredInfoValue::from_canonical_bytes(
        &serde_json::from_value::<Vec<u8>>(value.clone()).unwrap(),
    )
    .unwrap()
}
fn started_execution(source: String, entry: &str, epoch: usize) -> parser_kernel::Execution {
    eprintln!("VOC2 preparation start: {entry}");
    let mut execution = parser_kernel::Blueprint::prepare(source, entry).realize(epoch);
    execution.kernel.start().unwrap();
    eprintln!("VOC2 started: {entry}");
    execution
}
#[test]
#[ignore = "ordinary replay of actual learned VOC2 acquisition; no new model or played ACK claim"]
fn actual_vocative_two_survives_append_and_refuses_contradictory_choices() {
    let directory = std::path::PathBuf::from(
        std::env::var("CONDUIT_VOCATIVE_REPLAY_DIRECTORY")
            .expect("immutable acquisition evidence directory"),
    );
    let graphs: serde_json::Value =
        serde_json::from_slice(&std::fs::read(directory.join("original_graphs.json")).unwrap())
            .unwrap();
    let independent = StructuredInfoValue::from_canonical_bytes(
        &std::fs::read(directory.join("native_independent_partial_receipt.bin")).unwrap(),
    )
    .unwrap();
    let insert = LanguageParserProtectedInsertContext::from_structured(
        fixture::field(&independent, "insert").clone(),
    )
    .unwrap();
    let original = LanguageParserProtectedSetProposal::from_structured(
        fixture::field(&independent, "output").clone(),
    )
    .unwrap();
    assert_eq!(*original.active(), [false, false, true, false]);
    assert_eq!(*original.edge2().dependent(), 2);
    assert_eq!(*original.edge2().head(), 0);
    assert_eq!(
        *insert
            .context()
            .fact()
            .query()
            .beam()
            .candidate0()
            .parser()
            .state()
            .committed(),
        0
    );
    eprintln!("VOC2 exact Source check start");
    let text = source();
    let syntax = conduit_plot::parse_syntax_document(&text);
    let checked =
        conduit_plot::check_syntax_document(&syntax, &conduit_plot::StartupCatalog::new()).unwrap();
    let native = |name: &str| {
        checked
            .native_types
            .iter()
            .find(|ty| ty.name == name)
            .unwrap()
    };
    let admit = |name: &str, value: &StructuredInfoValue| {
        let ty = native(name);
        validate_native_contracts(value, &ty.value_contracts)?;
        validate_native_invariants(value, &ty.invariants)
    };
    let mut insert_execution =
        started_execution(text.clone(), "language-parser-protected-set-insert", 100);
    let inserted = LanguageParserProtectedSetProposal::from_structured(
        insert_execution.transact(0, &insert.clone().into_structured().unwrap()),
    )
    .unwrap();
    assert_eq!(inserted, original);
    let rebase = LanguageParserJointRebaseContext::from_structured(bytes(
        &graphs[1]["session"]["outcome"]["rebase_context_bytes"],
    ))
    .unwrap();
    assert_eq!(*rebase.previous().state().committed(), 0);
    assert_eq!(
        rebase
            .previous()
            .lexical()
            .tape()
            .source()
            .material()
            .text(),
        "Hello, Travis "
    );
    assert_eq!(
        rebase.next().tape().source().material().text(),
        "Hello, Travis."
    );
    let ty = native("LanguageParserProtectedSetRebaseContext")
        .value_type
        .clone();
    let tokens = rebase.next().tape().tokens();
    let query = fixture::record(
        &ty,
        vec![
            ("previous", inserted.clone().into_structured().unwrap()),
            ("rebase", rebase.clone().into_structured().unwrap()),
            ("token0", tokens[0].clone().into_structured().unwrap()),
            ("token1", tokens[1].clone().into_structured().unwrap()),
            ("token2", tokens[2].clone().into_structured().unwrap()),
            ("token3", tokens[3].clone().into_structured().unwrap()),
        ],
    );
    admit("LanguageParserProtectedSetRebaseContext", &query).unwrap();
    let mut execution =
        started_execution(text.clone(), "language-parser-protected-set-rebase", 101);
    let rebased =
        LanguageParserProtectedSetProposal::from_structured(execution.transact(0, &query)).unwrap();
    assert_eq!(
        rebased.edge2().origin_basis(),
        original.edge2().origin_basis()
    );
    assert_eq!(rebased.edge2().current_basis(), rebase.basis());
    assert_eq!(rebased.edge2().dependent_occurrence(), tokens[2].identity());
    assert_eq!(rebased.edge2().head_occurrence(), tokens[0].identity());
    assert_eq!(rebased.edge2().relation(), original.edge2().relation());
    assert_eq!(
        rebased.edge2().dependent_choice(),
        original.edge2().dependent_choice()
    );
    assert_eq!(
        rebased.edge2().head_choice(),
        original.edge2().head_choice()
    );
    let beam =
        LanguageParserJointRuntimeBeam::from_structured(bytes(&graphs[1]["session"]["beam_bytes"]))
            .unwrap();
    let hypothesis =
        LanguageParserJointRuntimeHypothesis::new(beam.beam().candidate0().clone(), 4).unwrap();
    let query_ty = native("LanguageParserProtectionForestQuery")
        .value_type
        .clone();
    let forest_query = fixture::record(
        &query_ty,
        vec![
            ("hypothesis", hypothesis.into_structured().unwrap()),
            ("retained", rebased.clone().into_structured().unwrap()),
        ],
    );
    admit("LanguageParserProtectionForestQuery", &forest_query).unwrap();
    let mut forest_execution =
        started_execution(text, "language-parser-protection-forest-projection", 102);
    let proposal = forest_execution.transact(0, &forest_query);
    let forest = joint::retype(
        &native("LanguageParserProtectionForest").value_type,
        &proposal,
    );
    admit("LanguageParserProtectionForest", &forest).unwrap();
    let compatibility = fixture::record(
        &native("LanguageParserProtectedHypothesisCompatibility").value_type,
        vec![("query", forest_query.clone()), ("forest", forest)],
    );
    admit(
        "LanguageParserProtectedHypothesisCompatibility",
        &compatibility,
    )
    .unwrap();
    let original_query = fixture::field(&compatibility, "query");
    let runtime = fixture::field(original_query, "hypothesis");

    for ordinal in [0, 2] {
        let mut choices = beam.beam().candidate0().choices().to_vec();
        choices[ordinal] = if choices[ordinal] == 0 { 1 } else { 0 };
        let changed = LanguageParserJointHypothesis::new(
            choices.try_into().unwrap(),
            beam.beam().candidate0().parser().clone(),
        )
        .unwrap();
        let changed = fixture::replace(runtime, "hypothesis", changed.into_structured().unwrap());
        let refused = fixture::replace(
            &compatibility,
            "query",
            fixture::replace(original_query, "hypothesis", changed),
        );
        assert!(
            matches!(
                admit("LanguageParserProtectedHypothesisCompatibility", &refused),
                Err(NativeBindingRefusal::ViolatedInvariant { .. })
            ),
            "ordinal{ordinal} contradictory choice accepted"
        );
    }
    if let Ok(path) = std::env::var("CONDUIT_VOCATIVE_PROTECTION_REPLAY_OUTPUT") {
        let receipt = serde_json::json!({"schema":"language/parser-v2-independent-vocative-protection-replay@1","scope":"ordinary Source insert/rebase/forest execution on immutable actual model arrivals; no played ACK","independent_admission_bytes":independent.canonical_bytes().unwrap(),"rebase_context_bytes":query.canonical_bytes().unwrap(),"rebased_set_bytes":rebased.into_structured().unwrap().canonical_bytes().unwrap(),"compatible_final_bytes":compatibility.canonical_bytes().unwrap(),"contradictory_choice_refusals":2,"played":false});
        std::fs::write(path, serde_json::to_vec_pretty(&receipt).unwrap()).unwrap();
    }
}
