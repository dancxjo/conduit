use conduit_core::{port_id, ConfigurationValue, FrontValueLocation, ValueConstraint};
use conduit_plot::{rust_binding::NativeRustBinding, *};
use conduit_text::*;
fn catalogs() -> (StartupCatalog, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_pattern_notation(&mut startup, &mut profile).unwrap();
    (startup, profile)
}
fn literal(document: &SyntaxDocument) -> &TypedGlyphLiteralSyntax {
    let BackStatement::LocalValue(local) = &document.plots[0].back[0] else {
        panic!()
    };
    let ExpressionSyntax::TypedGlyphLiteral(literal) = &local.value.syntax else {
        panic!()
    };
    literal
}
fn source(startup: &StartupCatalog, spelling: &str) -> SyntaxDocument {
    parse_syntax_document_with_glyph_notations(
        &format!(
            "with {PATTERN_NOTATION_EXPORT_PATH} as r\nplot example {{\n value = {spelling}\n}}\n"
        ),
        startup,
    )
}
fn bare(literal: &str) -> ValueConstraint {
    let document = parse_syntax_document(&format!(
        "plot bounded (\n >> value: Text <= 32B ~ {literal}\n) {{\n}}\n"
    ));
    check_syntax_document(&document, &StartupCatalog::new())
        .unwrap()
        .plots[0]
        .runtime_front
        .value_contract(&FrontValueLocation::Input(port_id("value")))
        .unwrap()
        .constraints[0]
        .clone()
}
#[test]
fn pattern_glyphs_reuse_ordinary_constructor_and_bare_pattern_law() {
    let (startup, profile) = catalogs();
    for (glyph, bare_pattern) in [
        ("r/^[A-Z]+$/i", "/^[A-Z]+$/i"),
        ("r⟦^[A-Z]+$⟧i", "/^[A-Z]+$/i"),
        ("r/a[/]b/", "/a[/]b/"),
        (r"r/a\./", r"/a\./"),
        (r"r/path\\$/", r"/path\\$/"),
        ("r/t͡ʃ/", "/t͡ʃ/"),
    ] {
        let document = source(&startup, glyph);
        let before = startup.clone();
        let automatically_checked =
            check_syntax_document_with_literal_constructors(&document, &startup, &profile).unwrap();
        assert_eq!(automatically_checked.plots[0].local_values.len(), 1);
        parse_with_startup(document.round_trip(), &startup, &profile).unwrap();
        assert_eq!(startup, before);
        assert!(
            document.diagnostics.is_empty(),
            "{:?}",
            document.diagnostics
        );
        let scope = resolve_glyph_notation_scope(&document, &startup).unwrap();
        let prepared = scope
            .prepare_literal(
                &document,
                literal(&document),
                &[],
                &PortablePatternConstructor,
                &startup,
                &profile,
            )
            .unwrap();
        let specification = PortablePatternSpecification::decode(
            &prepared.ordinary().value().canonical_bytes().unwrap(),
        )
        .unwrap();
        assert_eq!(
            specification.checked_constraint(32, false).unwrap(),
            bare(bare_pattern)
        );
        let checked = check_syntax_document_with_prepared_glyph_literals(
            &document,
            &startup,
            core::slice::from_ref(&prepared),
        )
        .unwrap();
        let CanonicalStartupValue::Structured(value) = &checked.plots[0].local_values[0].1 else {
            panic!()
        };
        assert_eq!(
            value.try_concrete().as_ref(),
            Some(prepared.ordinary().value())
        );
        let ordinary = "plot explicit {\n parsed: text/pattern-from-source(source = \"[A-Z]+\", case-insensitive = true, anchored-start = true, anchored-end = true)\n}\n";
        let ordinary = check_syntax_document(&parse_syntax_document(ordinary), &startup).unwrap();
        let expanded =
            expand_canonical_plot_for_authoring(&ordinary, "explicit", &profile).unwrap();
        let explicit = prepare_static_constructor(
            &PortablePatternConstructor,
            &startup,
            &profile,
            &expanded.expanded.gears[0].configuration,
        )
        .unwrap();
        if glyph.starts_with("r/^") || glyph.starts_with("r⟦^") {
            assert_eq!(explicit.value(), prepared.ordinary().value());
        }
        assert!(specification.checked_constraint(u32::MAX, false).is_err());
    }
}
#[test]
fn invalid_pattern_glyphs_refuse_without_extending_the_parser() {
    let (startup, profile) = catalogs();
    for glyph in [
        "r//", r"r/a\/b/", r"r/\d/", "r/[Z-A]/", "r/[a]/ii", "r/[a]/x", "r〈a〉",
    ] {
        let document = source(&startup, glyph);
        if !document.diagnostics.is_empty() {
            continue;
        }
        let scope = resolve_glyph_notation_scope(&document, &startup).unwrap();
        assert!(
            scope
                .prepare_literal(
                    &document,
                    literal(&document),
                    &[],
                    &PortablePatternConstructor,
                    &startup,
                    &profile
                )
                .is_err(),
            "{glyph}"
        );
    }
    let mut attempted_startup = StartupCatalog::new();
    let original = attempted_startup.clone();
    let mut attempted_profile = ProfileCatalog::new();
    attempted_profile
        .insert_kind(PortablePatternConstructor.contract())
        .unwrap();
    let original_kinds = attempted_profile.clone();
    assert!(install_pattern_notation(&mut attempted_startup, &mut attempted_profile).is_err());
    assert_eq!(attempted_startup, original);
    assert_eq!(attempted_profile, original_kinds);
    let empty_bare = parse_syntax_document("plot empty (\n >> value: Text <= 32B ~ //\n) {\n}\n");
    assert!(check_syntax_document(&empty_bare, &StartupCatalog::new()).is_err());
    let document = source(&startup, "r/a/");
    let scope = resolve_glyph_notation_scope(&document, &startup).unwrap();
    let context = [conduit_core::ConfigurationEntry {
        key: "ambient".into(),
        value: ConfigurationValue::Bool(true),
    }];
    assert!(scope
        .prepare_literal(
            &document,
            literal(&document),
            &context,
            &PortablePatternConstructor,
            &startup,
            &profile
        )
        .is_err());
}

