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
    if let Ok(names) = std::env::var("CONDUIT_PROTECTION_EXTRA_CODEC_TYPES") {
        for name in names.split(',') {
            let native = checked
                .native_types
                .iter()
                .find(|ty| ty.name == name)
                .unwrap();
            for (index, invariant) in native.invariants.iter().enumerate() {
                let bytes = invariant.canonical_bytes().unwrap();
                conduit_plot::PortableExpressionProgram::from_canonical_bytes(&bytes)
                    .unwrap_or_else(|error| panic!("{name} invariant {index}: {error:?}"));
            }
            println!(
                "{name}: {} exact invariant programs round-trip",
                native.invariants.len()
            );
        }
    }
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
    let source = if let Ok(path) = std::env::var("CONDUIT_PROTECTION_EXPANSION_SOURCE") {
        format!("{source}\n{}", std::fs::read_to_string(path).unwrap())
    } else {
        source
    };
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let mut failures = Vec::new();
    let selected = std::env::var("CONDUIT_PROTECTION_EXPANSION_ENTRIES").ok();
    let defaults = [
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
    ];
    let entries = selected
        .as_ref()
        .map_or_else(|| defaults.to_vec(), |names| names.split(',').collect());
    for entry in entries {
        match expand_canonical_plot_for_authoring(&checked, entry, &ProfileCatalog::new()) {
            Ok(_) => println!("{entry}: prepared"),
            Err(error) => failures.push(format!("{entry}: {error:?}")),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
#[ignore = "Source-native prospective forest diagnostic; no decoded or played graph claim"]
fn prospective_forest_laws_refuse_realized_cycles_and_invalid_heads() {
    use conduit_core::{StructuredInfoTypeShape, StructuredInfoValue};
    use conduit_plot::rust_binding::{validate_native_invariants, NativeBindingRefusal};
    let extra = std::fs::read_to_string(std::env::var("CONDUIT_PROTECTION_FOREST_SOURCE").unwrap())
        .unwrap();
    let source = [
        joint::source(),
        include_str!("../parser_session.conduit").into(),
        include_str!("../parser_session_facts.conduit").into(),
        include_str!("../parser_session_branch.conduit").into(),
        include_str!("../parser_session_protection.conduit").into(),
        include_str!("../parser_session_protected_set.conduit").into(),
        extra,
    ]
    .join("\n");
    let syntax = conduit_plot::parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked =
        conduit_plot::check_syntax_document(&syntax, &conduit_plot::StartupCatalog::new()).unwrap();
    let native = checked
        .native_types
        .iter()
        .find(|t| t.name == "LanguageParserProtectionForest")
        .unwrap();
    for invariant in &native.invariants {
        conduit_plot::PortableExpressionProgram::from_canonical_bytes(
            &invariant.canonical_bytes().unwrap(),
        )
        .unwrap();
    }
    let ty = &native.value_type;
    let heads_type = fixture::field_type(ty, "heads");
    let StructuredInfoTypeShape::Collection { element, .. } = heads_type.shape() else {
        panic!("heads");
    };
    let make = |count, heads: [u64; 5]| {
        fixture::record(
            ty,
            vec![
                (
                    "token_count",
                    fixture::number(fixture::field_type(ty, "token_count"), count),
                ),
                (
                    "heads",
                    StructuredInfoValue::collection(
                        heads_type.clone(),
                        heads
                            .into_iter()
                            .map(|h| fixture::number(element, h))
                            .collect(),
                    )
                    .unwrap(),
                ),
            ],
        )
    };
    // An independently held VOC2->Hello0 can remain prospective while the
    // ordinary transition state still has unread0 and every real head unset.
    validate_native_invariants(&make(4, [5, 5, 0, 5, 4]), &native.invariants).unwrap();
    validate_native_invariants(&make(1, [4, 5, 5, 5, 4]), &native.invariants).unwrap();
    for (count, heads) in [
        (4, [2, 5, 0, 5, 4]),
        (4, [1, 2, 0, 5, 4]),
        (4, [1, 2, 3, 0, 4]),
        (4, [4, 5, 4, 5, 4]),
        (3, [3, 5, 0, 5, 4]),
        (4, [5, 5, 2, 5, 4]),
        (4, [u64::MAX, 5, 0, 5, 4]),
    ] {
        assert!(matches!(
            validate_native_invariants(&make(count, heads), &native.invariants),
            Err(NativeBindingRefusal::ViolatedInvariant { .. })
        ));
    }
}
