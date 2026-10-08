//! Source/codec preflight only; no model or committed frontier execution claim.
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};

#[test]
#[ignore = "full Source independent-commit preflight"]
fn independent_commit_source_contracts_and_programs_fit_native_bounds() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../semantics/language");
    let build = std::fs::read_to_string(root.join("build.rs")).unwrap();
    let block = build
        .split("let source = [")
        .nth(1)
        .unwrap()
        .split("]\n    .join")
        .next()
        .unwrap();
    let mut source = String::new();
    for line in block.lines() {
        if let Some(tail) = line.split("include_str!(\"").nth(1) {
            let name = tail.split('"').next().unwrap();
            source.push_str(&std::fs::read_to_string(root.join(name)).unwrap());
            source.push('\n');
        }
    }
    for name in [
        "parser_session_custody_initialize.conduit",
        "parser_session_independent_commit.conduit",
        "parser_session_independent_commit_rebase.conduit",
    ] {
        if !block.contains(&format!("include_str!(\"{name}\")")) {
            source.push_str(&std::fs::read_to_string(root.join(name)).unwrap());
            source.push('\n');
        }
    }
    let checked =
        check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new()).unwrap();
    for name in [
        "LanguageParserProtectedInitializationReceipt",
        "LanguageParserIndependentCommitSet",
        "LanguageParserIndependentCommitSetProposal",
        "LanguageParserIndependentCommitRequest",
        "LanguageParserIndependentCommitRebaseSets",
        "LanguageParserIndependentCommitRebaseRequest",
    ] {
        let ty = checked
            .native_types
            .iter()
            .find(|t| t.name == name)
            .unwrap();
        let bytes = ty.value_type.canonical_bytes().unwrap();
        assert!(bytes.len() <= 65_536, "{name}: {}", bytes.len());
        eprintln!(
            "independent commit native {name}: {} Type bytes",
            bytes.len()
        );
    }
    for entry in [
        "language-parser-independent-commit",
        "language-parser-independent-commit-rebase-sets",
        "language-parser-independent-commit-rebase",
    ] {
        let expanded = conduit_plot::expand_canonical_plot_for_authoring(
            &checked,
            entry,
            &conduit_plot::ProfileCatalog::new(),
        )
        .unwrap();
        assert_eq!(expanded.expanded.gears.len(), 1);
        let conduit_core::ConfigurationValue::Text(encoded) =
            &expanded.expanded.gears[0].configuration[0].value
        else {
            panic!("owned expression program")
        };
        let program = conduit_plot::PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
        let bytes = program.canonical_bytes().unwrap();
        eprintln!("independent commit {entry}: {} program bytes", bytes.len());
    }
}
