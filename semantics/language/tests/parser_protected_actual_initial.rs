//! Re-admit the actual first insertion without changing its predecessor or body.
extern crate alloc;
use conduit_language::*;
use conduit_plot::rust_binding::NativeRustBinding;
// This focused initial gate intentionally does not exercise every origin getter.
#[allow(dead_code)]
#[path = "../src/parser_protected_origin.rs"]
mod parser_protected_origin;
use parser_protected_origin::PreparedProtectedOrigin;

#[test]
#[ignore = "requires actual flushed first-origin bytes"]
fn actual_empty_predecessor_first_insertion_has_source_initial_authority() {
    let path = std::env::var("CONDUIT_ACTUAL_FIRST_ORIGIN_PATH").unwrap();
    let bytes = std::fs::read(path).unwrap();
    let original = LanguageParserIndependentProtectedAdmission::decode(&bytes).unwrap();
    assert_eq!(*original.insert().previous().active(), [false; 4]);
    let prepared = PreparedProtectedOrigin::prepare(original).unwrap();
    let initial = prepared.initial().unwrap();
    assert_eq!(
        initial.insertion().previous(),
        prepared.original().insert().previous()
    );
    assert_eq!(initial.insertion().output(), prepared.original().output());
    assert_eq!(prepared.original().clone().encode().unwrap(), bytes);
    eprintln!("actual first origin: {} bytes, exact empty predecessor and Source initial receipt admitted", bytes.len());
}
