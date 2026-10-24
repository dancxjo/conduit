//! Explicit bounded selection does not make optional default retention authority.
use conduit_plot::*;
fn selected(
    source: &SyntaxDocument,
    names: &[&str],
) -> Result<CheckedSyntaxDocument, SyntaxCheckDiagnostic> {
    check_syntax_document_with_native_law_roots(source, &StartupCatalog::new(), names)
}
#[test]
fn late_root_selection_preserves_full_source_types_and_expression_programs() {
    let mut source = String::new();
    for index in 0..300 {
        source.push_str(&format!("type Earlier{index:03} = {{\n value: U64\n}}\n"));
    }
    source.push_str("type Late = {\n start: U64\n end: U64\n where .start <= .end\n}\nplot late/value (\n >> value: Late\n result: U64 >>\n) = (.end - .start)\n");
    let syntax = parse_syntax_document(&source);
    let before = syntax.clone();
    let legacy = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    assert!(legacy.native_law_witness("Late").is_none());
    let requested = selected(&syntax, &["Late"]).unwrap();
    assert_eq!(syntax, before);
    assert_eq!(requested.source_document_id, legacy.source_document_id);
    assert_eq!(requested.native_types, legacy.native_types);
    assert_eq!(requested.plots, legacy.plots);
    let old =
        expand_canonical_plot_for_authoring(&legacy, "late/value", &ProfileCatalog::new()).unwrap();
    let new = expand_canonical_plot_for_authoring(&requested, "late/value", &ProfileCatalog::new())
        .unwrap();
    assert_eq!(new.expanded, old.expanded);
    let witness = requested.native_law_witness("Late").unwrap();
    assert!(requested.native_law_witness("Earlier000").is_none());
    assert_eq!(
        witness.checked_type(),
        legacy
            .native_types
            .iter()
            .find(|ty| ty.name == "Late")
            .unwrap()
    );
    assert_eq!(
        NativeLawWitness::verify(
            &witness.material().unwrap(),
            &witness.checked_type().value_type
        )
        .unwrap(),
        *witness
    );
}
#[test]
fn duplicate_unknown_and_name_pressure_are_explicit_not_partial_results() {
    let valid = parse_syntax_document("type Root = {\n value: U64\n}\n");
    let error = selected(&valid, &["Root", "Absent"]).unwrap_err();
    assert!(error.message.contains("Absent") && error.message.contains("Declaration"));
    let malformed = parse_syntax_document("definitely not valid Source");
    let error = selected(&malformed, &["Root", "Root"]).unwrap_err();
    assert!(error.message.contains("DuplicateImport"));
    assert!(selected(&malformed, &[&"x".repeat(4097)])
        .unwrap_err()
        .message
        .contains("Capacity"));
    let names = (0..4096)
        .map(|index| format!("{index:04}{}", "x".repeat(1021)))
        .collect::<Vec<_>>();
    let names = names.iter().map(String::as_str).collect::<Vec<_>>();
    assert!(selected(&malformed, &names)
        .unwrap_err()
        .message
        .contains("Capacity"));
    assert!(selected(&valid, &[])
        .unwrap()
        .native_law_witness("Root")
        .is_none());
}
#[test]
fn legacy_unwitnessed_reachable_import_does_not_gain_source_custody() {
    let owner = selected(
        &parse_syntax_document("type Owner = {\n value: U64\n}\n"),
        &["Owner"],
    )
    .unwrap();
    let mut catalog = StartupCatalog::new();
    catalog
        .insert_checked_native_type(
            "Owner",
            owner.native_law_witness("Owner").unwrap().checked_type(),
        )
        .unwrap();
    let syntax = parse_syntax_document("type Root = {\n value: Owner\n}\n");
    assert!(check_syntax_document(&syntax, &catalog).is_ok());
    let error =
        check_syntax_document_with_native_law_roots(&syntax, &catalog, &["Root"]).unwrap_err();
    assert!(error.message.contains("Root") && error.message.contains("UnsupportedContext"));
}
#[test]
fn selected_aggregate_source_module_limit_refuses_whole_request() {
    let mut source = String::new();
    let names = (0..257)
        .map(|index| format!("Root{index:03}"))
        .collect::<Vec<_>>();
    for name in &names {
        source.push_str(&format!("type {name} = {{\n value: U64\n}}\n"));
    }
    let names = names.iter().map(String::as_str).collect::<Vec<_>>();
    let error = selected(&parse_syntax_document(&source), &names).unwrap_err();
    assert!(error.message.contains("Root256") && error.message.contains("Capacity"));
}
#[test]
fn individually_overbound_checked_law_closure_refuses_honestly() {
    // Both records stay below64 fields,63 laws and4096 structural nodes.
    // Repeated nested schema bytes make this one valid law closure exceed4MiB.
    let mut source = String::from("type Cell = {\n");
    for index in 0..63 {
        source.push_str(&format!(" f{index:03}: U64\n"));
    }
    source.push_str("}\ntype Large = {\n");
    for index in 0..63 {
        source.push_str(&format!(" f{index:03}: Cell\n"));
    }
    for index in 0..63 {
        source.push_str(&format!(" where .f000.f000 <= {}\n", index + 1));
    }
    source.push_str("}\n");
    let syntax = parse_syntax_document(&source);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    assert_eq!(checked.native_types.len(), 2);
    assert_eq!(
        checked
            .native_types
            .iter()
            .find(|ty| ty.name == "Large")
            .unwrap()
            .invariants
            .len(),
        63
    );
    assert!(checked.native_law_witness("Large").is_none());
    let error = selected(&syntax, &["Large"]).unwrap_err();
    assert!(error.message.contains("Large") && error.message.contains("Capacity"));
}
