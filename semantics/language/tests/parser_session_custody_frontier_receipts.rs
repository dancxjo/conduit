//! Replay of immutable actual receipts; no new model or playback execution.
use conduit_core::{
    ConfigurationValue, StructuredFieldValue, StructuredInfoValue, StructuredInfoValueShape,
};
use conduit_plot::rust_binding::validate_native_invariants;
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};
use std::path::PathBuf;
#[test]
#[ignore = "requires full Source root and actual contiguous-commit event trace"]
fn exact_actual_fact_snapshot_and_commit_correlation() {
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
    if !block.contains("parser_session_custody_frontier.conduit") {
        source.push_str(include_str!("../parser_session_custody_frontier.conduit"));
    }
    eprintln!("frontier replay: Source check start");
    let checked =
        check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new()).unwrap();
    eprintln!("frontier replay: Source checked");
    let events =
        std::fs::read_to_string(std::env::var("PARSER_CUSTODY_SESSION_EVENTS").unwrap()).unwrap();
    let row = events
        .lines()
        .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
        .find(|r| r["event"] == "snapshot" && r["text"] == "Travis Hello " && r["committed"] == 1)
        .unwrap();
    let decode = |v: &serde_json::Value| {
        let bytes: Vec<u8> = serde_json::from_value(v.clone()).unwrap();
        StructuredInfoValue::from_canonical_bytes(&bytes).unwrap()
    };
    let fact = decode(&row["stable_fact_bytes"][0]);
    let previous = field(field(&fact, "query"), "beam").clone();
    let next = field(&decode(&row["beam_bytes"]), "beam").clone();
    let ty = |name: &str| {
        checked
            .native_types
            .iter()
            .find(|t| t.name == name)
            .unwrap()
    };
    let record = |name: &str, fields: Vec<(&str, StructuredInfoValue)>| {
        StructuredInfoValue::record(
            ty(name).value_type.clone(),
            fields
                .into_iter()
                .map(|(n, v)| StructuredFieldValue::new(n, v).unwrap())
                .collect(),
        )
        .unwrap()
    };
    let admit = |name: &str, value: &StructuredInfoValue| {
        validate_native_invariants(value, &ty(name).invariants).unwrap();
        eprintln!(
            "frontier replay: {name} {}B admitted",
            value.canonical_bytes().unwrap().len()
        );
    };
    let retained_fact = record(
        "LanguageParserRetainedFactReceipt",
        vec![("fact", fact.clone()), ("beam", previous.clone())],
    );
    admit("LanguageParserRetainedFactReceipt", &retained_fact);
    let foreign_fact = record(
        "LanguageParserRetainedFactReceipt",
        vec![("fact", fact.clone()), ("beam", next.clone())],
    );
    assert!(validate_native_invariants(
        &foreign_fact,
        &ty("LanguageParserRetainedFactReceipt").invariants
    )
    .is_err());
    let lexical = field(&next, "lexical");
    let available_lexical = record(
        "LanguageParserAvailableLexical",
        vec![
            ("tape", field(lexical, "tape").clone()),
            ("token_count", field(lexical, "token_count").clone()),
        ],
    );
    admit("LanguageParserAvailableLexical", &available_lexical);
    let state = field(field(field(&next, "candidate0"), "parser"), "state");
    let available = record(
        "LanguageParserAvailableState",
        vec![("lexical", available_lexical), ("state", state.clone())],
    );
    admit("LanguageParserAvailableState", &available);
    let snapshot = record(
        "LanguageParserRetainedSnapshotReceipt",
        vec![("beam", next.clone()), ("available", available.clone())],
    );
    admit("LanguageParserRetainedSnapshotReceipt", &snapshot);
    let stale_snapshot = record(
        "LanguageParserRetainedSnapshotReceipt",
        vec![("beam", previous.clone()), ("available", available)],
    );
    assert!(validate_native_invariants(
        &stale_snapshot,
        &ty("LanguageParserRetainedSnapshotReceipt").invariants
    )
    .is_err());
    let query = record("LanguageParserJointCommitQuery", vec![("fact", fact)]);
    admit("LanguageParserJointCommitQuery", &query);
    let expanded = conduit_plot::expand_canonical_plot_for_authoring(
        &checked,
        "language-parser-retained-commit-anchor",
        &conduit_plot::ProfileCatalog::new(),
    )
    .unwrap();
    assert_eq!(expanded.expanded.gears.len(), 1);
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("program")
    };
    let program = conduit_plot::PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    let proposal = StructuredInfoValue::from_canonical_bytes(
        &program.evaluate(&query.canonical_bytes().unwrap()).unwrap(),
    )
    .unwrap();
    let anchor = record(
        "LanguageParserRetainedCommitAnchor",
        vec![
            ("basis", field(&proposal, "basis").clone()),
            ("dependent", field(&proposal, "dependent").clone()),
        ],
    );
    admit("LanguageParserRetainedCommitAnchor", &anchor);
    let receipt = record(
        "LanguageParserRetainedCommitReceipt",
        vec![
            ("previous", previous.clone()),
            ("next", next),
            ("anchor", anchor.clone()),
        ],
    );
    admit("LanguageParserRetainedCommitReceipt", &receipt);
    let unchanged = record(
        "LanguageParserRetainedCommitReceipt",
        vec![
            ("previous", previous.clone()),
            ("next", previous),
            ("anchor", anchor),
        ],
    );
    assert!(validate_native_invariants(
        &unchanged,
        &ty("LanguageParserRetainedCommitReceipt").invariants
    )
    .is_err());
    for value in [retained_fact, snapshot, receipt] {
        assert_eq!(
            value,
            StructuredInfoValue::from_canonical_bytes(&value.canonical_bytes().unwrap()).unwrap()
        );
    }
}
fn field<'a>(value: &'a StructuredInfoValue, name: &str) -> &'a StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("record")
    };
    fields.iter().find(|f| f.name() == name).unwrap().value()
}
