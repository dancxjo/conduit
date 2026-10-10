#![cfg(feature = "semantic-bindings")]
use conduit_plot::*;
use conduit_speech::ipa_constructors::*;
const PHONETIC: &str = include_str!("../examples/ipa/quoted-transcriptions.conduit");
const PHONEMIC: &str = include_str!("../examples/ipa/quoted-phonemic.conduit");
fn catalogs() -> (StartupCatalog, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_speech::authoring::install(&mut startup).unwrap();
    install(&mut startup, &mut profile).unwrap();
    install_notation(&mut startup, &profile).unwrap();
    (startup, profile)
}
fn material(quoted: &str) -> Vec<(String, String)> {
    let document = parse_syntax_document(quoted);
    let BackStatement::NamedGear(gear) = &document.plots[0].back[0] else {
        panic!()
    };
    gear.invocation
        .arguments
        .iter()
        .map(|argument| {
            let Argument::Named { name, value, .. } = argument else {
                panic!()
            };
            if name.text == "request" {
                let ExpressionSyntax::Record { fields, .. } = &value.syntax else {
                    panic!()
                };
                let provenance = fields
                    .iter()
                    .find(|field| field.name.text == "provenance")
                    .unwrap();
                let span = provenance.value.span();
                (
                    "provenance".into(),
                    document.round_trip()[span.start..span.end].into(),
                )
            } else {
                (name.text.clone(), value.text.clone())
            }
        })
        .collect()
}
fn literal(document: &SyntaxDocument) -> &TypedGlyphLiteralSyntax {
    let BackStatement::LocalValue(local) = document.plots[0].back.last().unwrap() else {
        panic!()
    };
    let ExpressionSyntax::TypedGlyphLiteral(value) = &local.value.syntax else {
        panic!()
    };
    value
}
#[test]
fn selected_source_locals_supply_context_without_a_quoted_ipa_request() {
    let (startup, profile) = catalogs();
    for (constructor, quoted, spelling) in [
        (IpaConstructor::Phonetic, PHONETIC, "ph[ˈt͡ʃãː.n̩]"),
        (IpaConstructor::Phonemic, PHONEMIC, "ph/ˈt͡ʃaː/"),
    ] {
        let fields = material(quoted);
        let declarations = fields
            .iter()
            .map(|(key, text)| format!(" chosen-{key} = {text}\n"))
            .collect::<String>();
        let source = format!("with {NOTATION_EXPORT_PATH} as ph\nplot authored {{\n{declarations} evidence = chosen-provenance\n value = {spelling}\n}}\n");
        let document = parse_syntax_document_with_glyph_notations(&source, &startup);
        assert!(
            document.diagnostics.is_empty(),
            "{:?}",
            document.diagnostics
        );
        let selections = fields
            .iter()
            .map(|(key, _)| {
                (
                    key.clone(),
                    if key == "provenance" {
                        "evidence".into()
                    } else {
                        format!("chosen-{key}")
                    },
                )
            })
            .collect::<Vec<_>>();
        let bindings = selections
            .iter()
            .map(|(key, name)| (key.as_str(), name.as_str()))
            .collect::<Vec<_>>();
        let scope = resolve_glyph_notation_scope(&document, &startup).unwrap();
        let prepared = scope
            .prepare_literal_from_source_locals(
                &document,
                "authored",
                literal(&document),
                &bindings,
                &constructor,
                &startup,
                &profile,
            )
            .unwrap();
        let checked = check_syntax_document_with_prepared_glyph_literals(
            &document,
            &startup,
            core::slice::from_ref(&prepared),
        )
        .unwrap();
        assert_eq!(checked.plots[0].local_values.len(), fields.len() + 2);
        let original = parse_syntax_document(quoted);
        let ordinary = check_syntax_document(&original, &startup).unwrap();
        let expanded =
            expand_canonical_plot_for_authoring(&ordinary, &original.plots[0].name.text, &profile)
                .unwrap();
        assert_eq!(
            prepared.ordinary().value().canonical_bytes().unwrap(),
            prepare_configuration(constructor, &expanded.expanded.gears[0].configuration)
                .unwrap()
                .bytes()
        );
        assert!(scope
            .prepare_literal_from_source_locals(
                &document,
                "authored",
                literal(&document),
                &[],
                &constructor,
                &startup,
                &profile
            )
            .is_err());
        let mut missing = bindings.clone();
        missing[0].1 = "missing";
        assert!(scope
            .prepare_literal_from_source_locals(
                &document,
                "authored",
                literal(&document),
                &missing,
                &constructor,
                &startup,
                &profile
            )
            .is_err());
        let cyclic = source.replace("evidence = chosen-provenance", "evidence = evidence");
        let cyclic = parse_syntax_document_with_glyph_notations(&cyclic, &startup);
        assert!(scope
            .prepare_literal_from_source_locals(
                &cyclic,
                "authored",
                literal(&cyclic),
                &bindings,
                &constructor,
                &startup,
                &profile
            )
            .is_err());
    }
}

