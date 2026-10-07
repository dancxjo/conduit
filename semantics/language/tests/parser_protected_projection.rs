#![cfg(all(feature = "parser-model-selection", target_has_atomic = "ptr"))]
use conduit_core::StructuredInfoValue;
use conduit_language::*;
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};
#[path = "common/parser_fixture.rs"]
mod fixture;
#[path = "common/parser_joint_v2_fixture.rs"]
mod joint;
#[path = "common/parser_kernel.rs"]
mod parser_kernel;
#[path = "common/scorer_model.rs"]
mod scorer_model;
#[test]
#[ignore = "actual Native fact projection through ordinary Source; cold preparation explicit"]
fn compact_projection_preserves_full_fact_endpoints_and_refuses_foreign_tokens() {
    let event = include_str!("../training/ewt_joint_v2/native_stream_protected_events.jsonl")
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .find(|r| r["event"] == "snapshot" && r["source_revision"] == "protected-stream/2")
        .unwrap();
    let fact_bytes: Vec<u8> =
        serde_json::from_value(event["stable_fact_bytes"][0].clone()).unwrap();
    let fact = LanguageParserJointStableFact::from_structured(
        StructuredInfoValue::from_canonical_bytes(&fact_bytes).unwrap(),
    )
    .unwrap();
    let state = fact.query().beam().candidate0().parser().state();
    let dependent = *fact.query().dependent() as usize;
    let head = state.heads()[dependent] as usize;
    let dependent_token = fact.query().beam().lexical().tape().tokens()[dependent].clone();
    let head_token = fact.query().beam().lexical().tape().tokens()[head].clone();
    let wrong_dependent = LanguageParserProtectedProjectionContext::new(
        head_token.clone(),
        fact.clone(),
        head_token.clone(),
    );
    assert!(
        matches!(
            wrong_dependent,
            Err(NativeBindingRefusal::ViolatedInvariant { .. })
        ),
        "wrong dependent outcome: {:?}",
        wrong_dependent.as_ref().err()
    );
    let wrong_head = LanguageParserProtectedProjectionContext::new(
        dependent_token.clone(),
        fact.clone(),
        dependent_token.clone(),
    );
    assert!(
        matches!(
            wrong_head,
            Err(NativeBindingRefusal::ViolatedInvariant { .. })
        ),
        "wrong head outcome: {:?}",
        wrong_head.as_ref().err()
    );
    let context = LanguageParserProtectedProjectionContext::new(
        dependent_token.clone(),
        fact.clone(),
        head_token.clone(),
    )
    .unwrap();
    let input = context.into_structured().unwrap();
    let source = [
        joint::source(),
        include_str!("../parser_session.conduit").into(),
        include_str!("../parser_session_facts.conduit").into(),
        include_str!("../parser_session_protection.conduit").into(),
    ]
    .join("\n");
    let mut execution =
        parser_kernel::Blueprint::prepare(source, "language-parser-protected-edge-projection")
            .realize(0);
    execution.kernel.start().unwrap();
    let output = execution.transact(0, &input);
    let edge = LanguageParserProtectedEdgeProposal::from_structured(output).unwrap();
    assert_eq!(edge.origin_basis(), fact.query().beam().basis());
    assert_eq!(edge.current_basis(), fact.query().beam().basis());
    assert_eq!(*edge.dependent(), dependent as u64);
    assert_eq!(*edge.head(), head as u64);
    assert_eq!(
        *edge.dependent_choice(),
        fact.query().beam().candidate0().choices()[dependent]
    );
    assert_eq!(
        *edge.head_choice(),
        fact.query().beam().candidate0().choices()[head]
    );
    assert_eq!(edge.relation(), state.relation0());
    assert_eq!(edge.dependent_occurrence(), dependent_token.identity());
    assert_eq!(edge.head_occurrence(), head_token.identity());
    if let Ok(path) = std::env::var("CONDUIT_PARSER_PROTECTED_PROJECTION") {
        std::fs::write(path,serde_json::to_vec(&serde_json::json!({
            "scope":"checked Source projection from actual admitted fact; compact output remains a private proposal, never raw caller truth authority",
            "fact_bytes":fact_bytes,"native_context_bytes":input.canonical_bytes().unwrap(),
            "compact_proposal_bytes":edge.into_structured().unwrap().canonical_bytes().unwrap()
        })).unwrap()).unwrap();
    }
}
