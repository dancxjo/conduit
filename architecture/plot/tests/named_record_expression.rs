use conduit_core::{
    ConfigurationValue, StructuredInfoTypeShape, StructuredInfoValue, StructuredInfoValueShape,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog,
};

fn program(body: &str) -> Result<PortableExpressionProgram, String> {
    let source = format!("type Pair = {{\n left: I128\n right: I128\n}}\nplot pair (\n value: I128 >> pair: Pair\n) = ({body})");
    let syntax = parse_syntax_document(&source);
    if !syntax.diagnostics.is_empty() {
        return Err(format!("{:?}", syntax.diagnostics));
    }
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, "pair", &ProfileCatalog::new())
        .map_err(|error| format!("{error:?}"))?
        .expanded;
    let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
        panic!("expression")
    };
    Ok(PortableExpressionProgram::from_canonical_hex(encoded).unwrap())
}

#[test]
fn contextual_record_constructor_retains_declared_identity_and_exact_members() {
    let p = program("{ left: ., right: . + 1 }").unwrap();
    let StructuredInfoTypeShape::Record { schema, .. } = p.output_type.shape() else {
        panic!("record")
    };
    assert!(!schema.as_str().contains("anonymous-record"));
    let output = p.evaluate(&7_i128.to_le_bytes()).unwrap();
    let value = StructuredInfoValue::from_canonical_bytes(&output).unwrap();
    assert_eq!(value.value_type(), &p.output_type);
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("fields")
    };
    assert_eq!(
        fields[0].value().shape(),
        StructuredInfoValueShape::Leaf(&7_i128.to_le_bytes())
    );
    let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    assert_eq!(prepared.evaluate(&7_i128.to_le_bytes()).unwrap(), output);
    for body in [
        "{ left: . }",
        "{ left: ., right: ., extra: . }",
        "{ left: ., right: true }",
        "{ left: ., left: . }",
    ] {
        assert!(program(body).is_err(), "{body}");
    }
}

#[test]
fn nested_constructor_keeps_both_native_record_schemas() {
    let source = "type Inner = {\n value: I128\n}\ntype Outer = {\n inner: Inner\n}\nplot nested (\n input: I128 >> output: Outer\n) = ({ inner: { value: . } })";
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, "nested", &ProfileCatalog::new())
        .unwrap()
        .expanded;
    let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
        panic!("expression")
    };
    let p = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    let output = p.evaluate(&42_i128.to_le_bytes()).unwrap();
    let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    assert_eq!(prepared.evaluate(&42_i128.to_le_bytes()).unwrap(), output);
    let value = StructuredInfoValue::from_canonical_bytes(&output).unwrap();
    assert_eq!(value.value_type(), &p.output_type);
}

fn custom(source: &str, entry: &str) -> Result<PortableExpressionProgram, String> {
    let syntax = parse_syntax_document(source);
    if !syntax.diagnostics.is_empty() {
        return Err(format!("{:?}", syntax.diagnostics));
    }
    let checked = check_syntax_document(&syntax, &StartupCatalog::new())
        .map_err(|error| format!("{error:?}"))?;
    let expanded = expand_canonical_plot_for_authoring(&checked, entry, &ProfileCatalog::new())
        .map_err(|error| format!("{error:?}"))?
        .expanded;
    let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
        panic!("expression")
    };
    Ok(PortableExpressionProgram::from_canonical_hex(encoded).unwrap())
}

#[test]
fn variants_keep_case_payload_identity_and_do_not_evaluate_unselected_failure() {
    let prelude =
        "type Reply =\\n completed {\\n value: U8\\n }\\n | refused\\n".replace("\\n", "\n");
    let source = format!("{prelude}plot reply (\n input: U8 >> reply: Reply\n) = (. == 0 ? refused(empty) : completed({{ value: 1 / . }}))");
    let p = custom(&source, "reply").unwrap();
    let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    for input in [0_u8, 1, 255] {
        let output = prepared.evaluate(&[input]).unwrap();
        assert_eq!(output, p.evaluate(&[input]).unwrap());
        let value = StructuredInfoValue::from_canonical_bytes(output).unwrap();
        assert_eq!(value.value_type(), &p.output_type);
        let StructuredInfoValueShape::Variant { tag, .. } = value.shape() else {
            panic!("variant")
        };
        assert_eq!(tag, if input == 0 { "refused" } else { "completed" });
    }
    for body in [
        "unknown(empty)",
        "refused(true)",
        "completed({ value: true })",
        "completed({ other: . })",
    ] {
        let source = format!("{prelude}plot reply (\n input: U8 >> reply: Reply\n) = ({body})");
        assert!(custom(&source, "reply").is_err(), "{body}");
    }
}

