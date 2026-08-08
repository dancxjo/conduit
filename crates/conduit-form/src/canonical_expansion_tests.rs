use crate::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ConfigurationField,
    ConfigurationRule, ConfigurationValue, KindDefinition, OperationSignature, ProfileCatalog,
    StartupCatalog, StartupParameterSignature,
};
use conduit_core::{kind_id, port_id, KindContractRevision, PortDescriptor, PortDirection};

fn port(name: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id("test/value"),
        direction,
    }
}

fn catalogs() -> (StartupCatalog, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    for signature in [
        OperationSignature {
            operation: "test/source".into(),
            startup_parameters: vec![],
        },
        OperationSignature {
            operation: "test/pass".into(),
            startup_parameters: vec![StartupParameterSignature {
                name: "count".into(),
                value_type: "Count".into(),
                default: Some("1".into()),
            }],
        },
        OperationSignature {
            operation: "test/sink".into(),
            startup_parameters: vec![],
        },
    ] {
        startup.insert(signature).unwrap();
    }
    let mut profile = ProfileCatalog::new();
    for definition in [
        KindDefinition {
            kind_id: kind_id("test/source"),
            kind_contract_revision: KindContractRevision::from("test/source@1"),
            inputs: vec![],
            outputs: vec![port("out", PortDirection::Output)],
            configuration: vec![],
        },
        KindDefinition {
            kind_id: kind_id("test/pass"),
            kind_contract_revision: KindContractRevision::from("test/pass@1"),
            inputs: vec![port("in", PortDirection::Input)],
            outputs: vec![port("out", PortDirection::Output)],
            configuration: vec![ConfigurationField {
                key: "count".into(),
                default_value: ConfigurationValue::U64(1),
                validation: ConfigurationRule::U64Range {
                    minimum: 1,
                    maximum: 8,
                },
            }],
        },
        KindDefinition {
            kind_id: kind_id("test/sink"),
            kind_contract_revision: KindContractRevision::from("test/sink@1"),
            inputs: vec![port("in", PortDirection::Input)],
            outputs: vec![],
            configuration: vec![],
        },
    ] {
        profile.insert(definition).unwrap();
    }
    (startup, profile)
}

fn expand(source: &str, root: &str) -> crate::ExpandedCanonicalForm {
    let (startup, profile) = catalogs();
    let syntax = parse_syntax_document(source);
    let checked = check_syntax_document(&syntax, &startup).expect("source checks");
    expand_canonical_form(&checked, root, &profile).expect("source expands")
}

#[test]
fn parameterized_form_flattens_to_ordinary_primitive_graph() {
    let source = "form relay (\n count: Count = 1\n input: test/value > output: test/value\n) {\n pass: test/pass(count)\n input > pass > output\n}\n\nform main {\n source: test/source\n relay: relay(2)\n sink: test/sink\n source > relay > sink\n}\n";
    let expanded = expand(source, "main");

    assert_eq!(
        expanded
            .operations
            .iter()
            .map(|operation| operation.operation_id.as_str())
            .collect::<Vec<_>>(),
        ["main/relay/pass", "main/sink", "main/source"]
    );
    let pass = expanded
        .operations
        .iter()
        .find(|operation| operation.kind_id.as_str() == "test/pass")
        .unwrap();
    assert_eq!(pass.configuration[0].value, ConfigurationValue::U64(2));
    assert_eq!(expanded.connections.len(), 2);
    assert_eq!(expanded.provenance[0].source_form, "relay");
    assert_ne!(
        expanded.checked_form_id.as_str(),
        expanded.expanded_form_id.as_str()
    );
}

#[test]
fn nested_expansion_and_source_reordering_have_deterministic_identity() {
    let first = "form inner (\n input: test/value > output: test/value\n) {\n pass: test/pass\n input > pass > output\n}\n\nform outer (\n input: test/value > output: test/value\n) {\n inner: inner\n input > inner > output\n}\n\nform main {\n source: test/source\n outer: outer\n sink: test/sink\n source > outer > sink\n}\n";
    let reordered = "form main {\n source > outer > sink\n sink: test/sink\n outer: outer\n source: test/source\n}\n\nform outer (\n input: test/value > output: test/value\n) {\n input > inner > output\n inner: inner\n}\n\nform inner (\n input: test/value > output: test/value\n) {\n input > pass > output\n pass: test/pass\n}\n";
    let first = expand(first, "main");
    let reordered = expand(reordered, "main");
    assert_eq!(first.checked_form_id, reordered.checked_form_id);
    assert_eq!(first.expanded_form_id, reordered.expanded_form_id);
    assert_eq!(first.operations, reordered.operations);
    assert_eq!(first.connections, reordered.connections);
}

