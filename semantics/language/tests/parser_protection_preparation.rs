#![cfg(all(feature = "parser-model-selection", target_has_atomic = "ptr"))]
#[path = "common/parser_fixture.rs"]
mod fixture;
#[path = "common/parser_joint_v2_fixture.rs"]
mod joint;
#[path = "common/scorer_model.rs"]
mod scorer_model;

#[test]
#[ignore = "direct Source invariant codec and admission diagnostic; no graph execution claim"]
fn insert_invariants_round_trip_and_retain_exact_fact() {
    use conduit_core::StructuredInfoValue;
    use conduit_language::{
        LanguageParserProtectedEdgeProposal, LanguageParserProtectedInsertContext,
        LanguageParserProtectedSetProposal,
    };
    use conduit_plot::rust_binding::NativeRustBinding;
    let source = std::env::var("CONDUIT_PROTECTION_SET_SOURCE")
        .map(|path| std::fs::read_to_string(path).unwrap())
        .unwrap_or_else(|_| include_str!("../parser_session_protected_set.conduit").into());
    let all = [
        joint::source(),
        include_str!("../parser_session.conduit").into(),
        include_str!("../parser_session_facts.conduit").into(),
        include_str!("../parser_session_branch.conduit").into(),
        include_str!("../parser_session_protection.conduit").into(),
        source,
    ]
    .join("\n");
    let syntax = conduit_plot::parse_syntax_document(&all);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked =
        conduit_plot::check_syntax_document(&syntax, &conduit_plot::StartupCatalog::new()).unwrap();
    let ty = checked
        .native_types
        .iter()
        .find(|ty| ty.name == "LanguageParserProtectedInsertContext")
        .unwrap();
    for (index, invariant) in ty.invariants.iter().enumerate() {
        let bytes = invariant.canonical_bytes().unwrap();
        conduit_plot::PortableExpressionProgram::from_canonical_bytes(&bytes)
            .unwrap_or_else(|error| panic!("invariant {index}: {error:?}"));
    }
    let receipt: serde_json::Value = serde_json::from_str(include_str!(
        "../training/ewt_joint_v2/native_stream_protected_projection_receipt.json"
    ))
    .unwrap();
    let decode = |field: &str| {
        StructuredInfoValue::from_canonical_bytes(
            &serde_json::from_value::<Vec<u8>>(receipt[field].clone()).unwrap(),
        )
        .unwrap()
    };
    let edge =
        LanguageParserProtectedEdgeProposal::from_structured(decode("compact_proposal_bytes"))
            .unwrap();
    let set = LanguageParserProtectedSetProposal::new(
        [true, false, false, false],
        edge.current_basis().clone(),
        edge.clone(),
        edge.clone(),
        edge.clone(),
        edge,
    )
    .unwrap()
    .into_structured()
    .unwrap();
    let query = fixture::record(
        &LanguageParserProtectedInsertContext::semantic_type().unwrap(),
        vec![
            ("context", decode("native_context_bytes")),
            ("previous", set),
        ],
    );
    let query = joint::retype(&ty.value_type, &query);
    conduit_plot::rust_binding::validate_native_invariants(&query, &ty.invariants).unwrap();
    let previous = fixture::field(&query, "previous");
    let edge = fixture::field(previous, "edge0");
    for field in ["head", "dependent_choice", "head_choice"] {
        let changed = fixture::replace(
            edge,
            field,
            fixture::number(fixture::field_type(edge.value_type(), field), 4),
        );
        let changed = fixture::replace(previous, "edge0", changed);
        let changed = fixture::replace(&query, "previous", changed);
        assert!(matches!(
            conduit_plot::rust_binding::validate_native_invariants(&changed, &ty.invariants),
            Err(conduit_plot::rust_binding::NativeBindingRefusal::ViolatedInvariant { .. })
        ));
    }
    println!(
        "{} exact Source invariants round-trip and admit the retained fact",
        ty.invariants.len()
    );
}

#[test]
#[ignore = "authored protection graph expansion; does not establish protected-session execution"]
fn protected_policy_drafts_expand_without_host_grammar() {
    use conduit_plot::{
        check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
        ProfileCatalog, StartupCatalog,
    };
    let source = [
        joint::source(),
        include_str!("../parser_session.conduit").into(),
        include_str!("../parser_session_facts.conduit").into(),
        include_str!("../parser_session_branch.conduit").into(),
        include_str!("../parser_session_protection.conduit").into(),
        include_str!("../parser_session_protected_set.conduit").into(),
        include_str!("../parser_session_protected_branch.conduit").into(),
        include_str!("../parser_session_protected_mask.conduit").into(),
    ]
    .join("\n");
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let mut failures = Vec::new();
    for entry in [
        "language-parser-protected-set-initialize",
        "language-parser-protected-insert-projection",
        "language-parser-protected-set-upsert",
        "language-parser-protected-set-insert",
        "language-parser-independent-branch-query",
        "language-parser-independent-branch",
        "language-parser-independent-mask-context",
        "language-parser-independent-mask-resolve",
        "language-parser-independent-mask-filter",
        "language-parser-independent-mask",
    ] {
        match expand_canonical_plot_for_authoring(&checked, entry, &ProfileCatalog::new()) {
            Ok(_) => println!("{entry}: prepared"),
            Err(error) => failures.push(format!("{entry}: {error:?}")),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