#[test]
fn duplicate_compiled_owner_refuses_without_mutating_catalog() {
    let (mut startup, profile) = catalogs();
    let before = startup.clone();
    assert!(startup
        .install_literal_constructor(&PortablePatternConstructor, &profile)
        .is_err());
    assert_eq!(startup, before);
}

#[test]
fn prefixed_front_refinements_consume_the_prepared_pattern_with_a_finite_bound() {
    let (startup, profile) = catalogs();
    for (glyph, ordinary) in [
        ("r/^[A-Z]+$/i", "/^[A-Z]+$/i"),
        ("r⟦a[/]b⟧", "/a[/]b/"),
        ("r/t͡ʃ/", "/t͡ʃ/"),
    ] {
        let authored = format!(
            "with {PATTERN_NOTATION_EXPORT_PATH} as r\nplot bounded (\n >> value: Text <= 32B ~ {glyph}\n) {{\n}}\n"
        );
        let document = parse_syntax_document_with_glyph_notations(&authored, &startup);
        assert_eq!(document.round_trip(), authored);
        assert!(
            document.diagnostics.is_empty(),
            "{:?}",
            document.diagnostics
        );
        assert!(check_syntax_document(&document, &startup).is_err());
        let checked =
            check_syntax_document_with_literal_constructors(&document, &startup, &profile).unwrap();
        assert_eq!(
            checked.plots[0]
                .runtime_front
                .value_contract(&FrontValueLocation::Input(port_id("value")))
                .unwrap()
                .constraints[0],
            bare(ordinary)
        );
    }
    for ty in ["Text <= 4294967296B", "U32 <= 32B"] {
        let document = parse_syntax_document_with_glyph_notations(&format!(
            "with {PATTERN_NOTATION_EXPORT_PATH} as r\nplot bounded (\n >> value: {ty} ~ r/foo/\n) {{\n}}\n"
        ), &startup);
        assert!(
            check_syntax_document_with_literal_constructors(&document, &startup, &profile).is_err(),
            "{ty}"
        );
    }
}

#[test]
fn native_type_pattern_refinements_reuse_the_same_sealed_consumer() {
    let (startup, profile) = catalogs();
    for definition in [
        "type Label = Text <= 32B ~ SPELLING\n",
        "type Label = {\n name: Text <= 32B !~ SPELLING\n}\n",
        "type Label = sequence Text <= 32B ~ SPELLING <= 4\n",
    ] {
        let ordinary = parse_syntax_document(&definition.replace("SPELLING", "/^[A-Z]+$/i"));
        let ordinary = check_syntax_document(&ordinary, &startup).unwrap();
        for spelling in ["r/^[A-Z]+$/i", "r⟦^[A-Z]+$⟧i"] {
            let authored = format!(
                "with {PATTERN_NOTATION_EXPORT_PATH} as r\n{}",
                definition.replace("SPELLING", spelling)
            );
            let document = parse_syntax_document_with_glyph_notations(&authored, &startup);
            assert!(
                document.diagnostics.is_empty(),
                "{:?}",
                document.diagnostics
            );
            assert!(check_syntax_document(&document, &startup).is_err());
            let checked =
                check_syntax_document_with_literal_constructors(&document, &startup, &profile)
                    .unwrap();
            assert_eq!(
                checked.native_types[0].value_contracts,
                ordinary.native_types[0].value_contracts
            );
            assert_eq!(document.round_trip(), authored);
        }
    }
}

