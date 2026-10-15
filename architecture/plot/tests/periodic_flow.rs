use conduit_core::{
    ConfigurationValue, StructuredFieldValue, StructuredInfoTypeShape, StructuredInfoValue,
    StructuredInfoValueShape,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog,
};
fn program() -> PortableExpressionProgram {
    let source = format!(
        "{}\n{}",
        include_str!("../../../semantics/audio/dsp/periodic-flow.conduit"),
        r#"
type FlowFixture = {
    input: U64
    current: I32
    previous: I32
}
plot flow-fixture (
    >> value: FlowFixture
    result: FlowFixture >>
) {
    flow: dsp/cubic-flow-pair(item = FlowFixture, output = FlowFixture)
    value >> flow.value
    flow.result >> result
}.
"#
    );
    let document = parse_syntax_document(&source);
    assert_eq!(document.round_trip(), source);
    let checked = check_syntax_document(&document, &StartupCatalog::new()).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "flow-fixture", &ProfileCatalog::new())
            .unwrap();
    assert_eq!(expanded.expanded.gears.len(), 1);
    let ConfigurationValue::Text(bytes) = &expanded.expanded.gears[0].configuration[0].value else {
        panic!("portable program")
    };
    PortableExpressionProgram::from_canonical_hex(bytes).unwrap()
}
fn input(program: &PortableExpressionProgram, current: i32, previous: i32) -> Vec<u8> {
    let StructuredInfoTypeShape::Record { fields, .. } = program.input_type.shape() else {
        panic!("record")
    };
    StructuredInfoValue::record(
        program.input_type.clone(),
        fields
            .iter()
            .map(|field| {
                let bytes = match field.name() {
                    "input" => 0xdecafbadcafebabe_u64.to_le_bytes().to_vec(),
                    "current" => current.to_le_bytes().to_vec(),
                    "previous" => previous.to_le_bytes().to_vec(),
                    _ => panic!("field"),
                };
                StructuredFieldValue::new(
                    field.name(),
                    StructuredInfoValue::leaf(field.value_type().clone(), bytes).unwrap(),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
#[test]
fn cubic_landmarks_and_closed_domain_preserve_opaque_context_in_both_evaluators() {
    // Fixed profile landmarks, including nonzero closure predecessor 255.
    let landmarks = [
        (i32::MIN, 0),
        (-1, 0),
        (0, 0),
        (1, 0),
        (64, 3072),
        (128, 8192),
        (192, 9216),
        (255, 254),
        (256, 0),
        (i32::MAX, 0),
    ];
    let program = program();
    let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    let capacity = prepared.output_capacity();
    for (current, expected_current) in landmarks {
        for (previous, expected_previous) in landmarks {
            let bytes = input(&program, current, previous);
            let output = program.evaluate(&bytes).unwrap();
            assert_eq!(prepared.evaluate(&bytes).unwrap(), output);
            let value = StructuredInfoValue::from_canonical_bytes(&output).unwrap();
            let StructuredInfoValueShape::Record(fields) = value.shape() else {
                panic!("record")
            };
            for field in fields {
                let StructuredInfoValueShape::Leaf(bytes) = field.value().shape() else {
                    panic!("leaf")
                };
                match field.name() {
                    "input" => assert_eq!(bytes, &0xdecafbadcafebabe_u64.to_le_bytes()[..]),
                    "current" => assert_eq!(
                        i32::from_le_bytes(bytes.try_into().unwrap()),
                        expected_current
                    ),
                    "previous" => assert_eq!(
                        i32::from_le_bytes(bytes.try_into().unwrap()),
                        expected_previous
                    ),
                    _ => panic!("field"),
                }
            }
        }
    }
    assert_eq!(prepared.output_capacity(), capacity);
}
