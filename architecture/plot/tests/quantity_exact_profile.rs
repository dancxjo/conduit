use conduit_core::{Quantity as Exact, QUANTITY_ENCODED_LEN, QUANTITY_INFO_ID};
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
                value_type: "Quantity".into(),
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
        let CanonicalStartupValue::Quantity(value) = &binding.value else {
            panic!("checked Quantity");
        };
        assert_eq!(value.value(), Exact::parse_plot_literal(literal).unwrap());
        assert_eq!(value.value().encode().len(), QUANTITY_ENCODED_LEN);
    }
}

#[test]
fn canonical_quantity_and_dimension_defaults_admit_extreme_scales() {
    let source = "plot exact (\n extent: Quantity = 1Qm\n) {\n}\n";
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    assert!(matches!(
        checked.plots[0].startup_parameters[0].default,
        Some(CanonicalStartupValue::Quantity(_))
    ));
    let source = source.replace("Quantity", "Distance");
    let checked =
        check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new()).unwrap();
    assert!(matches!(
        checked.plots[0].startup_parameters[0].default,
        Some(CanonicalStartupValue::Quantity(_))
    ));
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
        let CanonicalStartupValue::Quantity(value) =
            &checked.plots[0].gears[0].startup_bindings[0].value
        else {
            panic!("{literal}: Quantity");
        };
        assert_eq!(
            value.value(),
            Exact::parse_plot_literal(literal).unwrap(),
            "{literal}"
        );
        assert_eq!(parsed.round_trip(), source);
    }
}

#[test]
fn native_quantity_has_one_canonical_codec() {
    use conduit_core::{kind_id, Quantity, StructuredInfoType, Unit};
    use conduit_plot::rust_binding::{primitive_from_structured, primitive_into_structured};
    let ty = StructuredInfoType::leaf(kind_id(QUANTITY_INFO_ID)).unwrap();
    let exact = Exact::parse_plot_literal("1Qm").unwrap();
    let structured = primitive_into_structured(ty.clone(), &exact).unwrap();
    assert_eq!(
        primitive_from_structured::<Exact>(&structured).unwrap(),
        exact
    );
    assert_eq!(
        primitive_from_structured::<Quantity>(&structured).unwrap(),
        exact
    );
    assert!(primitive_into_structured(ty, &Quantity::new(1, Unit::Meter)).is_ok());
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
