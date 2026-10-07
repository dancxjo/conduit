use conduit_core::{
    kind_id, port_id, ConfigurationValue, InfoBool, KindIdentity, PortDescriptor, PortDirection,
    PortTemporal, Quantity, QuantityUnit, BOOL_INFO_ID, TEMPERATURE_INFO_ID,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot, parse_syntax_document, KindProjection,
    KindSignature, PortableExpressionProgram, ProfileCatalog, StartupCatalog,
    PURE_EXPRESSION_REVISION,
};

#[test]
fn immutable_quantity_local_is_captured_by_the_lowered_expression() {
    let mut startup = StartupCatalog::new();
    for kind in ["test/temperature-source", "test/bool-sink"] {
        startup
            .insert(KindSignature {
                kind: kind.into(),
                startup_parameters: vec![],
            })
            .unwrap();
    }
    let port = |name, value_kind, direction| PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value_kind),
        direction,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    };
    let mut profile = ProfileCatalog::new();
    profile
        .insert(KindProjection {
            kind_id: kind_id("test/temperature-source"),
            kind_contract_revision: KindIdentity::from("test/temperature-source@1"),
            inputs: vec![],
            outputs: vec![port("out", TEMPERATURE_INFO_ID, PortDirection::Output)],
            configuration: vec![],
        })
        .unwrap();
    profile
        .insert(KindProjection {
            kind_id: kind_id("test/bool-sink"),
            kind_contract_revision: KindIdentity::from("test/bool-sink@1"),
            inputs: vec![port("in", BOOL_INFO_ID, PortDirection::Input)],
            outputs: vec![],
            configuration: vec![],
        })
        .unwrap();
    let source = "plot classify {\n limit = 30°C\n source: test/temperature-source\n sink: test/bool-sink\n source >> (. > limit) >> sink\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let expanded = expand_canonical_plot(&checked, "classify", &profile).unwrap();
    let expression = expanded
        .gears
        .iter()
        .find(|gear| gear.kind_contract_revision.as_str() == PURE_EXPRESSION_REVISION)
        .unwrap();
    let ConfigurationValue::Text(program) = &expression.configuration[0].value else {
        panic!("expression program")
    };
    let program = PortableExpressionProgram::from_canonical_hex(program).unwrap();
    assert_eq!(
        program
            .evaluate(&Quantity::new(31, QuantityUnit::Celsius).encode())
            .unwrap(),
        InfoBool::TRUE.encode()
    );
}

#[test]
fn exact_structured_startup_capture_is_checked_and_evaluated_as_owned_profile() {
    let source = "type ReceiptBytes = collection U8 = 3\ntype Mode =\n    cold\n    | warm\ntype Receipt = {\n bytes: ReceiptBytes\n mode: Mode\n}\nplot capture (\n selected: Receipt = {bytes: [1, 2, 255], mode: warm(\"\")}\n >> input: U8\n output: Receipt >>\n) = (selected)\n";
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let expanded = conduit_plot::expand_canonical_plot_for_authoring(
        &checked,
        "capture",
        &ProfileCatalog::new(),
    )
    .unwrap()
    .expanded;
    let expression = expanded
        .gears
        .iter()
        .find(|gear| gear.kind_contract_revision.as_str() == PURE_EXPRESSION_REVISION)
        .unwrap();
    let ConfigurationValue::Text(encoded) = &expression.configuration[0].value else {
        panic!("program");
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    let selected = checked.plots[0].startup_parameters[0]
        .default
        .as_ref()
        .unwrap();
    let conduit_plot::CanonicalStartupValue::Structured(value) = selected else {
        panic!("structured");
    };
    let expected = value.try_concrete().unwrap().canonical_bytes().unwrap();
    assert_eq!(program.evaluate(&[7]).unwrap(), expected);
    let mut prepared = conduit_plot::PreparedPortableExpressionEvaluator::new(&program).unwrap();
    assert_eq!(prepared.evaluate(&[7]).unwrap(), expected);
    assert_eq!(program.output_type, *value.value_type());
}

#[test]
fn structured_capture_refuses_foreign_profile_unresolved_and_unsupported_leaf() {
    for source in [
        "type Selected = collection U8 = 2\ntype Foreign = collection U8 = 2\nplot capture (\n selected: Selected = [1,2]\n >> input: U8\n output: Foreign >>\n) = (selected)\n",
        "type Selected = collection U8 = 2\nplot capture (\n selected: Selected\n >> input: U8\n output: Selected >>\n) = (selected)\n",
        "type Selected = {\n text: Text\n}\nplot capture (\n selected: Selected = {text: \"unsupported\"}\n >> input: U8\n output: Selected >>\n) = (selected)\n",
        "type Positive = U8 in 1..=3\ntype Selected = collection Positive = 2\nplot capture (\n selected: Selected = [0,2]\n >> input: U8\n output: Selected >>\n) = (selected)\n",
    ] {
        let result = check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new());
        let refused = match result {
            Err(_) => true,
            Ok(checked) => conduit_plot::expand_canonical_plot_for_authoring(&checked,"capture",&ProfileCatalog::new()).is_err(),
        };
        assert!(refused, "{source}");
    }
}

#[test]
fn repeated_structured_startup_projections_capture_only_exact_selected_leaves() {
    let source = "type ReceiptBytes = collection U8 = 3\ntype ReceiptMode =\n cold\n | warm\ntype Receipt = {\n bytes: ReceiptBytes\n mode: ReceiptMode\n}\nplot capture (\n selected: Receipt = {bytes: [1, 2, 255], mode: warm(\"\")}\n >> input: U8\n output: Boolean >>\n) = (. >= selected.bytes.0 && . <= selected.bytes.2 && selected.mode is warm)\n";
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let expanded = conduit_plot::expand_canonical_plot_for_authoring(
        &checked,
        "capture",
        &ProfileCatalog::new(),
    )
    .unwrap();
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("program")
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    let mut prepared = conduit_plot::PreparedPortableExpressionEvaluator::new(&program).unwrap();
    for (input, expected) in [(0, false), (1, true), (255, true)] {
        assert_eq!(
            program.evaluate(&[input]).unwrap(),
            InfoBool::new(expected).encode()
        );
        assert_eq!(
            prepared.evaluate(&[input]).unwrap(),
            InfoBool::new(expected).encode()
        );
    }
}
