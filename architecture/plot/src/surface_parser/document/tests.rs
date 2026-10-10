use crate::glyph_notation_test_support::{fixture, MANIFEST, SOURCE};
use crate::*;

fn installed() -> StartupCatalog {
    let (mut startup, profile, _) = fixture();
    let original = SOURCE
        .split_once("branches: [")
        .unwrap()
        .1
        .split_once(']')
        .unwrap()
        .0;
    let square = original.replace("\"slash\"", "\"square\"");
    let source = SOURCE.replace(original, &format!("{square}, {original}"));
    let manifest = parse_syntax_document(MANIFEST);
    let members = [PackageMemberSource {
        path: "glyph/notation",
        source: &source,
    }];
    let bundle =
        CheckedPackageBundle::from_sources(MANIFEST, &manifest.packages[0], &members).unwrap();
    PackageExportCatalog::from_bundle(&bundle, MANIFEST, &manifest.packages[0], &members)
        .unwrap()
        .install_shipped_glyph_notations(&mut startup, &profile)
        .unwrap();
    startup
}

fn parsed(source: &str, startup: &StartupCatalog) -> SyntaxDocument {
    let document = parse_syntax_document_with_glyph_notations(source, startup);
    assert!(
        document.diagnostics.is_empty(),
        "{:?}",
        document.diagnostics
    );
    document
}

#[test]
fn real_shipped_declaration_drives_full_source_nested_literal_recognition() {
    let startup = installed();
    let before = startup.clone();
    let source = "with fixture/glyph/notation as ph\nplot author {\n spoken = ph[t͡ʃ]\n underlying = ph/ˈkæt/\n nested = {phones: [ph[x], ph/y/], pitch: 440Hz}\n constructor: fixture/ordinary(ph[π])\n}\n";
    let document = parsed(source, &startup);
    assert_eq!(document.round_trip(), source);
    assert_eq!(
        document.source_document_id(),
        parse_syntax_document(source).source_document_id()
    );
    let BackStatement::LocalValue(value) = &document.plots[0].back[0] else {
        panic!()
    };
    let ExpressionSyntax::TypedGlyphLiteral(literal) = &value.value.syntax else {
        panic!()
    };
    assert_eq!(literal.raw_payload.text, "t͡ʃ");
    assert_eq!(
        &source[literal.raw_payload.span.start..literal.raw_payload.span.end],
        "t͡ʃ"
    );
    let BackStatement::LocalValue(value) = &document.plots[0].back[1] else {
        panic!()
    };
    assert!(
        matches!(value.value.syntax, ExpressionSyntax::TypedGlyphLiteral(ref value) if value.delimiter == TypedLiteralDelimiter::Slash)
    );
    let BackStatement::LocalValue(value) = &document.plots[0].back[2] else {
        panic!()
    };
    assert!(matches!(
        value.value.syntax,
        ExpressionSyntax::Record { .. }
    ));
    let BackStatement::NamedGear(gear) = &document.plots[0].back[3] else {
        panic!()
    };
    assert!(
        matches!(gear.invocation.arguments[0], Argument::Positional(ref value) if matches!(value.syntax, ExpressionSyntax::TypedGlyphLiteral(_)))
    );
    assert_eq!(startup, before);
    // No syntax-only recognition is counted as an admitted constructor use.
    assert!(check_syntax_document(&document, &startup).is_err());
}

#[test]
fn metadata_catalogue_alone_never_imports_a_family_or_reinterprets_division() {
    let startup = installed();
    for source in [
        "plot ordinary {\n value = ph/x/y\n}\n",
        "with unrelated/kind as other\nplot ordinary {\n value = ph / x / y\n}\n",
        "sans glyphs\nplot ordinary {\n value = [1, 2]\n pitch = 440Hz\n}\n",
        "plot ordinary {\n text = \"ph[x]\"\n}\n",
    ] {
        assert_eq!(
            parse_syntax_document_with_glyph_notations(source, &startup),
            parse_syntax_document(source)
        );
    }
    let source =
        "with fixture/glyph/notation as ph\nsans glyphs\nplot ordinary {\n value = ph[x]\n}\n";
    parsed(source, &startup);
    assert!(!parse_syntax_document(source).diagnostics.is_empty());
}

#[test]
fn lexical_collision_check_is_repeated_after_all_source_definitions_are_known() {
    let startup = installed();
    for body in [
        "plot ph {\n value = ph[x]\n}\n",
        "plot ordinary {\n ph = 1\n value = ph[x]\n}\n",
        "type ph = Text\nplot ordinary {\n value = ph[x]\n}\n",
        "plot ordinary {\n plot ph {\n value = ph[x]\n }\n}\n",
    ] {
        let source = format!("with fixture/glyph/notation as ph\n{body}");
        let document = parse_syntax_document_with_glyph_notations(&source, &startup);
        assert!(!document.diagnostics.is_empty(), "{source}");
        assert!(
            document.diagnostics[0]
                .message
                .contains("conflicts with another lexical binding"),
            "{:?}",
            document.diagnostics
        );
    }
}

#[test]
fn full_source_refuses_conflicting_bindings_and_malformed_declared_branches() {
    let startup = installed();
    let before = startup.clone();
    for value in [
        "ph / x / y",
        "ph/x/y",
        "ph[unterminated",
        "ph〈x〉",
        "ph/x/g",
        "ph[x]junk",
    ] {
        let source =
            format!("with fixture/glyph/notation as ph\nplot ordinary {{\n value = {value}\n}}\n");
        assert!(
            !parse_syntax_document_with_glyph_notations(&source, &startup)
                .diagnostics
                .is_empty(),
            "{value}"
        );
    }
    let source = "with fixture/glyph/notation as ph\nwith fixture/glyph/notation as ph\nplot ordinary {\n value = ph[x]\n}\n";
    assert!(
        !parse_syntax_document_with_glyph_notations(source, &startup)
            .diagnostics
            .is_empty()
    );
    assert_eq!(startup, before);
}

#[test]
fn multiline_glyph_payloads_preserve_bytes_spans_and_outer_statement_boundaries() {
    let startup = installed();
    for newline in ["\n", "\r\n"] {
        let payload = format!("π{newline}# payload }} >> = \"{newline}t͡ʃ");
        let source = format!(
            "with fixture/glyph/notation as ph{newline}plot author {{{newline} value = ph[{payload}] # outside{newline} after = 440Hz{newline}}}{newline}"
        );
        let document = parsed(&source, &startup);
        assert_eq!(document.round_trip(), source);
        assert_eq!(document.plots[0].back.len(), 2);
        let BackStatement::LocalValue(value) = &document.plots[0].back[0] else {
            panic!()
        };
        let ExpressionSyntax::TypedGlyphLiteral(literal) = &value.value.syntax else {
            panic!()
        };
        assert_eq!(literal.raw_payload.text, payload);
        assert_eq!(
            &source[literal.raw_payload.span.start..literal.raw_payload.span.end],
            payload
        );
        assert_eq!(literal.raw_payload.span.line, 3);
        assert_eq!(literal.raw_payload.span.end_line, 5);
        assert_eq!(value.value.text, format!("ph[{payload}]"));
        let BackStatement::LocalValue(after) = &document.plots[0].back[1] else {
            panic!()
        };
        assert_eq!(after.value.text, "440Hz");
        assert_eq!(after.value.span.line, 6);
    }
}
