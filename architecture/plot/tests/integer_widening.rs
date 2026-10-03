use conduit_core::ConfigurationValue;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog,
};

fn program(input: &str, output: &str, expression: &str) -> PortableExpressionProgram {
    let source = format!("plot widen (\n value: {input} >> result: {output}\n) = {expression}\n");
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let authoring =
        expand_canonical_plot_for_authoring(&checked, "widen", &ProfileCatalog::new()).unwrap();
    assert_eq!(
        authoring.expanded.gears.len(),
        1,
        "intrinsic widening is one pure operation"
    );
    let ConfigurationValue::Text(encoded) = &authoring.expanded.gears[0].configuration[0].value
    else {
        panic!("pure expression program");
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}

#[test]
fn widening_expands_without_a_fictitious_catalog_kind_and_prepares_once() {
    for (input, output, expression, bytes, expected) in [
        (
            "U32",
            "U64",
            "value/u64(.)",
            u32::MAX.to_le_bytes().to_vec(),
            u64::from(u32::MAX).to_le_bytes().to_vec(),
        ),
        (
            "I8",
            "I16",
            "value/i16(.)",
            i8::MIN.to_le_bytes().to_vec(),
            i16::from(i8::MIN).to_le_bytes().to_vec(),
        ),
        (
            "U8",
            "I16",
            "value/i16(.)",
            u8::MAX.to_le_bytes().to_vec(),
            i16::from(u8::MAX).to_le_bytes().to_vec(),
        ),
        (
            "U64",
            "I128",
            "value/i128(.)",
            u64::MAX.to_le_bytes().to_vec(),
            i128::from(u64::MAX).to_le_bytes().to_vec(),
        ),
    ] {
        let p = program(input, output, expression);
        assert_eq!(p.evaluate(&bytes).unwrap(), expected);
        let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
        let capacity = prepared.output_capacity();
        for _ in 0..1000 {
            assert_eq!(prepared.evaluate(&bytes).unwrap(), expected);
            assert_eq!(prepared.output_capacity(), capacity);
        }
    }
}

#[test]
fn nested_widening_and_arithmetic_stay_in_the_same_pure_operation() {
    let p = program("U8", "U64", "(value/u64(value/u16(.)) * 256 + 33)");
    let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    assert_eq!(prepared.evaluate(&[255]).unwrap(), &65313_u64.to_le_bytes());
}

#[test]
fn narrowing_same_width_and_signed_to_unsigned_remain_refused() {
    for (input, output) in [("U64", "U32"), ("U8", "U8"), ("U8", "I8"), ("I8", "U16")] {
        let call = output.to_lowercase();
        let source =
            format!("plot bad (\n value: {input} >> result: {output}\n) = value/{call}(.)\n");
        let syntax = parse_syntax_document(&source);
        let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
        assert!(
            expand_canonical_plot_for_authoring(&checked, "bad", &ProfileCatalog::new()).is_err()
        );
    }
}

#[test]
fn serialized_program_cannot_change_widening_target_or_arity() {
    let mut p = program("U32", "U64", "value/u64(.)");
    let conduit_plot::PortableExpressionOperation::SemanticCall { kind, .. } =
        &mut p.root.operation
    else {
        panic!("intrinsic call");
    };
    *kind = "value/i64".into();
    assert!(PreparedPortableExpressionEvaluator::new(&p).is_err());
    let conduit_plot::PortableExpressionOperation::SemanticCall { kind, arguments } =
        &mut p.root.operation
    else {
        unreachable!();
    };
    *kind = "value/u64".into();
    arguments.clear();
    assert!(PreparedPortableExpressionEvaluator::new(&p).is_err());
}

#[test]
fn widening_around_a_real_semantic_call_keeps_that_calls_identity() {
    use conduit_core::{
        kind_id, port_id, CapabilityLimits, Kind, KindIdentity, PortDescriptor, PortDirection,
        PortTemporal,
    };
    let port = |name, direction| PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id("value/u8"),
        direction,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    };
    let mut profile = ProfileCatalog::new();
    profile
        .insert_kind(Kind {
            kind_id: kind_id("test/identity"),
            kind_contract_revision: KindIdentity::from("test/identity@1"),
            startup_parameters: vec![],
            shorthand: Some((port_id("value"), port_id("result"))),
            inputs: vec![port("value", PortDirection::Input)],
            outputs: vec![port("result", PortDirection::Output)],
            configuration: vec![],
            semantic_laws: conduit_plot::pure_expression_semantic_laws(),
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: 1,
            },
        })
        .unwrap();
    let source = "plot widen (\n value: U8 >> result: U64\n) = value/u64(test/identity(.))\n";
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, "widen", &profile)
        .unwrap()
        .expanded;
    assert_eq!(expanded.gears.len(), 2);
    assert_eq!(
        expanded
            .gears
            .iter()
            .filter(|gear| gear.kind_id.as_str() == "test/identity")
            .count(),
        1
    );
    assert_eq!(expanded.connections.len(), 1);
    expanded.validate_expansion().unwrap();
}