#[test]
fn scoped_comments_and_highlights_keep_payload_bytes_opaque() {
    let (startup, profile) = catalogs();
    for spelling in ["r/a#b/", "r⟦t͡ʃ#b⟧", "r/a'b/", "r/a\"b/"] {
        let authored = format!(
            "with {PATTERN_NOTATION_EXPORT_PATH} as r\nplot example {{\n value = {spelling} # outer comment\n}}\n"
        );
        let document = parse_syntax_document_with_glyph_notations(&authored, &startup);
        assert!(
            document.diagnostics.is_empty(),
            "{:?}",
            document.diagnostics
        );
        check_syntax_document_with_literal_constructors(&document, &startup, &profile).unwrap();
        let scope = resolve_glyph_notation_scope(&document, &startup).unwrap();
        let spans = highlight_syntax_in_scope(&authored, &scope).unwrap();
        assert_eq!(
            spans
                .iter()
                .map(|span| &authored[span.start..span.end])
                .collect::<String>(),
            authored
        );
        let payload = &literal(&document).raw_payload;
        assert!(spans.iter().any(|span| span.start == payload.span.start
            && span.end == payload.span.end
            && span.kind == SyntaxHighlightKind::Literal));
        assert!(spans
            .iter()
            .any(|span| span.kind == SyntaxHighlightKind::Comment
                && &authored[span.start..span.end] == "# outer comment"));
        assert_eq!(document.round_trip(), authored);
    }
}

#[test]
fn glyph_payload_punctuation_does_not_split_outer_grammar() {
    let (startup, profile) = catalogs();
    for spelling in ["r/a=b/", "r/a>>b/", "r/a{2}b/", "r/a,b/", "r⟦a,b=>>x{2}⟧"] {
        let document = source(&startup, spelling);
        assert!(
            document.diagnostics.is_empty(),
            "{spelling}: {:?}",
            document.diagnostics
        );
        check_syntax_document_with_literal_constructors(&document, &startup, &profile).unwrap();
        let authored = format!(
            "with {PATTERN_NOTATION_EXPORT_PATH} as r\nplot example {{\n work: example/accept(value={spelling}, count=1)\n}}\n"
        );
        let document = parse_syntax_document_with_glyph_notations(&authored, &startup);
        assert!(
            document.diagnostics.is_empty(),
            "{spelling}: {:?}",
            document.diagnostics
        );
        let BackStatement::NamedGear(gear) = &document.plots[0].back[0] else {
            panic!()
        };
        assert_eq!(gear.invocation.arguments.len(), 2);
        let Argument::Named { value, .. } = &gear.invocation.arguments[0] else {
            panic!()
        };
        let ExpressionSyntax::TypedGlyphLiteral(literal) = &value.syntax else {
            panic!()
        };
        assert_eq!(literal.authored.text, spelling);
        assert_eq!(
            &authored[literal.authored.span.start..literal.authored.span.end],
            spelling
        );
    }
    let authored = format!(
        "with {PATTERN_NOTATION_EXPORT_PATH} as r\nplot example (\n pattern: Text <= 32B ~ r/a=b/\n >> value: Text <= 32B ~ r/a>>b/\n) {{\n}}\n"
    );
    let document = parse_syntax_document_with_glyph_notations(&authored, &startup);
    assert!(
        document.diagnostics.is_empty(),
        "{:?}",
        document.diagnostics
    );
    check_syntax_document_with_literal_constructors(&document, &startup, &profile).unwrap();
}

#[test]
fn editing_header_resolution_preserves_exact_context_and_import_spans() {
    let (startup, _) = catalogs();
    let authored = format!("# Unicode π\nwith {PATTERN_NOTATION_EXPORT_PATH} as rx\nplot unfinished {{\n value = rx/a#b/\n");
    let scope = GlyphNotationScope::from_source_header(&authored, &startup).unwrap();
    let binding = scope.binding("rx").unwrap();
    assert_eq!(
        &authored[binding.import_span.start..binding.import_span.end],
        format!("with {PATTERN_NOTATION_EXPORT_PATH} as rx")
    );
    let spans = highlight_syntax_in_scope(&authored, &scope).unwrap();
    assert!(spans
        .iter()
        .any(|span| span.kind == SyntaxHighlightKind::Literal
            && &authored[span.start..span.end] == "a#b"));
    assert!(GlyphNotationScope::from_source_header(
        "with text/pattern/notation as r\nwith text/pattern/notation as r\nplot unfinished {",
        &startup
    )
    .is_err());
}

