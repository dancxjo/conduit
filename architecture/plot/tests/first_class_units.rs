use conduit_core::*;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    quantity_conversion, CanonicalStartupValue, ProfileCatalog, StartupCatalog,
};

fn expand(source: &str, root: &str) -> conduit_plot::ExpandedAuthoringPlot {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    quantity_conversion::install(&mut startup, &mut profile).unwrap();
    let syntax = parse_syntax_document(source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    quantity_conversion::validate_source(&syntax, &checked).unwrap();
    expand_canonical_plot_for_authoring(&checked, root, &profile).unwrap()
}
#[test]
fn typed_conversion_and_comparator_check_expand_and_validate_exact_facts() {
    for alias in [false, true] {
        let header = if alias {
            "with units/converted-equals as =?\n"
        } else {
            ""
        };
        let name = if alias {
            "=?"
        } else {
            "units/converted-equals"
        };
        let source=format!("{header}plot demo (\n result: Boolean >>\n) {{\n operation: units/convert(source = 1kHz, to = Hz)\n exact: {name}(expected = 1000Hz)\n operation.receipt >> exact.receipt\n exact.result >> result\n}}.\n");
        let expanded = expand(&source, "demo");
        let gears = &expanded.expanded.gears;
        let convert = gears
            .iter()
            .find(|g| g.kind_id.as_str() == quantity_conversion::KIND)
            .unwrap();
        let equals = gears
            .iter()
            .find(|g| g.kind_id.as_str() == quantity_conversion::converted_equals::KIND)
            .unwrap();
        let receipt = quantity_conversion::prepare_configuration(&convert.configuration).unwrap();
        let ConfigurationValue::Quantity(expected) = &equals.configuration[0].value else {
            panic!("typed expected")
        };
        assert!(
            quantity_conversion::converted_equals::compare_receipt(&receipt, expected.value())
                .unwrap()
        );
        assert_eq!(
            convert.startup_parameters[0].value_type,
            kind_id(QUANTITY_INFO_ID)
        );
        assert_eq!(
            convert.startup_parameters[1].value_type,
            kind_id(UNIT_INFO_ID)
        );
    }
}
#[test]
fn public_values_forward_through_parameters_and_preserve_source_spelling() {
    let source="plot convert (\n source: Quantity\n to: Unit\n receipt: ExactQuantityConversionReceipt <= 8192B >>\n) {\n operation: units/convert(source, to)\n operation.receipt >> receipt\n}\nplot demo (\n receipt: ExactQuantityConversionReceipt <= 8192B >>\n) {\n source = 1Qm\n target = qm\n operation: convert(source, to = target)\n operation.receipt >> receipt\n}.\n";
    let expanded = expand(source, "demo");
    let gear = expanded
        .expanded
        .gears
        .iter()
        .find(|g| g.kind_id.as_str() == quantity_conversion::KIND)
        .unwrap();
    let ConfigurationValue::Quantity(value) = &gear.configuration[0].value else {
        panic!("quantity")
    };
    assert_eq!(value.source(), "1Qm");
    assert_eq!(
        value.value().to_i64(Unit::Meter),
        Err(QuantityConversionRefusal::Overflow)
    );
    quantity_conversion::validate_receipt(
        &quantity_conversion::prepare_configuration(&gear.configuration).unwrap(),
    )
    .unwrap();
}
#[test]
fn physical_literals_have_intrinsic_types_and_local_names_take_precedence() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    quantity_conversion::install(&mut startup, &mut profile).unwrap();
    let syntax = parse_syntax_document(
        "plot demo {\n frequency = 1kHz\n target = Hz\n Hz = 2m\n shadowed = Hz\n}.\n",
    );
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let values = &checked.plots[0].local_values;
    assert!(matches!(
        values.iter().find(|(n, _)| n == "frequency").unwrap().1,
        CanonicalStartupValue::Quantity(_)
    ));
    assert!(matches!(
        values.iter().find(|(n, _)| n == "shadowed").unwrap().1,
        CanonicalStartupValue::Quantity(_)
    ));
}
#[test]
fn wrong_value_roles_quotes_and_unimported_aliases_refuse() {
    for source in [
        "plot demo {\n operation: units/convert(source = Hz, to = Hz)\n}.\n",
        "plot demo {\n operation: units/convert(source = 1kHz, to = 1000Hz)\n}.\n",
        "plot demo {\n operation: units/convert(source = \"1kHz\", to = Hz)\n}.\n",
        "plot demo {\n exact: =?(expected = 1kHz)\n}.\n",
        "plot point (\n point: Quantity = 21°C\n) {\n operation: units/convert-temperature-difference(source = point, to = K)\n}.\n",
    ] {
        let mut startup=StartupCatalog::new();quantity_conversion::install(&mut startup,&mut ProfileCatalog::new()).unwrap();
        let syntax=parse_syntax_document(source);assert!(syntax.diagnostics.is_empty(),"{:?}",syntax.diagnostics);
        assert!(check_syntax_document(&syntax,&startup).is_err(),"{source}");
    }
}

