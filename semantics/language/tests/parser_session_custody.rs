#![cfg(all(feature = "parser-model-selection", target_has_atomic = "ptr"))]
use conduit_core::StructuredInfoValue;
use conduit_language::*;
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};
#[path = "common/parser_fixture.rs"]
mod fixture;
#[path = "common/parser_joint_v2_fixture.rs"]
mod joint;
#[allow(dead_code)]
#[path = "common/parser_joint_flows.rs"]
mod parser_joint_flows;
#[path = "common/parser_kernel.rs"]
mod parser_kernel;
#[path = "common/scorer_model.rs"]
mod scorer_model;
fn source() -> String {
    [
        joint::source(),
        include_str!("../parser_revision.conduit").into(),
        include_str!("../parser_session.conduit").into(),
        include_str!("../parser_session_policy.conduit").into(),
        include_str!("../parser_session_facts.conduit").into(),
        include_str!("../parser_session_commit.conduit").into(),
        include_str!("../parser_session_rebase.conduit").into(),
    ]
    .join("\n")
}
fn bytes(row: &serde_json::Value) -> Vec<u8> {
    serde_json::from_value(row.clone()).unwrap()
}
#[test]
#[ignore = "ordinary Source custody replay; explicit cold preparation and immutable actual stream inputs"]
fn native_rebase_and_head_choice_custody_replay() {
    let events = include_str!("../training/ewt_joint_v2/native_stream_protected_events.jsonl")
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .filter(|event| {
            event["event"] == "snapshot" && event["outcome"]["rebase_context_bytes"].is_array()
        })
        .collect::<Vec<_>>();
    let rebase = parser_kernel::Blueprint::prepare(source(), "language-parser-joint-rebase");
    let branch = parser_kernel::Blueprint::prepare(
        format!(
            "{}\n{}",
            joint::runtime_source(),
            include_str!("../parser_session_branch.conduit")
        ),
        "language-parser-joint-protected-branch",
    );
    let mut flows = parser_joint_flows::Pipelines::new(&[rebase, branch], 0);
    let mut receipts = Vec::new();
    for event in events {
        let input_bytes = bytes(&event["outcome"]["rebase_context_bytes"]);
        let context = LanguageParserJointRebaseContext::from_structured(
            StructuredInfoValue::from_canonical_bytes(&input_bytes).unwrap(),
        )
        .unwrap();
        let output = flows.call(0, &context.clone().into_structured().unwrap());
        let proposal = LanguageParserJointRebaseProposal::from_structured(output).unwrap();
        let state = LanguageParserState::from_structured(joint::retype(
            &LanguageParserState::semantic_type().unwrap(),
            &proposal.state().clone().into_structured().unwrap(),
        ))
        .unwrap();
        assert_eq!(
            state
                .clone()
                .into_structured()
                .unwrap()
                .canonical_bytes()
                .unwrap(),
            bytes(&event["outcome"]["rebase_state_bytes"])
        );
        let lexical = LanguageParserJointLexical::new(
            context.next().tape().clone(),
            *context.next().token_count(),
        )
        .unwrap();
        // Synthetic identity/score are explicit: this replays preservation policy,
        // not the original branch transcript or learned numerical inference.
        let seed = LanguageParserJointRuntimeHypothesis::new(
            LanguageParserJointHypothesis::new(
                *proposal.choices(),
                LanguageParserHypothesis::new(true, 0, 0, state.clone()).unwrap(),
            )
            .unwrap(),
            *proposal.selected(),
        )
        .unwrap();
        let ty = LanguageParserJointProtectedBranchQuery::semantic_type().unwrap();
        let mut branches = Vec::new();
        for choice in 0..4 {
            let query =
                LanguageParserJointBranchQuery::new(choice, seed.clone(), lexical.clone()).unwrap();
            let raw = fixture::record(&ty, vec![("branch", query.into_structured().unwrap())]);
            let raw_bytes = raw.canonical_bytes().unwrap();
            match LanguageParserJointProtectedBranchQuery::from_structured(raw) {
                Ok(query) => {
                    let output = flows.call(1, &query.into_structured().unwrap());
                    let result = LanguageParserJointBranchResult::from_structured(output).unwrap();
                    if *state.committed() > 0 {
                        assert_eq!(choice, proposal.choices()[*state.unread() as usize]);
                        assert!(*result.accepted());
                    }
                    branches.push(serde_json::json!({"choice":choice,"native_admitted":true,
                        "source_accepted":result.accepted(),"input_bytes":raw_bytes,
                        "output_bytes":result.into_structured().unwrap().canonical_bytes().unwrap()}));
                }
                Err(NativeBindingRefusal::ViolatedInvariant { index }) => {
                    assert!(*state.committed() > 0);
                    assert_ne!(choice, proposal.choices()[*state.unread() as usize]);
                    branches.push(serde_json::json!({"choice":choice,"native_admitted":false,
                        "Source_law_index":index,"raw_refused_input_bytes":raw_bytes}));
                }
                Err(error) => panic!("unexpected custody admission: {error:?}"),
            }
        }
        receipts.push(serde_json::json!({"source_revision":event["source_revision"],
            "scope":"ordinary Source rebase replay from actual admitted stream; synthetic branch identity0/score0; no learned or played claim",
            "rebase_input_bytes":input_bytes,"rebase_proposal_bytes":proposal.into_structured().unwrap().canonical_bytes().unwrap(),
            "native_rebase_state_bytes":state.into_structured().unwrap().canonical_bytes().unwrap(),"branches":branches}));
    }
    assert_eq!(receipts.len(), 3);
    assert_eq!(flows.flows[0].sequence, 3);
    if let Ok(path) = std::env::var("CONDUIT_PARSER_CUSTODY_RECEIPTS") {
        std::fs::write(path, serde_json::to_vec(&receipts).unwrap()).unwrap();
    }
}