#[test]
fn formatting_preserves_glyph_spelling_values_and_distinct_source_custody() {
    let (startup, profile) = catalogs();
    for spelling in ["r/^[A-Z]+$/i", "r⟦a#'\",b={2}⟧", "r⟦a[{]b⟧"] {
        let source = format!("with {PATTERN_NOTATION_EXPORT_PATH} as r\nplot example {{\n\t value = {spelling}   \n   }}");
        let formatted = format_syntax(&source, &startup).unwrap();
        assert_eq!(formatted, format!("with {PATTERN_NOTATION_EXPORT_PATH} as r\nplot example {{\n    value = {spelling}\n}}\n"));
        assert_eq!(format_syntax(&formatted, &startup).unwrap(), formatted);
        let before = parse_syntax_document_with_glyph_notations(&source, &startup);
        let after = parse_syntax_document_with_glyph_notations(&formatted, &startup);
        assert_eq!(
            literal(&before).authored.text,
            literal(&after).authored.text
        );
        assert_eq!(
            literal(&before).raw_payload.text,
            literal(&after).raw_payload.text
        );
        assert_ne!(before.source_document_id(), after.source_document_id());
        let before =
            check_syntax_document_with_literal_constructors(&before, &startup, &profile).unwrap();
        let after =
            check_syntax_document_with_literal_constructors(&after, &startup, &profile).unwrap();
        assert_eq!(before.plots[0].local_values, after.plots[0].local_values);
    }
}

#[test]
fn multiline_pattern_source_survives_formatting_and_checked_admission() {
    let (startup, profile) = catalogs();
    for newline in ["\n", "\r\n"] {
        let payload = format!("a{newline}#[}}]=>>{newline}t͡ʃ");
        let document = source(&startup, &format!("r⟦{payload}⟧"));
        assert!(
            document.diagnostics.is_empty(),
            "{:?}",
            document.diagnostics
        );
        let before =
            check_syntax_document_with_literal_constructors(&document, &startup, &profile).unwrap();
        let formatted = format_syntax(document.round_trip(), &startup).unwrap();
        assert!(formatted.contains(&format!("r⟦{payload}⟧")));
        assert_eq!(format_syntax(&formatted, &startup).unwrap(), formatted);
        let after_document = parse_syntax_document_with_glyph_notations(&formatted, &startup);
        let after =
            check_syntax_document_with_literal_constructors(&after_document, &startup, &profile)
                .unwrap();
        assert_eq!(before.plots[0].local_values, after.plots[0].local_values);
        assert_eq!(literal(&after_document).raw_payload.text, payload);
    }
}

#[test]
fn notation_import_preserves_native_quantities_and_source_owned_celsius_refusal() {
    let (startup, profile) = catalogs();
    let values = " tone = 440Hz\n delay = 250ms\n temperature = 21°C\n distance = 3.2m\n angle = 90°\n voltage = 12V\n width = 640px\n";
    let ordinary = format!("plot quantities {{\n{values}}}\n");
    let imported = format!(
        "with {PATTERN_NOTATION_EXPORT_PATH} as r\nplot quantities {{\n{values} marker = r/a/\n}}\n"
    );
    let check = |source: &str| {
        let document = parse_syntax_document_with_glyph_notations(source, &startup);
        check_syntax_document_with_literal_constructors(&document, &startup, &profile)
    };
    let baseline = check(&ordinary).unwrap();
    let scoped = check(&imported).unwrap();
    let scoped_values: Vec<_> = scoped.plots[0]
        .local_values
        .iter()
        .filter(|(name, _)| name != "marker")
        .cloned()
        .collect();
    assert_eq!(baseline.plots[0].local_values, scoped_values);
    for source in [ordinary, imported] {
        let near_miss = source.replace("21°C", "21C");
        let refused = check(&near_miss).unwrap_err();
        assert_eq!(refused.code, "CND-FRM-055");
        assert!(refused.message.contains("use '°C'"));
        // Authored units take precedence over the builtin-only near-miss rule.
        let declared = near_miss.replacen(
            "plot quantities {",
            "unit C : Temperature = { reference: K, scale: 1, delta: { quantity: TemperatureDelta, reference: K, scale: 1 } }\nplot quantities {",
            1,
        );
        let checked = check(&declared).unwrap();
        let (_, CanonicalStartupValue::Quantity(value)) = checked.plots[0]
            .local_values
            .iter()
            .find(|(name, _)| name == "temperature")
            .unwrap()
        else {
            panic!("checked authored Unit")
        };
        assert_eq!(value.value().unit().symbol(), "C");
    }
}
