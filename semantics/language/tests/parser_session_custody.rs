//! Explicit receipt replay; no model execution or played commitment is claimed.
use conduit_core::{StructuredFieldValue, StructuredInfoValue, StructuredInfoValueShape};
use conduit_plot::rust_binding::validate_native_invariants;
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};
use std::path::PathBuf;

#[test]
#[ignore = "requires exact full-source root and actual protected rebase receipt"]
fn full_source_correlates_actual_rebase_output_and_refuses_foreign_previous_set() {
    let started = std::time::Instant::now();
    eprintln!("custody: full-source load start");
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
            let file = tail.split('"').next().unwrap();
            source.push_str(&std::fs::read_to_string(root.join(file)).unwrap());
            source.push('\n');
        }
    }
    source.push_str(include_str!("../parser_session_custody.conduit"));
    let checked =
        check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new()).unwrap();
    eprintln!(
        "custody: full-source checked {}ms",
        started.elapsed().as_millis()
    );
    let ty = checked
        .native_types
        .iter()
        .find(|t| t.name == "LanguageParserProtectedRebaseReceipt")
        .unwrap();
    let row: serde_json::Value = serde_json::from_slice(
        &std::fs::read(std::env::var("PARSER_CUSTODY_REBASE_RECEIPT").unwrap()).unwrap(),
    )
    .unwrap();
    let read = |name: &str| {
        let bytes: Vec<u8> = serde_json::from_value(row[name].clone()).unwrap();
        StructuredInfoValue::from_canonical_bytes(&bytes).unwrap()
    };
    let context = read("rebase_context_bytes");
    let output = read("rebased_set_bytes");
    let bind = |output| {
        StructuredInfoValue::record(
            ty.value_type.clone(),
            vec![
                StructuredFieldValue::new("context", context.clone()).unwrap(),
                StructuredFieldValue::new("output", output).unwrap(),
            ],
        )
        .unwrap()
    };
    let good = bind(output);
    eprintln!(
        "custody: full Native rebase admission start {}ms",
        started.elapsed().as_millis()
    );
    validate_native_invariants(&good, &ty.invariants).unwrap();
    eprintln!(
        "custody: full Native rebase admitted {}ms",
        started.elapsed().as_millis()
    );
    let StructuredInfoValueShape::Record(fields) = context.shape() else {
        panic!("record")
    };
    let previous = fields
        .iter()
        .find(|f| f.name() == "previous")
        .unwrap()
        .value()
        .clone();
    let bad = bind(previous);
    assert!(validate_native_invariants(&bad, &ty.invariants).is_err());
    eprintln!(
        "custody: foreign prior refused {}ms",
        started.elapsed().as_millis()
    );
    assert_eq!(
        good,
        StructuredInfoValue::from_canonical_bytes(&good.canonical_bytes().unwrap()).unwrap()
    );
}
