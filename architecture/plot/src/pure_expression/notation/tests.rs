use super::*;
use crate::glyph_notation_test_support::fixture;
use crate::{GlyphNotationScope, TypedLiteralDelimiter as D, TypedLiteralLexicalPolicy as P};

fn scope(policy: P) -> GlyphNotationScope {
    let (mut startup, profile, mut family) = fixture();
    let branch = family.branches[0].clone();
    family.branches = [D::Square, D::Slash, D::Angle, D::DoubleSquare]
        .into_iter()
        .filter(|delimiter| {
            policy != P::PortablePattern || matches!(delimiter, D::Slash | D::DoubleSquare)
        })
        .map(|delimiter| crate::TypedLiteralBranch {
            delimiter,
            lexical_policy: policy,
            ..branch.clone()
        })
        .collect();
    startup
        .insert_typed_literal_family("fixture/notation", family, &profile)
        .unwrap();
    let document =
        crate::parse_syntax_document("with fixture/notation as ph\nplot ordinary {\n}\n");
    crate::resolve_glyph_notation_scope(&document, &startup).unwrap()
}

fn parse(scope: &GlyphNotationScope, text: &str) -> Result<ExpressionSyntax, (String, Span)> {
    crate::pure_expression::parse_with_scope(text, text, 0, Some(scope))
}

fn literal(syntax: &ExpressionSyntax) -> &TypedGlyphLiteralSyntax {
    let ExpressionSyntax::TypedGlyphLiteral(value) = syntax else {
        panic!("{syntax:?}")
    };
    value
}

#[test]
fn actual_bound_family_recognizes_exact_pairs_in_nested_expression_positions() {
    let scope = scope(P::RawUnicode);
    let expression = parse(&scope, "{phones: [ph[t͡ʃ], ph/ˈkæt/], position: ph⟨1, 2⟩}").unwrap();
    let ExpressionSyntax::Record { fields, .. } = expression else {
        panic!()
    };
    let ExpressionSyntax::Collection { values, .. } = &fields[0].value else {
        panic!()
    };
    assert_eq!(literal(&values[0]).raw_payload.text, "t͡ʃ");
    assert_eq!(literal(&values[0]).delimiter, D::Square);
    assert_eq!(literal(&values[1]).delimiter, D::Slash);
    assert_eq!(literal(&fields[1].value).delimiter, D::Angle);
    assert_eq!(
        literal(&values[0]).family_identity,
        literal(&values[1]).family_identity
    );
    let value = parse(&scope, "fixture/ordinary(ph⟦π⟧)").unwrap();
    let ExpressionSyntax::SemanticCall { arguments, .. } = value else {
        panic!()
    };
    assert_eq!(literal(&arguments[0]).raw_payload.text, "π");
}

#[test]
fn raw_escape_parity_unicode_and_multiline_ranges_preserve_original_bytes() {
    let scope = scope(P::RawUnicode);
    for text in [
        r"ph[a\]b]",
        r"ph[a\\]",
        "ph[e\u{301}]",
        "ph[é]",
        "ph[t͡ʃ\nπ]",
    ] {
        let expression = parse(&scope, text).unwrap();
        let value = literal(&expression);
        assert_eq!(value.authored.text, text);
        assert_eq!(
            &text[value.raw_payload.span.start..value.raw_payload.span.end],
            value.raw_payload.text
        );
        assert_eq!(value.payload, value.raw_payload.text);
    }
    let source = "# π\n\nph[t͡ʃ\nπ]";
    let start = source.find("ph[").unwrap();
    let expression =
        crate::pure_expression::parse_with_scope(source, &source[start..], start, Some(&scope))
            .unwrap();
    let value = literal(&expression);
    assert_eq!((value.alias.span.line, value.alias.span.column), (3, 1));
    assert_eq!(value.raw_payload.span.end_line, 4);
    assert_eq!(
        &source[value.authored.span.start..value.authored.span.end],
        value.authored.text
    );
}

#[test]
fn portable_pattern_retains_shared_class_escape_anchor_and_flag_policy() {
    let scope = scope(P::PortablePattern);
    for text in [r"ph/^a[/]b$/i", r"ph⟦^a[/]b$⟧i", r"ph/a\.b/", r"ph/a\\b/"] {
        let expression = parse(&scope, text).unwrap();
        let value = literal(&expression);
        let scanned = scope.scan_literal("ph", text).unwrap();
        assert_eq!(value.raw_payload.text, scanned.raw_payload);
        assert_eq!(value.payload, scanned.payload);
        assert_eq!(value.case_insensitive, scanned.case_insensitive);
        assert_eq!(value.anchored_start, scanned.anchored_start);
        assert_eq!(value.anchored_end, scanned.anchored_end);
        // Lexing remains weaker than the ordinary owner's grammar admission.
        crate::parse_text_pattern(&value.payload).unwrap();
    }
    for text in ["ph/a/ii", "ph/a/g", "ph⟦unterminated", "ph[unsupported]"] {
        assert!(parse(&scope, text).is_err(), "{text}");
    }
    let expression = parse(&scope, r"ph/a\/b/").unwrap();
    assert!(crate::parse_text_pattern(&literal(&expression).payload).is_err());
}

