//! Original finite commitment Source extraction, before Session/runtime wiring.
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
#[test]
fn original_commit_source_checks_and_expands_on_current_language_contracts() {
    let source = [
        include_str!("../types.conduit"),
        include_str!("../identity.conduit"),
        include_str!("../coverage.conduit"),
        include_str!("../syntax.conduit"),
        include_str!("../text_revision.conduit"),
        include_str!("../revision_lineage.conduit"),
        include_str!("../lexical.conduit"),
        include_str!("../parser.conduit"),
        include_str!("../parser_beam.conduit"),
        include_str!("../parser_available.conduit"),
        include_str!("../discourse.conduit"),
        include_str!("../prosody.conduit"),
        include_str!("common/commit_source/parser_joint.conduit"),
        include_str!("common/commit_source/parser_joint_decode.conduit"),
        include_str!("common/commit_source/parser_session.conduit"),
        include_str!("common/commit_source/parser_session_commit.conduit"),
        include_str!("common/commit_source/parser_session_committed_dependency.conduit"),
        include_str!("common/commit_source/parser_session_dependency.conduit"),
        include_str!("common/commit_source/parser_session_facts.conduit"),
    ]
    .join("\n");
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let expanded = expand_canonical_plot_for_authoring(
        &checked,
        "language-parser-joint-commit",
        &ProfileCatalog::new(),
    )
    .unwrap();
    assert_eq!(expanded.expanded.gears.len(), 1);
    assert_eq!(expanded.expanded.gears[0].configuration.len(), 1);
}
