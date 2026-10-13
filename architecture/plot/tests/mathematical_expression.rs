//! Exact denotation admission through authored Plot and the ordinary checker.
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};

fn checked(source: &str) -> Result<conduit_plot::ExpandedAuthoringPlot, String> {
    let startup = StartupCatalog::new();
    let syntax = parse_syntax_document(source);
    if let Some(error) = syntax.diagnostics.first() {
        return Err(error.message.clone());
    }
    let syntax = check_syntax_document(&syntax, &startup).map_err(|e| format!("{e:?}"))?;
    expand_canonical_plot_for_authoring(&syntax, "demo", &ProfileCatalog::new())
        .map_err(|e| format!("{e:?}"))
}

#[test]
fn exact_rational_decimal_and_large_integer_operations_enter_checked_plots() {
    for expression in [
        "1 / 3 + 1 / 3 + 1 / 3",
        "0.1 + 0.2",
        "math/pow(10, 1000) + 1",
        "pi / 180",
        "math/sqrt(2)",
    ] {
        let source = format!("plot demo (\n input: Scalar >> output: Scalar\n) {{\n input >> ({expression}) >> output\n}}.\n");
        assert!(
            checked(&source).is_ok(),
            "{expression}: {:?}",
            checked(&source)
        );
    }
}

#[test]
fn ordinary_trig_never_infers_an_angle_from_scalar_or_non_angle_quantity() {
    for expression in ["math/sin(42)", "math/sin(1m)", "math/sin(1Hz)"] {
        let source = format!("plot demo (\n input: Scalar >> output: Scalar\n) {{\n input >> ({expression}) >> output\n}}.\n");
        assert!(checked(&source).is_err(), "{expression}");
    }
}

#[test]
fn exact_equality_retains_the_whole_authored_computation() {
    for expression in ["1 / 3 + 1 / 3 + 1 / 3 == 1", "0.1 + 0.2 == 0.3"] {
        let source = format!("plot demo (\n input: Scalar >> output: Boolean\n) {{\n input >> ({expression}) >> output\n}}.\n");
        let expanded = checked(&source).unwrap();
        let gear = &expanded.expanded.gears[0];
        let program = gear
            .configuration
            .iter()
            .find(|entry| entry.key == "program")
            .unwrap();
        let graph = gear
            .configuration
            .iter()
            .find(|entry| entry.key == "numeric_computation")
            .unwrap();
        let conduit_core::ConfigurationValue::Text(program) = &program.value else {
            panic!("program custody");
        };
        let conduit_core::ConfigurationValue::Structured(graph) = &graph.value else {
            panic!("typed graph custody");
        };
        let graph = conduit_core::NumericComputation::from_configuration(
            graph,
            conduit_plot::numeric_program_identity(program),
        )
        .unwrap();
        assert_eq!(
            graph.evaluate_exact(&[]).unwrap(),
            conduit_core::NumericComputedResult::Boolean(true)
        );
        assert!(
            graph.nodes().len() > 1,
            "operations must not collapse to the final answer"
        );
        assert_eq!(gear.startup_parameters.len(), 2);
    }
}

#[test]
fn undefined_mathematical_domains_refuse_before_play() {
    for expression in [
        "1 / 0",
        "math/sqrt(-1)",
        "math/tan(0.25turn)",
        "math/asin(2)",
        "math/atan2(0m, 0m)",
    ] {
        let output = if expression.starts_with("math/asin") || expression.starts_with("math/atan2")
        {
            "Angle"
        } else {
            "Scalar"
        };
        let source = format!("plot demo (\n input: Scalar >> output: {output}\n) {{\n input >> ({expression}) >> output\n}}.\n");
        let error = checked(&source).unwrap_err();
        assert!(error.contains("CND-MATH-001"), "{expression}: {error}");
    }
}

#[test]
fn cardinal_trig_and_compatible_atan2_use_captured_unit_laws() {
    for expression in [
        "math/sin(90°) == 1",
        "math/cos(0.5turn) == -1",
        "math/sin(0turn) == 0",
    ] {
        let source = format!("plot demo (\n input: Scalar >> output: Boolean\n) {{\n input >> ({expression}) >> output\n}}.\n");
        let expanded = checked(&source).unwrap();
        let gear = &expanded.expanded.gears[0];
        let conduit_core::ConfigurationValue::Text(program) = &gear.configuration[0].value else {
            panic!("program");
        };
        let conduit_core::ConfigurationValue::Structured(graph) = &gear.configuration[1].value
        else {
            panic!("graph");
        };
        let graph = conduit_core::NumericComputation::from_configuration(
            graph,
            conduit_plot::numeric_program_identity(program),
        )
        .unwrap();
        assert_eq!(
            graph.evaluate_exact(&[]).unwrap(),
            conduit_core::NumericComputedResult::Boolean(true)
        );
    }
    let source = "plot demo (\n input: Scalar >> output: Angle\n) {\n input >> (math/atan2(3m, 400cm)) >> output\n}.\n";
    assert!(checked(source).is_ok());
    let source = source.replace("400cm", "4s");
    assert!(checked(&source).is_err());
}
