use conduit_core::{
    ExactDecimalQuantity as Exact, StructuredInfoValueShape, EXACT_DECIMAL_QUANTITY_INFO_ID,
};
use conduit_plot::{
    check_syntax_document, parse_syntax_document, CanonicalStartupValue, KindSignature,
    StartupCatalog, StartupParameterSignature,
};

fn catalog() -> StartupCatalog {
    let mut catalog = StartupCatalog::new();
    catalog
        .insert(KindSignature {
            kind: "test/exact-consumer".into(),
            startup_parameters: vec![StartupParameterSignature {
                name: "value".into(),
                value_type: "ExactQuantity".into(),
                default: None,
            }],
        })
        .unwrap();
    catalog
}

#[test]
fn explicit_exact_type_admits_extreme_prefixes_as_checked_canonical_startup_values() {
    for literal in [
        "1Qm", "1qm", "1Qm³", "1qm³", "-1qm", "1um2", "1uW", "273.15K",
    ] {
        let source = format!("plot sample {{\n sink: test/exact-consumer({literal})\n}}\n");
        let parsed = parse_syntax_document(&source);
        assert_eq!(parsed.round_trip(), source);
        let checked = check_syntax_document(&parsed, &catalog()).unwrap();
        let binding = &checked.plots[0].gears[0].startup_bindings[0];
        let CanonicalStartupValue::Structured(value) = &binding.value else {
            panic!("explicit checked profile");
        };
        let concrete = value.try_concrete().unwrap();
        let StructuredInfoValueShape::Leaf(bytes) = concrete.shape() else {
            panic!("exact leaf");
        };
        assert!(
            matches!(concrete.value_type().shape(), conduit_core::StructuredInfoTypeShape::Leaf(kind)
            if kind.as_str() == EXACT_DECIMAL_QUANTITY_INFO_ID)
        );
        assert_eq!(
            Exact::decode(bytes),
            Exact::parse_plot_literal(literal),
            "{literal}"
        );
        assert_eq!(bytes.len(), 20);
    }
}

#[test]
fn exact_startup_defaults_are_explicit_and_legacy_defaults_still_refuse_extreme_scales() {
    let source = "plot exact (\n extent: ExactQuantity = 1Qm\n) {\n}\n";
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    assert!(matches!(
        checked.plots[0].startup_parameters[0].default,
        Some(CanonicalStartupValue::Structured(_))
    ));
    let source = source.replace("ExactQuantity", "Distance");
    let diagnostic =
        check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new()).unwrap_err();
    assert!(diagnostic.message.contains("RepresentationIneligible"));
    assert_eq!(&source[diagnostic.span.start..diagnostic.span.end], "1Qm");
}

#[test]
fn complete_reviewed_prefix_matrix_enters_the_ordinary_authored_exact_profile() {
    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../../core/tests/fixtures/quantity_prefix_scales.json"
    ))
    .unwrap();
    let catalog = catalog();
    for case in corpus["cases"].as_array().unwrap() {
        let literal = case["source"].as_str().unwrap();
        let source = format!("plot sample {{\n sink: test/exact-consumer(value = {literal})\n}}\n");
        let parsed = parse_syntax_document(&source);
        let checked =
            check_syntax_document(&parsed, &catalog).unwrap_or_else(|e| panic!("{literal}: {e:?}"));
        let CanonicalStartupValue::Structured(value) =
            &checked.plots[0].gears[0].startup_bindings[0].value
        else {
            panic!("{literal}: checked exact leaf");
        };
        let concrete = value.try_concrete().unwrap();
        let StructuredInfoValueShape::Leaf(bytes) = concrete.shape() else {
            panic!("{literal}: leaf");
        };
        assert_eq!(
            Exact::decode(bytes),
            Exact::parse_plot_literal(literal),
            "{literal}"
        );
        assert_eq!(parsed.round_trip(), source);
    }
}

#[test]
fn exact_native_carrier_keeps_type_codec_and_identity_separate_from_legacy() {
    use conduit_core::{kind_id, Quantity, QuantityUnit, StructuredInfoType};
    use conduit_plot::rust_binding::{primitive_from_structured, primitive_into_structured};
    let ty = StructuredInfoType::leaf(kind_id(EXACT_DECIMAL_QUANTITY_INFO_ID)).unwrap();
    let exact = Exact::parse_plot_literal("1Qm").unwrap();
    let structured = primitive_into_structured(ty.clone(), &exact).unwrap();
    assert_eq!(
        primitive_from_structured::<Exact>(&structured).unwrap(),
        exact
    );
    assert!(primitive_from_structured::<Quantity>(&structured).is_err());
    assert!(primitive_into_structured(ty, &Quantity::new(1, QuantityUnit::Meter)).is_err());
}

#[test]
fn reviewed_compound_suffixes_do_not_capture_ordinary_division_or_names() {
    use conduit_plot::{BackStatement, BinaryOperator, ExpressionSyntax};
    for expression in ["9/3", "9 / 3", "1dam / second", "1dam/second"] {
        let source = format!("plot grammar {{\n value = {expression}\n}}\n");
        let parsed = parse_syntax_document(&source);
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        let BackStatement::LocalValue(value) = &parsed.plots[0].back[0] else {
            panic!("local value");
        };
        assert!(
            matches!(
                value.value.syntax,
                ExpressionSyntax::Binary {
                    operator: BinaryOperator::Divide,
                    ..
                }
            ),
            "{expression}"
        );
        assert_eq!(parsed.round_trip(), source);
    }
    let source = "plot grammar {\n value = 1dam/s\n}\n";
    let parsed = parse_syntax_document(source);
    let BackStatement::LocalValue(value) = &parsed.plots[0].back[0] else {
        panic!("local value");
    };
    assert!(matches!(&value.value.syntax,
        ExpressionSyntax::Atomic(atom) if atom.text == "1dam/s"));
}
