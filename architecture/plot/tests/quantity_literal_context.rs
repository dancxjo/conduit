//! A resolved physical literal can inhabit generic Quantity without losing its Unit.
use conduit_core::*;
use conduit_plot::*;
fn program(output: &str, literal: &str) -> Result<PortableExpressionProgram, String> {
    let source =
        format!("plot literal (\n >> trigger: Empty\n result: {output} >>\n) = ({literal})\n");
    let syntax = parse_syntax_document(&source);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).map_err(|e| e.message)?;
    let expanded = expand_canonical_plot_for_authoring(&checked, "literal", &ProfileCatalog::new())
        .map_err(|e| e.message)?;
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("canonical expression")
    };
    PortableExpressionProgram::from_canonical_hex(encoded).map_err(|e| format!("{e:?}"))
}
#[test]
fn generic_quantity_literals_keep_the_resolved_capsule_in_reference_and_prepared_execution() {
    for literal in ["1m", "0.25turn", "21°C"] {
        let program = program("Quantity", literal).unwrap();
        assert_eq!(
            program.output_type,
            StructuredInfoType::leaf(kind_id(QUANTITY_INFO_ID)).unwrap()
        );
        let expected = Quantity::parse_plot_literal(literal).unwrap().encode();
        assert_eq!(program.evaluate(&[]).unwrap(), expected);
        assert_eq!(
            PreparedPortableExpressionEvaluator::new(&program)
                .unwrap()
                .evaluate(&[])
                .unwrap(),
            expected
        );
    }
}
#[test]
fn contextual_quantity_does_not_guess_units_or_weaken_named_dimensions() {
    for (output, literal) in [
        ("Quantity", "42"),
        ("Quantity", "m"),
        ("Unit", "1m"),
        ("Dimension", "1m"),
        ("Quantity", "3unknown"),
        ("Duration", "1m"),
        ("Distance", "1Hz"),
        ("Quantity", "1m + 1Hz"),
        ("Quantity", "21°C * 2"),
        ("Quantity", "2 * 21°C"),
        ("Quantity", "1m / 1cm"),
    ] {
        assert!(program(output, literal).is_err(), "{output}: {literal}");
    }
}

#[test]
fn generic_result_context_keeps_physical_operand_types_inside_nested_arithmetic() {
    let program = program("Quantity", "((1m / 1cm) + 2) * (1m * 1m)").unwrap();
    let reference = program.evaluate(&[]).unwrap();
    let prepared = PreparedPortableExpressionEvaluator::new(&program)
        .unwrap()
        .evaluate(&[])
        .unwrap()
        .to_vec();
    assert_eq!(prepared, reference);
    let quantity = Quantity::decode(&reference).unwrap();
    assert_eq!((quantity.coefficient(), quantity.exponent()), (102, 0));
    assert_eq!(quantity.dimension(), QuantityDimension::Area);
    assert_eq!(quantity.role(), QuantityRole::Linear);
}