#[test]
fn source_context_refuses_excessive_dependency_depth_and_wrong_type() {
    let (startup, profile) = catalogs();
    let provenance = material(PHONETIC)
        .into_iter()
        .find(|(key, _)| key == "provenance")
        .unwrap()
        .1;
    for (declarations, selected, expected) in [
        (
            format!(
                "chosen = {provenance}\n{}",
                (0..65)
                    .map(|index| format!(
                        "alias-{index} = {}\n",
                        if index == 0 {
                            "chosen".into()
                        } else {
                            format!("alias-{}", index - 1)
                        }
                    ))
                    .collect::<String>()
            ),
            "alias-64",
            "immutable Source dependency depth exceeds 64",
        ),
        ("chosen = \"not provenance\"\n".into(), "chosen", ""),
    ] {
        let source = format!("with {NOTATION_EXPORT_PATH} as ph\nplot authored {{\n{declarations} value = ph[a]\n}}\n");
        let document = parse_syntax_document_with_glyph_notations(&source, &startup);
        assert!(
            document.diagnostics.is_empty(),
            "{:?}",
            document.diagnostics
        );
        let scope = resolve_glyph_notation_scope(&document, &startup).unwrap();
        let error = scope
            .prepare_literal_from_source_locals(
                &document,
                "authored",
                literal(&document),
                &[("provenance", selected)],
                &IpaConstructor::Phonetic,
                &startup,
                &profile,
            )
            .unwrap_err();
        let LiteralPreparationRefusal::SourceContext(diagnostic) = error else {
            panic!("unexpected refusal: {error:?}")
        };
        assert!(!diagnostic.message.is_empty());
        assert!(diagnostic.span.end > diagnostic.span.start);
        if !expected.is_empty() {
            assert!(diagnostic.message.contains(expected), "{diagnostic:?}");
        }
    }
}

#[test]
fn authored_import_selects_exact_source_context() {
    let (startup, profile) = catalogs();
    for (constructor, quoted, spelling) in [
        (IpaConstructor::Phonetic, PHONETIC, "ph[ˈt͡ʃãː.n̩]"),
        (IpaConstructor::Phonemic, PHONEMIC, "ph/ˈt͡ʃaː/"),
    ] {
        let fields = material(quoted);
        let context = fields
            .iter()
            .map(|(key, _)| format!("{key}: chosen-{key}"))
            .collect::<Vec<_>>()
            .join(", ");
        let declarations = fields
            .iter()
            .map(|(key, value)| format!("chosen-{key} = {value}\n"))
            .collect::<String>();
        let source = format!("with {NOTATION_EXPORT_PATH} as ph using {{{context}}}\nplot authored {{\n{declarations}value = {spelling}\n}}\n");
        let document = parse_syntax_document_with_glyph_notations(&source, &startup);
        assert!(
            document.diagnostics.is_empty(),
            "{:?}",
            document.diagnostics
        );
        for (key, local) in &document.uses[0].glyph_context {
            assert_eq!(&source[key.span.start..key.span.end], key.text);
            assert_eq!(&source[local.span.start..local.span.end], local.text);
        }
        let scope = resolve_glyph_notation_scope(&document, &startup).unwrap();
        let receipt = scope
            .prepare_literal_from_authored_context(
                &document,
                "authored",
                literal(&document),
                &constructor,
                &startup,
                &profile,
            )
            .unwrap();
        let explicit =
            check_syntax_document_with_prepared_glyph_literals(&document, &startup, &[receipt])
                .unwrap();
        let before = startup.clone();
        let automatic =
            check_syntax_document_with_literal_constructors(&document, &startup, &profile).unwrap();
        assert_eq!(automatic, explicit);
        parse_with_startup(&source, &startup, &profile).unwrap();
        assert_eq!(startup, before);
        let stale_source = source.replace("provenance: chosen-provenance", "provenance: absent");
        let stale = parse_syntax_document_with_glyph_notations(&stale_source, &startup);
        assert!(scope
            .prepare_literal_from_authored_context(
                &stale,
                "authored",
                literal(&stale),
                &constructor,
                &startup,
                &profile,
            )
            .is_err());
        let fresh = resolve_glyph_notation_scope(&stale, &startup).unwrap();
        assert!(fresh
            .prepare_literal_from_authored_context(
                &stale,
                "authored",
                literal(&stale),
                &constructor,
                &startup,
                &profile,
            )
            .is_err());
    }
}

