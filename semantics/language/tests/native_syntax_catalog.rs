use conduit_language::install_linguistics_catalogs;
use conduit_plot::{check_syntax_document, parse_syntax_document, ProfileCatalog, StartupCatalog};
#[test]
fn language_owned_syntax_is_available_to_ordinary_authored_plots() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_linguistics_catalogs(&mut startup, &mut profile).unwrap();
    let source = "plot language/syntax-fixture (\n    >> value: LinguisticSyntacticLinkKind\n    result: LinguisticSyntacticLinkKind >>\n) = (LinguisticSyntacticLinkKind.vocative)";
    check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
}
