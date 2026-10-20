//! Generic physical arithmetic retains concrete Unit and Delta role evidence.
use conduit_core::*;
use conduit_plot::*;

fn compile(kind: &str, body: &str) -> PortableExpressionProgram {
    let source = format!(
        "plot sum (\n >> value: {kind} <= 33829B\n result: {kind} <= 33829B >>\n) = ({body})\n"
    );
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "sum", &ProfileCatalog::new()).unwrap();
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!()
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}

#[test]
fn native_and_rational_outputs_keep_a_difference_in_the_celsius_coordinate_convention() {
    for (body, expected) in [
        (". + TemperatureDelta(1, °C)", (11, -1)),
        (". * 2", (2, -1)),
    ] {
        let program = compile("TemperatureDelta", body);
        let input = Quantity::parse_plot_literal("TemperatureDelta(0.1, °C)").unwrap();
        let native = Quantity::decode(&program.evaluate(&input.encode()).unwrap()).unwrap();
        assert_eq!((native.coefficient(), native.exponent()), expected);
        assert_eq!(native.role(), QuantityRole::Delta);
        assert_eq!(native.unit(), Unit::resolve("°C").unwrap());
        let mut prepared =
            PreparedPortableExpressionEvaluator::new_with_exact_quantities(&program).unwrap();
        let encoded = prepared.evaluate(&input.encode()).unwrap();
        let value = QuantityValueProfile::Exact
            .decode(
                BUILTIN_TEMPERATURE_DELTA_INFO_ID,
                encoded,
                &mut ExactNumericWork::new(100_000_000),
            )
            .unwrap();
        assert_eq!(value.role(), QuantityRole::Delta);
        assert_eq!(value.unit(), input.unit());
    }
}

#[test]
fn a_generic_input_does_not_erase_the_literals_physical_type_or_bypass_compatibility() {
    let program = compile("Quantity", ". + 1m");
    let valid = Quantity::parse_plot_literal("0.1m").unwrap();
    let foreign = Quantity::parse_plot_literal("0.1Hz").unwrap();
    let result = Quantity::decode(&program.evaluate(&valid.encode()).unwrap()).unwrap();
    assert_eq!((result.coefficient(), result.exponent()), (11, -1));
    assert!(program.evaluate(&foreign.encode()).is_err());
    let mut prepared =
        PreparedPortableExpressionEvaluator::new_with_exact_quantities(&program).unwrap();
    assert!(prepared.evaluate(&foreign.encode()).is_err());
    assert!(prepared.evaluate(&valid.encode()).is_ok());
}
