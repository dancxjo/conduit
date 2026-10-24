//! Checked Source fixture shared by prepared-expression conformance.
use conduit_plot::*;
pub fn compile(source: &str, entry: &str) -> PortableExpressionProgram {
    let syntax = parse_syntax_document(source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, entry, &ProfileCatalog::new()).unwrap();
    let conduit_core::ConfigurationValue::Text(encoded) =
        &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("expression");
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
