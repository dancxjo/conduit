//! Generated runtime bytes must equal the independently checked Source Types.
use super::contract::*;
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};
#[test]
fn generated_clock_types_preserve_exact_checked_abi() {
    let checked =
        check_syntax_document(&parse_syntax_document(CLOCK_TYPES), &StartupCatalog::new()).unwrap();
    let contract = MonotonicClockContract::prepare().unwrap();
    for (name, generated) in [
        ("MonotonicClockRequest", contract.request_type()),
        ("MonotonicClockResult", contract.result_type()),
    ] {
        let original = &checked
            .native_types
            .iter()
            .find(|ty| ty.name == name)
            .unwrap()
            .value_type;
        assert_eq!(
            generated.canonical_bytes().unwrap(),
            original.canonical_bytes().unwrap()
        );
    }
    contract.kind().validate().unwrap();
    let _ = contract.catalogs();
}
