use crate::{
    check_syntax_document, parse_syntax_document, NativeIntegerExpressionSyntax as Integer,
    StartupCatalog, TypeDefinitionSyntax, TypeExpressionSyntax,
};

#[test]
fn closed_integer_extents_preserve_source_and_lower_to_existing_types() {
    let source = "type Fenêtre = collection U8 = (2 + 1) * 64\n";
    let parsed = parse_syntax_document(source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let TypeDefinitionSyntax::Scalar(TypeExpressionSyntax::Collection { length, .. }) =
        &parsed.types[0].definition
    else {
        panic!("collection expected")
    };
    assert!(matches!(length.as_ref(), Integer::Binary { .. }));
    assert_eq!(
        &source[length.span().start..length.span().end],
        "(2 + 1) * 64"
    );
    let checked = check_syntax_document(&parsed, &StartupCatalog::new()).unwrap();
    let literal = check_syntax_document(
        &parse_syntax_document("type Fenêtre = collection U8 = 192\n"),
        &StartupCatalog::new(),
    )
    .unwrap();
    assert_eq!(checked.native_types, literal.native_types);
}

#[test]
fn sequence_arithmetic_preserves_variable_cardinality_constraints() {
    let source = "type Tier = sequence U8 in 1 + 1..=(2 + 1) * 4\n";
    let computed =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let literal = check_syntax_document(
        &parse_syntax_document("type Tier = sequence U8 in 2..=12\n"),
        &StartupCatalog::new(),
    )
    .unwrap();
    assert_eq!(computed.native_types, literal.native_types);
    assert!(check_syntax_document(
        &parse_syntax_document("type Tier = sequence U8 in 3 + 2..=1 + 1\n"),
        &StartupCatalog::new()
    )
    .is_err());
}

#[test]
fn integer_arithmetic_checks_intermediates_and_reports_original_operator() {
    let source = "type Fenêtre = collection U8 = (65535 + 1) * 0\n";
    let error =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap_err();
    assert!(error.message.contains("overflows U16"));
    assert_eq!(&source[error.span.start..error.span.end], "+");
    for expression in ["0 * 1", "1024 + 1", "65535 * 2", "N + 1"] {
        assert!(
            check_syntax_document(
                &parse_syntax_document(&format!("type Value = collection U8 = {expression}\n")),
                &StartupCatalog::new()
            )
            .is_err(),
            "{expression}"
        );
    }
}

#[test]
fn unsupported_integer_operations_and_excessive_work_refuse() {
    let deep = format!("{}1{}", "(".repeat(17), ")".repeat(17));
    let nodes = ["1"; 34].join(" + ");
    let bytes = " ".repeat(257) + "1";
    for expression in [
        "-1", "1 / 2", "2 - 1", "1.0", "65536", "1 +", "(1 + 2", &deep, &nodes, &bytes,
    ] {
        let parsed = parse_syntax_document(&format!("type Value = collection U8 = {expression}\n"));
        assert!(!parsed.diagnostics.is_empty(), "{expression:?}");
    }
}
