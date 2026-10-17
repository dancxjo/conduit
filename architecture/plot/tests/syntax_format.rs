use conduit_plot::*;

#[test]
fn block_formatting_preserves_quotes_comments_patterns_and_crlf() {
    let source = "plot demo (\r\n\t >> input: Text <= 32B ~ /a[{]b/\r\n ) {\r\n\t value = \" [ } # exact  \"   \r\n# exact comment  \r\n }\r\n";
    let formatted = format_syntax(source, &StartupCatalog::new()).unwrap();
    assert_eq!(formatted, "plot demo (\r\n    >> input: Text <= 32B ~ /a[{]b/\r\n) {\r\n    value = \" [ } # exact  \"\r\n    # exact comment  \r\n}\r\n");
    assert_eq!(
        format_syntax(&formatted, &StartupCatalog::new()).unwrap(),
        formatted
    );
    let checked_before =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let checked_after =
        check_syntax_document(&parse_syntax_document(&formatted), &StartupCatalog::new()).unwrap();
    assert_eq!(
        checked_before.plots[0].runtime_front,
        checked_after.plots[0].runtime_front
    );
    assert_eq!(
        checked_before.plots[0].local_values,
        checked_after.plots[0].local_values
    );
}

#[test]
fn invalid_source_and_output_expansion_refuse_at_finite_boundaries() {
    assert!(matches!(
        format_syntax("plot unfinished {", &StartupCatalog::new()),
        Err(SyntaxFormatRefusal::InvalidSource(_))
    ));
    let oversized = "x".repeat(MAXIMUM_PLOT_SOURCE_BYTES + 1);
    assert!(matches!(
        format_syntax(&oversized, &StartupCatalog::new()),
        Err(SyntaxFormatRefusal::InvalidSource(_))
    ));
    let header = "plot pressure {\n #";
    let tail = "\n}\n";
    let source = format!(
        "{header}{}{tail}",
        "x".repeat(MAXIMUM_PLOT_SOURCE_BYTES - header.len() - tail.len())
    );
    assert!(parse_syntax_document(&source).diagnostics.is_empty());
    assert_eq!(
        format_syntax(&source, &StartupCatalog::new()),
        Err(SyntaxFormatRefusal::OutputLimit)
    );
}
