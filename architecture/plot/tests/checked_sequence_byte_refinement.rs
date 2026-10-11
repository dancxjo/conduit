//! Existing all-element bounds survive exact selection; no per-index guard leaks.
use conduit_core::{
    ConfigurationValue, StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape as Shape,
    StructuredInfoValue as Value,
};
use conduit_plot::{
    CanonicalExpansionDiagnostic, PortableExpressionProgram, PreparedPortableExpressionEvaluator,
    ProfileCatalog, StartupCatalog, check_syntax_document, expand_canonical_plot_for_authoring,
    parse_syntax_document,
};
const TYPES: &str = "
type Packet = { wire: Bytes <= 256B }
type WidePacket = { wire: Bytes <= 2048B }
type Input = {
 packets: sequence Packet <= 8
 other: sequence WidePacket <= 8
 index: U64
}
type Output = { wire: Bytes <= 256B }
type Narrow = { wire: Bytes <= 128B }
";
fn expand(
    expression: &str,
    output: &str,
) -> Result<PortableExpressionProgram, CanonicalExpansionDiagnostic> {
    let source =
        format!("{TYPES}\nplot select (\n input: Input >> output: {output}\n) = ({expression})\n");
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, "select", &ProfileCatalog::new())?;
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("program")
    };
    Ok(PortableExpressionProgram::from_canonical_hex(encoded).unwrap())
}
fn field<'a>(ty: &'a StructuredInfoType, name: &str) -> &'a StructuredInfoType {
    let Shape::Record { fields, .. } = ty.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value_type()
}
fn input(program: &PortableExpressionProgram, packets: &[&[u8]], index: u64) -> Vec<u8> {
    let sequence = field(&program.input_type, "packets");
    let Shape::Sequence { element, .. } = sequence.shape() else {
        panic!("sequence")
    };
    let packets = packets
        .iter()
        .map(|bytes| {
            Value::record(
                element.clone(),
                vec![
                    StructuredFieldValue::new(
                        "wire",
                        Value::leaf(field(element, "wire").clone(), bytes.to_vec()).unwrap(),
                    )
                    .unwrap(),
                ],
            )
            .unwrap()
        })
        .collect();
    Value::record(
        program.input_type.clone(),
        vec![
            StructuredFieldValue::new(
                "packets",
                Value::sequence(sequence.clone(), packets).unwrap(),
            )
            .unwrap(),
            StructuredFieldValue::new(
                "other",
                Value::sequence(field(&program.input_type, "other").clone(), vec![]).unwrap(),
            )
            .unwrap(),
            StructuredFieldValue::new(
                "index",
                Value::leaf(
                    field(&program.input_type, "index").clone(),
                    index.to_le_bytes().to_vec(),
                )
                .unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
#[test]
fn constant_and_unknown_index_forward_exact_declared_bytes_without_rewriting() {
    for expression in [
        "{wire:sequence/at(.packets,0).wire}",
        "{wire:sequence/at(.packets,.index).wire}",
    ] {
        let program = expand(expression, "Output").unwrap();
        let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
        for count in [1, 8] {
            let bytes: Vec<_> = (0..256).map(|byte| byte as u8).collect();
            let packets = vec![bytes.as_slice(); count];
            let encoded = input(&program, &packets, (count - 1) as u64);
            let output = prepared.evaluate(&encoded).unwrap();
            assert_eq!(output, program.evaluate(&encoded).unwrap());
            let value = Value::from_canonical_bytes(output).unwrap();
            let conduit_core::StructuredInfoValueShape::Record(fields) = value.shape() else {
                panic!("output")
            };
            let conduit_core::StructuredInfoValueShape::Leaf(observed) = fields[0].value().shape()
            else {
                panic!("wire")
            };
            assert_eq!(observed, &bytes);
        }
    }
}
#[test]
fn out_of_range_and_empty_sequence_refuse_without_fabricated_output() {
    let program = expand("{wire:sequence/at(.packets,.index).wire}", "Output").unwrap();
    let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    for (packets, index) in [
        (vec![], 0),
        (vec![b"a".as_slice()], 1),
        (vec![b"a".as_slice()], u64::MAX),
    ] {
        let encoded = input(&program, &packets, index);
        assert!(prepared.evaluate(&encoded).is_err());
        assert!(program.evaluate(&encoded).is_err());
    }
    assert!(prepared.evaluate(&input(&program, &[b"owned"], 0)).is_ok());
}
#[test]
fn wider_unknown_metadata_and_other_index_guard_never_establish_a_bound() {
    for (expression, output) in [
        ("{wire:sequence/at(.other,.index).wire}", "Output"),
        ("{wire:sequence/at(.packets,.index).wire}", "Narrow"),
        (
            "bytes/length(sequence/at(.other,0).wire)<=256 ? {wire:sequence/at(.other,1).wire} : {wire:sequence/at(.packets,0).wire}",
            "Output",
        ),
        (
            "bytes/length(sequence/at(.other,.index).wire)<=256 ? {wire:sequence/at(.other,.index).wire} : {wire:sequence/at(.packets,0).wire}",
            "Output",
        ),
    ] {
        assert_eq!(
            expand(expression, output).unwrap_err().code,
            "CND-FRM-046",
            "{expression}"
        );
    }
}