#[test]
fn one_family_context_admits_both_branches_and_checks_unused_selections() {
    let (startup, profile) = catalogs();
    let fields = material(PHONEMIC);
    let selections = fields
        .iter()
        .map(|(key, _)| format!("{key}: chosen-{key}"))
        .collect::<Vec<_>>()
        .join(", ");
    let declarations = fields
        .iter()
        .map(|(key, value)| format!("chosen-{key} = {value}\n"))
        .collect::<String>();
    let source = format!("with {NOTATION_EXPORT_PATH} as ph using {{{selections}}}\nplot authored {{\n{declarations}phonetic = ph[ˈt͡ʃaː]\nphonemic = ph/ˈt͡ʃaː/\n}}\n");
    let document = parse_syntax_document_with_glyph_notations(&source, &startup);
    let checked =
        check_syntax_document_with_literal_constructors(&document, &startup, &profile).unwrap();
    assert_eq!(checked.plots[0].local_values.len(), fields.len() + 2);
    for (name, constructor) in [
        ("phonetic", IpaConstructor::Phonetic),
        ("phonemic", IpaConstructor::Phonemic),
    ] {
        let (_, CanonicalStartupValue::Structured(value)) = checked.plots[0]
            .local_values
            .iter()
            .find(|(key, _)| key == name)
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(value.value_type(), &constructor.result_type());
    }
    parse_with_startup(&source, &startup, &profile).unwrap();
    // Even the phonetic branch validates explicitly selected phonemic context.
    let only_phonetic = source.replace("phonemic = ph/ˈt͡ʃaː/\n", "");
    let document = parse_syntax_document_with_glyph_notations(&only_phonetic, &startup);
    check_syntax_document_with_literal_constructors(&document, &startup, &profile).unwrap();
    let invalid_native = only_phonetic.replace("explicit notation fixture", "");
    assert_ne!(invalid_native, only_phonetic);
    let invalid = parse_syntax_document_with_glyph_notations(&invalid_native, &startup);
    let error =
        check_syntax_document_with_literal_constructors(&invalid, &startup, &profile).unwrap_err();
    assert!(error.message.contains("retained Native laws"), "{error:?}");
    let wrong = only_phonetic.replace(
        "inventory: chosen-inventory",
        "inventory: chosen-provenance",
    );
    let document = parse_syntax_document_with_glyph_notations(&wrong, &startup);
    assert!(
        check_syntax_document_with_literal_constructors(&document, &startup, &profile).is_err()
    );
    let unknown = source.replace(
        "provenance: chosen-provenance",
        "unknown: chosen-provenance",
    );
    let document = parse_syntax_document_with_glyph_notations(&unknown, &startup);
    let error =
        check_syntax_document_with_literal_constructors(&document, &startup, &profile).unwrap_err();
    assert!(error.message.contains("not declared"), "{error:?}");
}