#[test]
fn no_binding_preserves_division_quantities_collections_and_ordinary_calls() {
    let empty = GlyphNotationScope::default();
    for text in [
        "ph/x/y",
        "ph / x / y",
        "[1, 2]",
        "440Hz",
        "21°C",
        "math/sin(0)",
        "{text: \"ph[x]\", value: 12 / 3}",
    ] {
        assert_eq!(
            parse(&empty, text),
            crate::pure_expression::parse(text, text, 0),
            "{text}"
        );
    }
    let scope = scope(P::RawUnicode);
    for text in [
        "ph / x / y",
        "ph/x/y",
        "ph(x)",
        "ph〈x〉",
        "ph[unterminated",
        "ph/x]",
        "ph",
    ] {
        assert!(parse(&scope, text).is_err(), "{text}");
    }
    for text in [
        "12 / 3",
        "[1, 2]",
        "250ms",
        "440Hz",
        "21°C",
        "3.2m",
        "90°",
        "12V",
        "640px",
        "{text: \"ph[x]\"}",
        "math/sin(0)",
    ] {
        assert_eq!(
            parse(&scope, text),
            crate::pure_expression::parse(text, text, 0),
            "{text}"
        );
    }
}

#[test]
fn payload_and_existing_expression_depth_item_and_node_budgets_still_refuse() {
    let scope = scope(P::RawUnicode);
    parse(&scope, &format!("ph[{}]", "x".repeat(64))).unwrap();
    parse(&scope, &format!("ph[{}]", "π".repeat(32))).unwrap();
    assert!(parse(&scope, &format!("ph[{}]", "π".repeat(33))).is_err());
    for count in [65, 1_000_000] {
        let message = parse(&scope, &format!("ph[{}]", "x".repeat(count)))
            .unwrap_err()
            .0;
        assert!(message.contains("finite byte bound"));
    }
    let nested = format!("{}ph[x]{}", "[".repeat(40), "]".repeat(40));
    assert!(parse(&scope, &nested).is_err());
    let many = format!(
        "[{}]",
        vec!["ph[x]"; MAXIMUM_STRUCTURED_COLLECTION_ITEMS + 1].join(",")
    );
    assert!(parse(&scope, &many).is_err());
}

#[test]
fn family_revision_changes_syntax_identity_without_rewriting_source() {
    let first = scope(P::RawUnicode);
    let (mut startup, profile, _) = fixture();
    let mut family = first.binding("ph").unwrap().family.clone();
    family.revision = "fixture/notation@2".into();
    startup
        .insert_typed_literal_family("fixture/notation", family, &profile)
        .unwrap();
    let document =
        crate::parse_syntax_document("with fixture/notation as ph\nplot ordinary {\n}\n");
    let second = crate::resolve_glyph_notation_scope(&document, &startup).unwrap();
    let left = parse(&first, "ph/π/").unwrap();
    let right = parse(&second, "ph/π/").unwrap();
    assert_eq!(literal(&left).authored, literal(&right).authored);
    assert_ne!(
        literal(&left).family_identity,
        literal(&right).family_identity
    );
    assert_ne!(
        crate::syntax_identity::canonical_expression(&left),
        crate::syntax_identity::canonical_expression(&right)
    );
}

#[test]
fn lexical_candidate_cannot_bypass_ordinary_info_admission() {
    use alloc::collections::{BTreeMap, BTreeSet};
    let scope = scope(P::RawUnicode);
    let syntax = parse(&scope, "ph[π]").unwrap();
    let input = crate::CheckedExpressionType::semantic("value/text");
    let empty = BTreeMap::new();
    let context = crate::ExpressionTypeContext {
        glyph_values: None,
        input: &input,
        immutable_values: &empty,
        structured_types: &BTreeMap::new(),
        literal_types: &empty,
        numeric_types: &BTreeSet::new(),
        semantic_kinds: &BTreeMap::new(),
    };
    let refusal = crate::check_expression(&syntax, &context).unwrap_err();
    assert_eq!(refusal.span, syntax.span());
    assert!(refusal.message.contains("ordinary constructor admission"));
}

#[test]
fn public_scope_expression_entrance_rejects_invalid_utf8_ranges() {
    let scope = scope(P::RawUnicode);
    let source = "ph[π]";
    let valid = Span {
        start: 0,
        end: source.len(),
        line: 1,
        column: 1,
        end_line: 1,
        end_column: 6,
    };
    assert_eq!(
        scope.parse_expression(source, valid).unwrap().syntax,
        parse(&scope, source).unwrap()
    );
    for invalid in [
        Span {
            end: source.len() + 1,
            ..valid
        },
        Span { start: 4, ..valid },
        Span {
            start: 6,
            end: 1,
            ..valid
        },
    ] {
        assert!(scope.parse_expression(source, invalid).is_err());
    }
}
