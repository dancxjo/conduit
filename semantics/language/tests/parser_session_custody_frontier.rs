//! Source schema preflight only; does not claim actual frontier transaction execution.
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};
use std::path::PathBuf;
#[test]
#[ignore = "requires authoritative full parser Source root"]
fn retained_frontier_correlation_source_preflight() {
    let root = PathBuf::from(std::env::var("PARSER_CUSTODY_SOURCE_ROOT").unwrap());
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
            source.push_str(
                &std::fs::read_to_string(root.join(tail.split('"').next().unwrap())).unwrap(),
            );
            source.push('\n');
        }
    }
    source.push_str(include_str!("../parser_session_custody_frontier.conduit"));
    let started = std::time::Instant::now();
    eprintln!("custody frontier: full Source check start");
    let checked =
        check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new()).unwrap();
    for name in [
        "LanguageParserRetainedFactReceipt",
        "LanguageParserRetainedSnapshotReceipt",
        "LanguageParserRetainedCommitAnchor",
        "LanguageParserRetainedCommitReceipt",
    ] {
        let ty = checked
            .native_types
            .iter()
            .find(|t| t.name == name)
            .unwrap();
        eprintln!(
            "custody frontier: {name} Type={}B laws={}",
            ty.value_type.canonical_bytes().unwrap().len(),
            ty.invariants.len()
        );
    }
    eprintln!(
        "custody frontier: Source check complete {}ms",
        started.elapsed().as_millis()
    );
}
