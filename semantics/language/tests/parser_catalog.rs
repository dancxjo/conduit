use conduit_language::{install_linguistics_catalogs, parser_types};

#[test]
fn symbolic_parser_schemas_are_admitted_through_the_installed_catalog() {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    install_linguistics_catalogs(&mut startup, &mut profile).unwrap();
    for (name, _) in parser_types() {
        let source = format!("plot language/parser-catalog-fixture (\n >> value: {name}\n result: {name} >>\n) = (.)");
        conduit_plot::check_syntax_document(
            &conduit_plot::parse_syntax_document(&source),
            &startup,
        )
        .unwrap();
    }
}
