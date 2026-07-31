use conduit_panel::{SOURCE_AST_SCHEMA_VERSION, parse_document};
use serde_json::Value;

const FIXTURES: &str = include_str!("../../../conformance/c3/panel-directional-syntax.json");

#[test]
fn every_directional_syntax_vector_has_the_exact_result() {
    let suite: Value = serde_json::from_str(FIXTURES).unwrap();
    assert_eq!(suite["grammar_version"], 0);
    assert_eq!(
        suite["source_ast_schema_version"],
        u64::from(SOURCE_AST_SCHEMA_VERSION)
    );

    for case in suite["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let source = case["source"].as_str().unwrap();
        let document = parse_document(source);
        assert_eq!(document.round_trip(), source, "{id}: source drift");
        let actual = document
            .panel()
            .map(|_| ("accepted", "-"))
            .unwrap_or_else(|error| ("rejected", error.code));
        assert_eq!(actual.0, case["outcome"], "{id}: outcome");
        assert_eq!(actual.1, case["code"], "{id}: diagnostic");
        if actual.0 == "accepted" {
            assert!(document.semantic_hash().is_some(), "{id}: semantic hash");
        }
    }
}
