#![cfg(feature = "semantic-bindings")]
//! Inventory authoring retains exact Language-owned laws and Native identity.
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    rust_binding::NativeRustBinding, PortableExpressionProgram, ProfileCatalog, StartupCatalog,
};

fn compile(source: &str, catalog: &StartupCatalog) -> Result<PortableExpressionProgram, String> {
    let syntax = parse_syntax_document(source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, catalog).map_err(|e| format!("{e:?}"))?;
    let expanded = expand_canonical_plot_for_authoring(&checked, "basis", &ProfileCatalog::new())
        .map_err(|e| format!("{e:?}"))?;
    let conduit_core::ConfigurationValue::Text(hex) =
        &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("portable constructor")
    };
    PortableExpressionProgram::from_canonical_hex(hex).map_err(|e| format!("{e:?}"))
}

#[test]
fn inventory_basis_import_retains_language_laws_and_owner_abi() {
    // Match the product's pre-existing shape-only registrations.
    let mut catalog = StartupCatalog::new();
    for (name, ty) in conduit_language::identity_types() {
        catalog.insert_structured_type(name, ty).unwrap();
    }
    conduit_speech::authoring::install(&mut catalog).unwrap();
    let source = "with language/LanguageVariety as Variety\nplot basis (\n >> input: Boolean\n result: Variety >>\n) = ({identity: \"variety/test\", language: \"language/test\"})\n";
    let program = compile(source, &catalog).unwrap();
    let bytes = program.evaluate(&[1]).unwrap();
    let variety = conduit_language::LanguageVariety::decode(&bytes).unwrap();
    assert_eq!(variety.identity().get(), "variety/test");
    assert_eq!(variety.language().get(), "language/test");
    for invalid in [
        source.replace("\"variety/test\"", "\"\""),
        source.replace("\"language/test\"", "\"\""),
    ] {
        assert!(compile(&invalid, &catalog).is_err());
    }
    let range = "with language/LanguageTextRange as Range\nplot basis (\n >> input: Boolean\n result: Range >>\n) = ({start: 2, end: 1})\n";
    assert!(compile(range, &catalog).is_err());
    assert!(compile(&range.replace("end: 1", "end: 2"), &catalog).is_ok());
}
