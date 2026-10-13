//! Generated Native retained-commit custody test draft. Immutable actual trace,
//! no model execution or retrospective played-frontier claim.
extern crate alloc;
use conduit_language::*;
use conduit_plot::rust_binding::NativeRustBinding;
#[path = "../src/parser_retained_commit.rs"]
mod parser_retained_commit;
use parser_retained_commit::PreparedRetainedCommit;

fn bytes(row: &serde_json::Value) -> Vec<u8> {
    serde_json::from_value(row.clone()).expect("whole canonical native bytes")
}
#[test]
#[ignore = "requires immutable actual trace via CONDUIT_RETAINED_FRONTIER_TRACE_PATH"]
fn exact_full_commit_query_projection_and_source_receipt_retain_original_evidence() {
    let path = std::env::var("CONDUIT_RETAINED_FRONTIER_TRACE_PATH").unwrap();
    let text = std::fs::read_to_string(path).unwrap();
    let row = text
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .find(|row| {
            row["event"] == "snapshot" && row["text"] == "Travis Hello " && row["committed"] == 1
        })
        .expect("exact actual postcommit snapshot");
    let fact_bytes = bytes(&row["stable_fact_bytes"][0]);
    let fact = LanguageParserJointStableFact::decode(&fact_bytes).unwrap();
    let before = fact.query().beam().clone();
    assert_eq!(*before.candidate0().parser().state().committed(), 0);
    let query = LanguageParserJointCommitQuery::new(fact).unwrap();
    let query_bytes = query.clone().encode().unwrap();
    let prepared = PreparedRetainedCommit::prepare(query).unwrap();
    assert_eq!(prepared.source_input(), query_bytes);
    assert_eq!(*prepared.anchor().dependent(), 0);
    assert_eq!(prepared.anchor().basis(), before.basis());
    assert_eq!(
        prepared
            .program()
            .evaluate(prepared.source_input())
            .unwrap(),
        prepared.source_output()
    );
    assert_eq!(
        prepared.query().fact().clone().encode().unwrap(),
        fact_bytes
    );
    let runtime = LanguageParserJointRuntimeBeam::decode(&bytes(&row["beam_bytes"])).unwrap();
    let next = runtime.beam().clone();
    let receipt = prepared.admit_output(next.clone()).unwrap();
    assert_eq!(receipt.previous(), &before);
    assert_eq!(receipt.next(), &next);
    assert_eq!(receipt.anchor(), prepared.anchor());
    assert!(prepared.admit_output(before).is_err());
    let receipt_bytes = receipt.clone().encode().unwrap();
    assert_eq!(
        LanguageParserRetainedCommitReceipt::decode(&receipt_bytes).unwrap(),
        receipt
    );
    assert_eq!(
        prepared.query().fact().clone().encode().unwrap(),
        fact_bytes
    );
}
