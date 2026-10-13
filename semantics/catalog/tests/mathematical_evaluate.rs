//! Source-bound ordered computations and independently sealed consumer requirements.
#![cfg(feature = "plot-catalog")]
use conduit_core::{
    ConfigurationValue, MathValue, NumericComputation, NumericComputedResult, NumericRequirement,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};

fn requirement(representation: &str) -> String {
    format!(
        r#"{{ semantics: mathematical(empty), representation: {representation}(empty),
      rounding: nearestEven(empty), accuracy: exact(empty),
      budget: {{ storageBytes: 65536, workSteps: 100000, limbs: 128, symbolicNodes: 128 }},
      parameters: [], reproducible: true,
      exceptions: {{ invalid: false, divisionByZero: false, overflow: false,
        underflow: false, inexact: false }} }}"#
    )
}
fn expand(expression: &str, representation: &str) -> conduit_plot::ExpandedCanonicalPlot {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_mathematical_kinds(&mut startup, &mut profile).unwrap();
    let source = format!(
        "plot main (\n) {{\n evaluation: math/evaluate(value = {expression}, requirement = {})\n}}.\n",
        requirement(representation).split_whitespace().collect::<Vec<_>>().join(" ")
    );
    let syntax = parse_syntax_document(&source);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    expand_canonical_plot(&checked, "main", &profile).unwrap()
}
fn graph(plot: &conduit_plot::ExpandedCanonicalPlot) -> NumericComputation {
    let ConfigurationValue::Structured(value) = &plot.gears[0]
        .configuration
        .iter()
        .find(|entry| entry.key == "numeric_computation")
        .unwrap()
        .value
    else {
        panic!("typed graph");
    };
    let structured =
        conduit_core::StructuredInfoValue::from_canonical_bytes(value.canonical_value()).unwrap();
    let conduit_core::StructuredInfoValueShape::Record(fields) = structured.shape() else {
        panic!("graph record");
    };
    let leaf = |name: &str| {
        fields
            .iter()
            .find(|field| field.name() == name)
            .and_then(|field| match field.value().shape() {
                conduit_core::StructuredInfoValueShape::Leaf(bytes) => Some(bytes),
                _ => None,
            })
            .unwrap()
    };
    NumericComputation::from_configuration(value, leaf("program_identity").try_into().unwrap())
        .unwrap()
}
#[test]
fn decimal_source_retains_order_and_exact_denotation() {
    let sum = expand("0.1 + 0.2", "rational");
    let literal = expand("0.3", "rational");
    let sum_graph = graph(&sum);
    assert_ne!(
        sum_graph.identity().unwrap(),
        graph(&literal).identity().unwrap()
    );
    let NumericComputedResult::Number(result) = sum_graph.evaluate_exact(&[]).unwrap() else {
        panic!("Scalar");
    };
    assert_eq!(result.value, MathValue::parse_decimal("0.3").unwrap());
}
#[test]
fn same_computation_has_independent_consumer_requirement_custody() {
    let rational = expand("0.1 + 0.2", "rational");
    let symbolic = expand("0.1 + 0.2", "symbolic");
    assert_eq!(graph(&rational), graph(&symbolic));
    assert_ne!(rational.gears[0].kind_id, symbolic.gears[0].kind_id);
    for plot in [&rational, &symbolic] {
        let ConfigurationValue::Structured(requirement) = &plot.gears[0]
            .configuration
            .iter()
            .find(|entry| entry.key == "numeric_requirement")
            .unwrap()
            .value
        else {
            panic!("ordinary requirement");
        };
        NumericRequirement::from_configuration(requirement).unwrap();
    }
}
#[test]
fn pi_root_and_large_integer_use_the_same_source_boundary() {
    for expression in ["pi", "math/sqrt(2)", "math/pow(10, 1000)"] {
        graph(&expand(expression, "symbolic"))
            .evaluate_exact(&[])
            .unwrap();
    }
}

#[test]
fn mathematical_source_refuses_domains_and_retains_boolean_result_type() {
    let boolean = expand("0.1 + 0.2 == 0.3", "rational");
    assert_eq!(
        boolean.gears[0].outputs[0].value_kind.as_str(),
        conduit_core::BOOL_INFO_ID
    );
    assert!(matches!(
        graph(&boolean).evaluate_exact(&[]).unwrap(),
        NumericComputedResult::Boolean(true)
    ));
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_mathematical_kinds(&mut startup, &mut profile).unwrap();
    for expression in ["1 / 0", "math/sqrt(-1)", "math/sin(42)", ". + 1"] {
        let source = format!(
            "plot main (\n) {{\n evaluation: math/evaluate(value = {expression}, requirement = {}) }}.\n",
            requirement("rational")
        );
        assert!(
            check_syntax_document(&parse_syntax_document(&source), &startup).is_err(),
            "{expression}"
        );
    }
}

#[test]
fn exact_angle_constructor_retains_coordinate_computation_and_unit_law() {
    let plot = expand("math/sin(Angle(pi / 2, rad))", "symbolic");
    let graph = graph(&plot);
    assert!(graph.nodes().iter().any(|node| matches!(node.operation,
        conduit_core::NumericComputationOperation::Apply {
            operation: conduit_core::NumericOperation::QuantityConstruction, ..
        })));
    let NumericComputedResult::Number(result) = graph.evaluate_exact(&[]).unwrap() else {
        panic!("Scalar result");
    };
    assert_eq!(result.value, MathValue::ONE);
    assert_eq!(result.semantic_kind, conduit_core::SCALAR_INFO_ID);
}

#[test]
fn explicit_ieee_source_retains_software_literal_rounding_and_operational_order() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_mathematical_kinds(&mut startup, &mut profile).unwrap();
    let requirement = requirement("ieeeBinary64").replace("mathematical(empty)", "ieeeOperational(empty)").split_whitespace().collect::<Vec<_>>().join(" ");
    let source = format!("plot main (\n) {{\n evaluation: math/evaluate(value = F64(0.1) + F64(0.2) == F64(0.3), requirement = {requirement})\n}}.");
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    let expanded = expand_canonical_plot(&checked, "main", &profile).unwrap();
    let graph = graph(&expanded);
    assert_eq!(graph.nodes()[graph.root() as usize].semantic_kind, conduit_core::BOOL_INFO_ID);
    assert_eq!(graph.nodes().iter().filter(|node| matches!(node.operation,
        conduit_core::NumericComputationOperation::IeeeRoundedLiteral { .. })).count(), 3);
    assert!(graph.evaluate_exact(&[]).is_err());
}

#[test]
fn computed_quantity_startup_retains_checked_coordinate_proof() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_plot::quantity_conversion::install(&mut startup, &mut profile).unwrap();
    let source = "plot main (\n receipt: ExactQuantityConversionReceipt <= 8192B >>\n) {\n convert: units/convert(source = Angle(pi / 2, rad), to = turn)\n convert.receipt >> receipt\n}.\n";
    let syntax = parse_syntax_document(source);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let expanded = conduit_plot::expand_canonical_plot_for_authoring(&checked, "main", &profile).unwrap().expanded;
    let source = expanded.gears[0].configuration.iter().find(|field| field.key == "source").unwrap();
    let ConfigurationValue::Quantity(quantity) = &source.value else { panic!("checked quantity") };
    assert!(quantity.computation().is_some());
    assert_eq!(quantity.value().convert_to_unit(&conduit_core::Unit::Turn).unwrap(), MathValue::parse_decimal("0.25").unwrap());
}