#[test]
fn recursion_and_expansion_depth_fail_with_distinct_diagnostics() {
    let (startup, profile) = catalogs();
    let recursive = parse_syntax_document("form a {\n child: b\n}\n\nform b {\n child: a\n}\n");
    let checked = check_syntax_document(&recursive, &startup).unwrap();
    let error = expand_canonical_form(&checked, "a", &profile).unwrap_err();
    assert_eq!(error.code, "CND-FRM-035");
    assert!(error.message.contains("a -> b -> a"));

    let mut source = String::new();
    for index in 0..=crate::MAXIMUM_FORM_NESTING_DEPTH + 1 {
        source.push_str(&format!("form f{index} {{\n"));
        if index <= crate::MAXIMUM_FORM_NESTING_DEPTH {
            source.push_str(&format!(" child: f{}\n", index + 1));
        }
        source.push_str("}\n\n");
    }
    let syntax = parse_syntax_document(&source);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let error = expand_canonical_form(&checked, "f0", &profile).unwrap_err();
    assert_eq!(error.code, "CND-FRM-034");
}

#[test]
fn reusable_form_without_declared_shorthand_requires_named_port() {
    let source = "form source (\n value: test/value >\n) {\n primitive: test/source\n primitive > value\n}\n\nform main {\n source: source\n sink: test/sink\n source > sink\n}\n";
    let (startup, profile) = catalogs();
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let error = expand_canonical_form(&checked, "main", &profile).unwrap_err();
    assert_eq!(error.code, "CND-FRM-044");
}

#[test]
fn primitive_contract_bounds_and_face_types_fail_closed() {
    let bounded = "form main {\n source: test/source\n pass: test/pass(9)\n sink: test/sink\n source > pass > sink\n}\n";
    let (startup, profile) = catalogs();
    let checked = check_syntax_document(&parse_syntax_document(bounded), &startup).unwrap();
    assert_eq!(
        expand_canonical_form(&checked, "main", &profile)
            .unwrap_err()
            .code,
        "CND-FRM-040"
    );

    let wrong_face = "form relay (\n input: wrong/value > output: wrong/value\n) {\n pass: test/pass\n input > pass > output\n}\n\nform main {\n source: test/source\n relay: relay\n sink: test/sink\n source > relay > sink\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(wrong_face), &startup).unwrap();
    assert_eq!(
        expand_canonical_form(&checked, "main", &profile)
            .unwrap_err()
            .code,
        "CND-FRM-045"
    );
}

#[test]
fn public_input_fanout_flattens_to_explicit_ordinary_connections() {
    let source = "form fan (\n > input: test/value\n) {\n left: test/pass\n right: test/pass\n input > left\n input > right\n}\n\nform main {\n source: test/source\n fan: fan\n source > fan.input\n}\n";
    let expanded = expand(source, "main");
    assert_eq!(expanded.connections.len(), 2);
    assert!(expanded.connections.iter().all(|connection| {
        connection.source_operation_id.as_str() == "main/source"
            && matches!(
                connection.sink_operation_id.as_str(),
                "main/fan/left" | "main/fan/right"
            )
    }));
}

#[test]
fn expanded_identity_rejects_graph_contract_and_provenance_mutation() {
    let source = "form main {\n source: test/source\n sink: test/sink\n source > sink\n}\n";
    let baseline = expand(source, "main");

    let mut operation = baseline.clone();
    operation.operations[0].kind_contract_revision = KindContractRevision::from("mutated@1");
    assert_eq!(
        operation.validate_expansion().unwrap_err().code,
        "CND-FRM-049"
    );

    let mut cord = baseline.clone();
    cord.connections[0].sink_port_id = port_id("mutated");
    assert_eq!(cord.validate_expansion().unwrap_err().code, "CND-FRM-049");

    let mut provenance = baseline;
    provenance.provenance[0].source_cell = "substituted".into();
    assert_eq!(
        provenance.validate_expansion().unwrap_err().code,
        "CND-FRM-049"
    );

    let mut span = expand(source, "main");
    span.provenance[0].source_span.start += 1;
    assert_eq!(span.validate_expansion().unwrap_err().code, "CND-FRM-049");
}

#[test]
fn inline_reusable_and_primitive_cells_expand_without_a_parallel_path() {
    let source = "form relay (\n input: test/value > output: test/value\n) {\n input > test/pass > output\n}\n\nform main {\n test/source > relay() > test/sink\n}\n";
    let expanded = expand(source, "main");
    assert_eq!(expanded.operations.len(), 3);
    assert_eq!(expanded.connections.len(), 2);
    assert!(expanded
        .provenance
        .iter()
        .all(|row| row.source_cell.starts_with("inline-")));
}
