use std::collections::{BTreeMap, BTreeSet};

use conduit_panel::{LoadedModule, ModuleLoader, parse_document_with_root, resolve_modules};
use serde_json::Value;

const FIXTURES: &str = include_str!("../../../conformance/c3/panel-grammar.json");

struct MemoryLoader(BTreeMap<String, String>);

impl ModuleLoader for MemoryLoader {
    fn load(&self, canonical_uri: &str) -> Result<Option<LoadedModule>, String> {
        Ok(self.0.get(canonical_uri).map(|source| LoadedModule {
            canonical_uri: canonical_uri.to_owned(),
            source: source.clone(),
        }))
    }
}

#[test]
fn every_grammar_vector_has_the_exact_outcome_and_diagnostic() {
    let suite: Value = serde_json::from_str(FIXTURES).unwrap();
    assert_eq!(suite["grammar_version"], 0);
    for case in suite["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let source = case["source"].as_str().unwrap();
        let selected_root = case["selected_root"].as_str();
        let expected_outcome = case["outcome"].as_str().unwrap();
        let expected_code = case["code"].as_str().unwrap();
        let outcome = match case["mode"].as_str().unwrap() {
            "parse" => {
                let document = parse_document_with_root(source, selected_root);
                assert_eq!(document.round_trip(), source, "{id}: source drift");
                assert_lossless_spans(&document.tokens, source, id);
                if case["round_trip"] == true {
                    assert!(document.ast.is_some(), "{id}: expected round-trip AST");
                }
                if let Some(equivalent) = case["equivalent_source"].as_str() {
                    let equivalent = parse_document_with_root(equivalent, selected_root);
                    assert_eq!(
                        document.semantic_hash(),
                        equivalent.semantic_hash(),
                        "{id}: formatting changed source semantic identity"
                    );
                }
                document
                    .panel()
                    .map(|_| ("accepted", "-"))
                    .unwrap_or_else(|error| ("rejected", error.code))
            }
            "resolve" => {
                let entry_uri = case["entry_uri"].as_str().unwrap();
                let mut sources = BTreeMap::from([(entry_uri.to_owned(), source.to_owned())]);
                for (uri, source) in case["modules"].as_object().unwrap() {
                    sources.insert(uri.clone(), source.as_str().unwrap().to_owned());
                }
                resolve_modules(entry_uri, selected_root, &MemoryLoader(sources))
                    .map(|_| ("accepted", "-"))
                    .unwrap_or_else(|error| ("rejected", error.code))
            }
            mode => panic!("{id}: unknown mode {mode}"),
        };
        assert_eq!(outcome.0, expected_outcome, "{id}: outcome");
        assert_eq!(outcome.1, expected_code, "{id}: diagnostic");
    }
}

#[test]
fn fixtures_cover_every_normative_grammar_production() {
    let suite: Value = serde_json::from_str(FIXTURES).unwrap();
    let covered: BTreeSet<&str> = suite["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| case["outcome"] == "accepted")
        .flat_map(|case| case["productions"].as_array().unwrap())
        .map(|production| production.as_str().unwrap())
        .collect();
    let required = BTreeSet::from([
        "admission",
        "binding",
        "boolean",
        "cleanup",
        "comment",
        "constraint",
        "cord",
        "cord-policy",
        "definition",
        "document",
        "endpoint",
        "export",
        "group-member",
        "import",
        "indexed-group",
        "instance",
        "integer",
        "keyed-group",
        "literal-call",
        "module-resolution",
        "number",
        "parameter",
        "parameter-list",
        "pool",
        "pool-policy",
        "port-group",
        "pressure-policy",
        "qualified-name",
        "record-field",
        "root",
        "string",
        "source-value",
        "supervision",
        "trivia",
        "version",
    ]);
    assert_eq!(covered, required);
}

#[test]
fn every_positive_production_seed_has_lossless_malformed_recovery() {
    let suite: Value = serde_json::from_str(FIXTURES).unwrap();
    for case in suite["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| case["outcome"] == "accepted")
    {
        let id = case["id"].as_str().unwrap();
        let mut malformed = case["source"].as_str().unwrap().to_owned();
        malformed.push('\0');
        let document = parse_document_with_root(&malformed, case["selected_root"].as_str());
        assert!(document.ast.is_none(), "{id}: malformed mutation parsed");
        assert_eq!(document.round_trip(), malformed);
        assert_lossless_spans(&document.tokens, &malformed, id);
    }
}

fn assert_lossless_spans(tokens: &[conduit_panel::CstToken], source: &str, case: &str) {
    let mut offset = 0;
    for token in tokens {
        assert_eq!(token.span.start, offset, "{case}: CST gap");
        assert_eq!(&source[token.span.start..token.span.end], token.text);
        offset = token.span.end;
    }
    assert_eq!(offset, source.len(), "{case}: CST truncation");
}
