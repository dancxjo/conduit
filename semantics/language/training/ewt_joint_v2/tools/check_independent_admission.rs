fn main() {
    let source = std::env::args()
        .skip(1)
        .map(|path| std::fs::read_to_string(path).unwrap())
        .collect::<Vec<_>>()
        .join("\n");
    let syntax = conduit_plot::parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked =
        conduit_plot::check_syntax_document(&syntax, &conduit_plot::StartupCatalog::new()).unwrap();
    let native = checked
        .native_types
        .iter()
        .find(|ty| ty.name == "LanguageParserIndependentProtectedAdmission")
        .unwrap();
    for (index, program) in native.invariants.iter().enumerate() {
        let bytes = program.canonical_bytes().unwrap();
        conduit_plot::PortableExpressionProgram::from_canonical_bytes(&bytes)
            .unwrap_or_else(|error| panic!("invariant{index}: {error:?}"));
    }
    println!(
        "{} actual Source invariant programs round-trip",
        native.invariants.len()
    );
}