#[test]
fn comparator_alias_does_not_capture_assignment_equality_or_ternaries() {
    let source = "with units/converted-equals as =?\nplot demo {\n expected = 1000Hz\n choice = true == false ? 1 : 2\n exact: =?(expected)\n}.\n";
    let parsed = parse_syntax_document(source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    assert_eq!(parsed.round_trip(), source);
    let conduit_plot::BackStatement::LocalValue(value) = &parsed.plots[0].back[1] else {
        panic!("assignment");
    };
    assert!(matches!(
        value.value.syntax,
        conduit_plot::ExpressionSyntax::Conditional { .. }
    ));
    let mut startup = StartupCatalog::new();
    quantity_conversion::install(&mut startup, &mut ProfileCatalog::new()).unwrap();
    let collision = "with units/converted-equals as =?\nwith units/compare as =?\nplot demo {}\n";
    let parsed = parse_syntax_document(collision);
    assert!(check_syntax_document(&parsed, &startup).is_err());
}

#[test]
fn source_evidence_cannot_be_readmitted_against_a_foreign_document() {
    let source = "plot demo {\n operation: units/convert(source = 1kHz, to = Hz)\n}.\n";
    let mut startup = StartupCatalog::new();
    quantity_conversion::install(&mut startup, &mut ProfileCatalog::new()).unwrap();
    let parsed = parse_syntax_document(source);
    let checked = check_syntax_document(&parsed, &startup).unwrap();
    let foreign = parse_syntax_document(&format!("# different source\n{source}"));
    assert_eq!(
        *quantity_conversion::validate_source(&foreign, &checked)
            .unwrap_err()
            .refusal,
        quantity_conversion::QuantityConversionPreparationRefusal::SourceCorrelation
    );
}

#[test]
fn reviewed_punctuation_units_do_not_change_numeric_operator_grammar() {
    for unit in ["%", "m/s", "m/s²"] {
        let source =
            format!("plot demo {{\n operation: units/convert(source = 1m/s, to = {unit})\n}}.\n");
        let mut startup = StartupCatalog::new();
        quantity_conversion::install(&mut startup, &mut ProfileCatalog::new()).unwrap();
        let parsed = parse_syntax_document(&source);
        assert!(
            parsed.diagnostics.is_empty(),
            "{unit}: {:?}",
            parsed.diagnostics
        );
        assert!(check_syntax_document(&parsed, &startup).is_ok(), "{unit}");
    }
    for expression in ["9%2", "9/3", "m / s", "m/unknown"] {
        let source = format!("plot demo {{\n value = {expression}\n}}.\n");
        let parsed = parse_syntax_document(&source);
        let conduit_plot::BackStatement::LocalValue(value) = &parsed.plots[0].back[0] else {
            panic!("assignment");
        };
        assert!(
            matches!(
                value.value.syntax,
                conduit_plot::ExpressionSyntax::Binary { .. }
            ),
            "{expression}"
        );
    }
}

#[test]
fn intrinsic_custom_quantity_parameters_forward_into_generic_quantity_contracts() {
    let source = "dimension wobble\ntype Wobble = quantity {dimension:wobble}\nunit wob/s : Wobble = {reference:origin,scale:1}\nunit doublewob/s : Wobble = {reference:wob/s,scale:2}\nplot forward (\n seed: Wobble\n target: Unit\n receipt: ExactQuantityConversionReceipt <= 8192B >>\n) {\n operation: units/convert(source=seed,to=target)\n operation.receipt >> receipt\n}\nplot demo (\n receipt: ExactQuantityConversionReceipt <= 8192B >>\n) {\n operation: forward(seed=3doublewob/s,target=wob/s)\n operation.receipt >> receipt\n}.\n";
    let expanded = expand(source, "demo");
    let gear = expanded
        .expanded
        .gears
        .iter()
        .find(|gear| gear.kind_id.as_str() == quantity_conversion::KIND)
        .unwrap();
    let receipt = quantity_conversion::prepare_configuration(&gear.configuration).unwrap();
    quantity_conversion::validate_receipt(&receipt).unwrap();
    let ConfigurationValue::Quantity(value) = &gear.configuration[0].value else {
        panic!("quantity")
    };
    assert_eq!(value.source(), "3doublewob/s");
    // Unit is independent and cannot satisfy even the generic Quantity contract.
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    quantity_conversion::install(&mut startup, &mut profiles).unwrap();
    let invalid = parse_syntax_document(
        &source
            .replace("seed: Wobble", "seed: Unit")
            .replace("seed=3doublewob/s", "seed=wob/s"),
    );
    assert!(check_syntax_document(&invalid, &startup).is_err());
}