#[test]
fn sequence_literals_obey_declared_bounds_and_allow_exact_empty_payloads() {
    for (bounds, literal, accepted) in [
        ("<= 2", "[]", true),
        ("<= 2", "[.]", true),
        ("in 1..=2", "[]", false),
        ("<= 2", "[.,.,.]", false),
        ("<= 2", "[true]", false),
    ] {
        let source = format!("type Packet = {{\n octets: sequence U8 {bounds}\n}}\nplot packet (\n input: U8 >> packet: Packet\n) = ({{ octets: {literal} }})");
        let result = custom(&source, "packet");
        assert_eq!(result.is_ok(), accepted, "{literal}: {result:?}");
        if let Ok(p) = result {
            let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
            assert_eq!(
                prepared.evaluate(&[255]).unwrap(),
                p.evaluate(&[255]).unwrap()
            );
        }
    }
}

#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

#[test]
fn finite_constructor_play_never_allocates_or_grows_with_reuse() {
    let source = "type Reply =\n completed {\n octets: sequence U8 <= 2\n }\n | refused\nplot reply (\n input: U8 >> reply: Reply\n) = (. == 0 ? refused(empty) : completed({ octets: [., .] }))";
    let p = custom(source, "reply").unwrap();
    let outputs: Vec<_> = [0_u8, 1, 255]
        .into_iter()
        .map(|input| p.evaluate(&[input]).unwrap())
        .collect();
    let (mut prepared, storage) =
        allocation_probe::observe(|| PreparedPortableExpressionEvaluator::new(&p).unwrap());
    assert!(storage.peak_bytes < 64 * 1024, "{storage:?}");
    assert!(prepared.output_capacity() < 4096);
    let (_, play) = allocation_probe::observe(|| {
        for _ in 0..10000 {
            for (index, input) in [0_u8, 1, 255].into_iter().enumerate() {
                assert_eq!(prepared.evaluate(&[input]).unwrap(), outputs[index]);
            }
        }
    });
    assert_eq!((play.allocations, play.reallocations), (0, 0), "{play:?}");
}

#[test]
fn checked_case_and_actual_length_guard_optional_sequence_members_without_allocation() {
    let types = "type Reply =\n completed {\n octets: sequence U8 <= 2\n }\n | refused\n";
    let source = format!("{types}plot first (\n input: Reply >> octet: U8\n) = (variant/is(., \"completed\") ? (sequence/length(.completed.octets) == 1 ? .completed.octets.0 : 0) : 0)");
    let p = custom(&source, "first").unwrap();
    let inputs: Vec<_> = [
        "refused(empty)",
        "completed({ octets: [] })",
        "completed({ octets: [.] })",
        "completed({ octets: [., .] })",
    ]
    .into_iter()
    .map(|body| {
        let source = format!("{types}plot reply (\n input: U8 >> reply: Reply\n) = ({body})");
        let constructor = custom(&source, "reply").unwrap();
        assert_eq!(constructor.output_type, p.input_type);
        constructor.evaluate(&[255]).unwrap()
    })
    .collect();
    let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    for (input, expected) in inputs.iter().zip([0_u8, 0, 255, 0]) {
        assert_eq!(prepared.evaluate(input).unwrap(), [expected]);
        assert_eq!(
            prepared.evaluate(input).unwrap(),
            p.evaluate(input).unwrap()
        );
    }
    let (_, play) = allocation_probe::observe(|| {
        for _ in 0..10000 {
            for (input, expected) in inputs.iter().zip([0_u8, 0, 255, 0]) {
                assert_eq!(prepared.evaluate(input).unwrap(), [expected]);
            }
        }
    });
    assert_eq!((play.allocations, play.reallocations), (0, 0), "{play:?}");
    let source =
        format!("{types}plot absent (\n input: Reply >> octet: U8\n) = (.completed.octets.0)");
    let p = custom(&source, "absent").unwrap();
    let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    assert!(prepared.evaluate(&inputs[0]).is_err());
    assert!(prepared.evaluate(&inputs[1]).is_err());
    assert_eq!(prepared.evaluate(&inputs[2]).unwrap(), [255]);
}

#[test]
fn forwarding_nested_protocol_values_preserves_full_types_and_stays_bounded() {
    let types = "type Payload = {\n octets: sequence U8 <= 3\n}\ntype Envelope = {\n left: Payload\n right: Payload\n}\n";
    let p = custom(&format!("{types}plot forward (\n input: Payload >> output: Envelope\n) = ({{ left: ., right: . }})"),"forward").unwrap();
    let constructor = custom(
        &format!(
            "{types}plot payload (\n input: U8 >> output: Payload\n) = ({{ octets: [.,.,.] }})"
        ),
        "payload",
    )
    .unwrap();
    let input = constructor.evaluate(&[42]).unwrap();
    let expected = p.evaluate(&input).unwrap();
    let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    let (_, play) = allocation_probe::observe(|| {
        for _ in 0..10000 {
            assert_eq!(prepared.evaluate(&input).unwrap(), expected);
        }
    });
    assert_eq!((play.allocations, play.reallocations), (0, 0), "{play:?}");
    let conduit_plot::PortableExpressionOperation::Record(fields) = &p.root.operation else {
        panic!("record")
    };
    let mut forged = p.clone();
    let conduit_plot::PortableExpressionOperation::Record(forged_fields) =
        &mut forged.root.operation
    else {
        panic!("record")
    };
    forged_fields[1] = fields[0].clone();
    assert!(PreparedPortableExpressionEvaluator::new(&forged).is_err());
}
