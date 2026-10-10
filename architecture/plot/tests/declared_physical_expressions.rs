use conduit_core::{ConfigurationValue, InfoBool, Quantity, Unit};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, ProfileCatalog, StartupCatalog,
};

fn program(declarations: &str, input_type: &str, expression: &str) -> PortableExpressionProgram {
    let source=format!("{declarations}\nplot expression (\n >> input: {input_type} <= 788B\n result: Boolean >>\n) {{\n input >> ({expression}) >> result\n}}.\n");
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "expression", &ProfileCatalog::new())
            .unwrap();
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("program")
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
#[test]
fn custom_quantity_capsule_survives_expression_lowering_without_ambient_catalogue() {
    let declarations="dimension wobble\ntype Wobble = quantity { dimension: wobble }\nunit wob/s : Wobble = {reference: origin,scale:1}\nunit doublewob/s : Wobble = {reference:wob/s,scale:2}";
    let program = program(declarations, "Wobble", ". == 6wob/s");
    let catalog = conduit_plot::checked_physical_catalog_for_document(
        &parse_syntax_document(declarations),
        &StartupCatalog::new(),
    )
    .unwrap();
    let Some(conduit_plot::CanonicalStartupValue::Quantity(value)) =
        conduit_plot::parse_checked_physical_value("3doublewob/s", None, &catalog).unwrap()
    else {
        panic!("quantity")
    };
    assert_eq!(
        program.evaluate(&value.value().encode()),
        Ok(InfoBool::new(true).encode().to_vec())
    );
    let mut prepared = conduit_plot::PreparedPortableExpressionEvaluator::new(&program).unwrap();
    assert_eq!(
        prepared.evaluate(&value.value().encode()).unwrap(),
        InfoBool::new(true).encode()
    );
    assert!(prepared
        .evaluate(&Quantity::new(6, Unit::Hertz).encode())
        .is_err());
    let encoded = program.canonical_bytes().unwrap();
    let retained = PortableExpressionProgram::from_canonical_bytes(&encoded).unwrap();
    assert_eq!(
        retained.evaluate(&value.value().encode()),
        Ok(InfoBool::new(true).encode().to_vec())
    );
}
#[test]
fn unit_aliases_compare_physical_laws_but_never_coerce_to_dimensionless_quantities() {
    let program = program(
        "unit span : Distance = {reference:m,scale:2,prefixes:si}",
        "Unit",
        ". == uspan",
    );
    let catalog = conduit_plot::checked_physical_catalog_for_document(
        &parse_syntax_document("unit span : Distance = {reference:m,scale:2,prefixes:si}"),
        &StartupCatalog::new(),
    )
    .unwrap();
    let Some(conduit_plot::CanonicalStartupValue::Unit(value)) =
        conduit_plot::parse_checked_physical_value("µspan", None, &catalog).unwrap()
    else {
        panic!("unit")
    };
    assert_eq!(
        program.evaluate(&value.value().encode()),
        Ok(InfoBool::new(true).encode().to_vec())
    );
    let mut prepared = conduit_plot::PreparedPortableExpressionEvaluator::new(&program).unwrap();
    assert_eq!(
        prepared.evaluate(&value.value().encode()).unwrap(),
        InfoBool::new(true).encode()
    );
    for expression in [". == one", ". * one"] {
        let syntax=parse_syntax_document(&format!("plot invalid (\n >> input: Ratio <= 788B\n result: Boolean >>\n) {{\n input >> ({expression}) >> result\n}}.\n"));
        let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
        assert!(
            expand_canonical_plot_for_authoring(&checked, "invalid", &ProfileCatalog::new())
                .is_err()
        );
    }
    assert!(Quantity::new(1, Unit::One).role() == conduit_core::QuantityRole::Linear);
}
