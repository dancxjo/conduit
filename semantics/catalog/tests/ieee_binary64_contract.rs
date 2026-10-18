use conduit_core::*;
use conduit_plot::*;
use conduit_semantic_catalog::*;

fn expand(text: &str) -> Result<ExpandedAuthoringPlot, String> {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_numerical_catalogs(&mut startup, &mut profile)?;
    let syntax = parse_syntax_document(text);
    let checked = check_syntax_document(&syntax, &startup)
        .map_err(|e| format!("{}: {}", e.code, e.message))?;
    expand_canonical_plot_for_authoring(&checked, "main", &profile).map_err(|e| e.message)
}
fn source(kind: &str, configuration: &str, result: &str) -> String {
    format!("plot main (\n >> left: F64\n >> right: F64\n result: {result} >>\n) {{\n op: {kind}({configuration})\n left >> op.left\n right >> op.right\n op.result >> result\n}}\n")
}
#[test]
fn all_arithmetic_modes_and_quiet_predicates_have_exact_checked_source_contracts() {
    for (operation, expected_operation) in [
        ("add", IeeeBinary64Operation::Add),
        ("subtract", IeeeBinary64Operation::Subtract),
        ("multiply", IeeeBinary64Operation::Multiply),
        ("divide", IeeeBinary64Operation::Divide),
    ] {
        for (rounding, expected_rounding) in [
            ("nearest_ties_even", Binary64Rounding::NearestTiesEven),
            ("toward_zero", Binary64Rounding::TowardZero),
            ("toward_positive", Binary64Rounding::TowardPositive),
            ("toward_negative", Binary64Rounding::TowardNegative),
        ] {
            let authored = expand(&source(
                IEEE_BINARY64_KIND,
                &format!("operation={operation}(empty),rounding={rounding}(empty)"),
                "IeeeBinary64Result",
            ))
            .unwrap();
            let gear = &authored.expanded.gears[0];
            assert_eq!(gear.inputs.len(), 2);
            assert!(gear
                .inputs
                .iter()
                .all(|p| p.value_kind.as_str() == F64_INFO_ID));
            assert_eq!(
                ieee_binary64_configuration(&gear.kind_id, &gear.configuration).unwrap(),
                IeeeBinary64Configuration::Arithmetic {
                    operation: expected_operation,
                    rounding: expected_rounding
                }
            );
        }
    }
    for (predicate, expected) in [
        ("equal", IeeeBinary64QuietPredicate::Equal),
        ("less", IeeeBinary64QuietPredicate::Less),
        ("less_equal", IeeeBinary64QuietPredicate::LessEqual),
        ("unordered", IeeeBinary64QuietPredicate::Unordered),
    ] {
        let authored = expand(&source(
            IEEE_BINARY64_QUIET_KIND,
            &format!("predicate={predicate}(empty)"),
            "IeeeBinary64ComparisonResult",
        ))
        .unwrap();
        let gear = &authored.expanded.gears[0];
        assert_eq!(
            ieee_binary64_configuration(&gear.kind_id, &gear.configuration).unwrap(),
            IeeeBinary64Configuration::Quiet {
                predicate: expected
            }
        );
    }
    for kind in [
        ieee_binary64_semantic_contract(),
        ieee_binary64_quiet_semantic_contract(),
    ] {
        pure_expression_facts(&kind).unwrap();
    }
}
#[test]
fn rounding_is_explicit_and_unknown_modes_foreign_types_and_extra_fields_refuse() {
    for configuration in [
        "operation=add(empty)",
        "rounding=nearest_ties_even(empty)",
        "operation=add(empty),rounding=up(empty)",
        "operation=sqrt(empty),rounding=nearest_ties_even(empty)",
    ] {
        assert!(expand(&source(
            IEEE_BINARY64_KIND,
            configuration,
            "IeeeBinary64Result"
        ))
        .is_err());
    }
    let authored = expand(&source(
        IEEE_BINARY64_KIND,
        "operation=add(empty),rounding=nearest_ties_even(empty)",
        "IeeeBinary64Result",
    ))
    .unwrap();
    let gear = &authored.expanded.gears[0];
    let mut configuration = gear.configuration.clone();
    configuration.push(configuration[0].clone());
    assert!(ieee_binary64_configuration(&gear.kind_id, &configuration).is_err());
    let mut configuration = gear.configuration.clone();
    let ConfigurationValue::Structured(value) = &mut configuration[0].value else {
        panic!()
    };
    assert!(StructuredConfigurationValue::new(
        kind_id("foreign/ieee-mode@1"),
        value.canonical_value().to_vec(),
    )
    .is_none());
    let foreign = StructuredInfoValue::leaf(
        StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap(),
        InfoBool::new(true).encode().to_vec(),
    )
    .unwrap();
    *value = StructuredConfigurationValue::new(
        foreign.value_type().profile().unwrap().value_kind().clone(),
        foreign.canonical_bytes().unwrap(),
    )
    .unwrap();
    assert!(ieee_binary64_configuration(&gear.kind_id, &configuration).is_err());
    assert!(ieee_binary64_configuration(&kind_id("foreign/ieee@1"), &gear.configuration).is_err());
}
