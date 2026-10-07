use conduit_language::*;
use conduit_plot::rust_binding::NativeRustBinding;
#[path = "common/parser_kernel.rs"]
mod parser_kernel;

#[test]
fn eight_hop_source_walk_runs_in_ordinary_plan_and_play() {
    let source = [
        include_str!("../identity.conduit"),
        include_str!("../types.conduit"),
        include_str!("../text_revision.conduit"),
        include_str!("../lexical.conduit"),
        include_str!("../parser.conduit"),
        include_str!("../parser_mask.conduit"),
        include_str!("../parser_window8.conduit"),
    ]
    .join("\n");
    let mut run = parser_kernel::Execution::prepare(source, "language-window8-walk");
    run.kernel.start().unwrap();
    let heads = [8, 0, 1, 2, 3, 4, 5, 6, 8];
    let query = LanguageParserWindow8WalkQuery::new(heads, 7).unwrap();
    let output = run.transact(0, &query.into_structured().unwrap());
    let walk = LanguageParserWindow8RawWalk::from_structured(output).unwrap();
    assert_eq!(*walk.steps(), 8);
    let witness = LanguageParserWindow8Ancestry::new(
        *walk.query().heads(),
        *walk.path(),
        *walk.query().start(),
    )
    .unwrap();
    assert_eq!(witness.path(), &[7, 6, 5, 4, 3, 2, 1, 0, 8]);
    let cyclic = LanguageParserWindow8WalkQuery::new([1, 2, 3, 4, 5, 6, 7, 0, 8], 7).unwrap();
    let output = run.transact(1, &cyclic.into_structured().unwrap());
    let raw = LanguageParserWindow8RawWalk::from_structured(output).unwrap();
    assert_eq!(raw.path(), &[7, 0, 1, 2, 3, 4, 5, 6, 7]);
    assert!(LanguageParserWindow8Ancestry::new(
        *raw.query().heads(),
        *raw.path(),
        *raw.query().start(),
    )
    .is_err());
}

#[test]
fn window8_schemas_are_admitted_by_the_installed_catalog() {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    install_linguistics_catalogs(&mut startup, &mut profiles).unwrap();
    for (name, _) in parser_window8::window8_types() {
        let source = format!(
            "plot language/window8-catalog (\n >> value: {name}\n result: {name} >>\n) = (.)"
        );
        conduit_plot::check_syntax_document(
            &conduit_plot::parse_syntax_document(&source),
            &startup,
        )
        .unwrap();
    }
}
