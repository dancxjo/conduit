//! Explicit reference commit from an unchanged actual stable admission.
//! This executes Source policy; it is not a new parser/model prediction.
extern crate alloc;
use conduit_language::*;
use conduit_plot::rust_binding::NativeRustBinding;
#[allow(dead_code)]
#[path = "../src/parser_independent_commit.rs"]
mod parser_independent_commit;
use parser_independent_commit::PreparedIndependentCommittedDependency;

#[test]
#[ignore = "requires CONDUIT_PROTECTED_CUSTODY_RECEIPT_PATH exact retained receipt"]
fn source_commit_preserves_other_slots_and_refuses_foreign_basis() {
    let path = std::env::var("CONDUIT_PROTECTED_CUSTODY_RECEIPT_PATH").unwrap();
    let row: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let bytes: Vec<u8> =
        serde_json::from_value(row["independent_admission_bytes"].clone()).unwrap();
    let original = LanguageParserIndependentProtectedAdmission::decode(&bytes).unwrap();
    let protection = original.output();
    let previous = LanguageParserIndependentCommitSet::new(
        [false; 4],
        protection.basis().clone(),
        protection.edge0().clone(),
        protection.edge1().clone(),
        protection.edge2().clone(),
        protection.edge3().clone(),
    )
    .unwrap();
    let request = LanguageParserIndependentCommitRequest::new(
        original.admission().clone(),
        previous.clone(),
        protection.clone(),
    )
    .unwrap();
    let prepared = PreparedIndependentCommittedDependency::prepare(request.clone()).unwrap();
    assert_eq!(prepared.request(), &request);
    let dependent = *original.admission().fact().query().dependent() as usize;
    let mut active = [false; 4];
    active[dependent] = true;
    assert_eq!(*prepared.committed().active(), active);
    assert_eq!(prepared.committed().basis(), previous.basis());
    assert_eq!(prepared.committed().edge0(), previous.edge0());
    assert_eq!(prepared.committed().edge1(), previous.edge1());
    assert_eq!(prepared.committed().edge2(), previous.edge2());
    assert_eq!(prepared.committed().edge3(), previous.edge3());
    assert_eq!(request.encode().unwrap(), prepared.source_input());
    let raw = LanguageParserIndependentCommitSetProposal::decode(prepared.source_output()).unwrap();
    assert_eq!(raw.active(), prepared.committed().active());
    assert!(prepared.program().canonical_bytes().unwrap().len() > 2_000_000);
    // The unchanged original remains an actual acquisition, not this reference commit.
    assert_eq!(original.clone().encode().unwrap(), bytes);
    let rebased: Vec<u8> = serde_json::from_value(row["rebased_set_bytes"].clone()).unwrap();
    let foreign = LanguageParserProtectedSetProposal::decode(&rebased).unwrap();
    assert_ne!(foreign.basis(), protection.basis());
    assert!(LanguageParserIndependentCommitRequest::new(
        original.admission().clone(),
        previous,
        foreign,
    )
    .is_err());
}
