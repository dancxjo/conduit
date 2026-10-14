//! Checks the actual Commons-owned Source, without inventing Body or Face truth.
use conduit_core::*;
use conduit_plot::*;
const SOURCE: &str = include_str!("../../../plots/commons/main.conduit");
#[test]
fn commons_places_source_checks_and_emits_its_exact_bounded_graph() {
    let syntax = parse_syntax_document(SOURCE);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    assert_eq!(syntax.round_trip(), SOURCE);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let authored =
        expand_canonical_plot_for_authoring(&checked, "commons", &ProfileCatalog::new()).unwrap();
    assert_eq!(authored.expanded.gears.len(), 1);
    let ConfigurationValue::Text(encoded) = &authored.expanded.gears[0].configuration[0].value
    else {
        panic!("checked program")
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    let bytes = evaluator.evaluate(&InfoBool::TRUE.encode()).unwrap();
    let graph = StructuredInfoValue::from_canonical_bytes(bytes).unwrap();
    let owner = checked
        .native_types
        .iter()
        .find(|ty| ty.name == "CommonsPlaces")
        .unwrap();
    rust_binding::validate_native_invariants(&graph, &owner.invariants).unwrap();
    assert_eq!(graph.canonical_bytes().unwrap(), bytes);
    println!(
        "COMMONS_SOURCE_EVIDENCE={}",
        serde_json::json!({"source_document_id":checked.source_document_id.as_str(),"checked_plot_id":checked.plots[0].checked_plot_id.as_str(),"expanded_plot_id":authored.expanded.expanded_plot_id.as_str()})
    );
}

fn rejects(source: &str) -> bool {
    let syntax = parse_syntax_document(source);
    assert!(
        syntax.diagnostics.is_empty(),
        "negative must be syntactically valid: {:?}",
        syntax.diagnostics
    );
    let Ok(checked) = check_syntax_document(&syntax, &StartupCatalog::new()) else {
        return true;
    };
    expand_canonical_plot_for_authoring(&checked, "commons", &ProfileCatalog::new()).is_err()
}

#[test]
fn unknown_duplicate_places_and_broken_relations_refuse_before_play() {
    let (declarations, body) = SOURCE.split_once("plot commons").unwrap();
    for (description, malformed) in [
        (
            "unknown place",
            body.replace(
                "identity: \"commons/creche\"",
                "identity: \"commons/unknown\"",
            ),
        ),
        (
            "duplicate place",
            body.replace(
                "identity: \"commons/workshop\"",
                "identity: \"commons/hub\"",
            ),
        ),
        (
            "self relation",
            body.replace("target: \"commons/workshop\"", "target: \"commons/hub\""),
        ),
        (
            "wrong relation meaning",
            body.replace(
                "meaning: \"commons/contains-place\"",
                "meaning: \"commons/reaches-place\"",
            ),
        ),
        ("missing relation", body.replace("containment: [ {source: \"commons/hub\", target: \"commons/workshop\", meaning: \"commons/contains-place\"}, ", "containment: [ ")),
        ("excess relation", body.replace("containment: [ ", "containment: [ {source: \"commons/hub\", target: \"commons/workshop\", meaning: \"commons/contains-place\"}, ")),
        (
            "oversized name",
            body.replace("name: \"Hub\"", &format!("name: \"{}\"", "H".repeat(33))),
        ),
        (
            "unknown domain role",
            body.replace("role: \"commons/place\"", "role: \"commons/room\""),
        ),
    ] {
        assert!(
            rejects(&format!("{declarations}plot commons{malformed}")),
            "{description}"
        );
    }
}
