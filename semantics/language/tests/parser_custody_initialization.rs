//! Exact fixed Source initializer execution over a retained actual sentinel.
//! The initializer does not run a parser, advance commitment, or acknowledge PCM.
extern crate alloc;
use conduit_language::*;
use conduit_plot::rust_binding::NativeRustBinding;
#[path = "../src/parser_custody_initialization.rs"]
mod parser_custody_initialization;
use parser_custody_initialization::PreparedCustodyInitialization;

#[test]
#[ignore = "requires retained actual acquisition JSON via CONDUIT_PROTECTED_CUSTODY_RECEIPT_PATH"]
fn source_initializer_retains_exact_execution_and_refuses_active_edge_as_empty_origin() {
    let path = std::env::var("CONDUIT_PROTECTED_CUSTODY_RECEIPT_PATH").unwrap();
    let row: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let original_bytes: Vec<u8> =
        serde_json::from_value(row["independent_admission_bytes"].clone()).unwrap();
    let original = LanguageParserIndependentProtectedAdmission::decode(&original_bytes).unwrap();
    let sentinel = original.insert().previous().edge0().clone();
    assert_eq!(*sentinel.dependent(), 4);
    let prepared = PreparedCustodyInitialization::prepare(sentinel.clone()).unwrap();
    assert_eq!(prepared.receipt().input(), &sentinel);
    assert_eq!(*prepared.receipt().output().active(), [false; 4]);
    assert_eq!(prepared.source_input(), sentinel.encode().unwrap());
    assert_eq!(
        prepared.source_output(),
        prepared.receipt().output().clone().encode().unwrap()
    );
    assert_eq!(
        prepared
            .program()
            .evaluate(prepared.source_input())
            .unwrap(),
        prepared.source_output()
    );
    // The same checked initializer can project an active edge, but that result
    // cannot be admitted as the Session's empty initialization receipt.
    let active = original.output().edge2().clone();
    assert_eq!(*active.dependent(), 2);
    assert!(PreparedCustodyInitialization::prepare(active).is_err());
    assert_eq!(original.encode().unwrap(), original_bytes);
}
