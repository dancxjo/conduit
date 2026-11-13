//! Authored output admission survives expression expansion without shrinking scratch.
use conduit_core::*;
use conduit_plot::*;

const SOURCE: &str = "type Wire = {\n wire: Bytes\n}\ntype Envelope = {\n value: Bytes\n}\nplot bounded (\n input: Wire <= 2048B >> output: Envelope <= 256B\n) = ({ value: .wire })\n";

fn program() -> PortableExpressionProgram {
    program_from_source(SOURCE)
}

fn program_from_source(source: &str) -> PortableExpressionProgram {
    let syntax = parse_syntax_document(source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "bounded", &ProfileCatalog::new()).unwrap();
    assert_eq!(expanded.expanded.gears.len(), 1);
    let entry = expanded.expanded.gears[0]
        .configuration
        .iter()
        .find(|entry| entry.key == "program")
        .unwrap();
    let ConfigurationValue::Text(encoded) = &entry.value else {
        panic!("canonical program")
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}

fn wire(ty: &StructuredInfoType, count: usize) -> Vec<u8> {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("Wire record")
    };
    StructuredInfoValue::record(
        ty.clone(),
        vec![StructuredFieldValue::new(
            "wire",
            StructuredInfoValue::leaf(fields[0].value_type().clone(), vec![7; count]).unwrap(),
        )
        .unwrap()],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}

#[test]
fn authored_output_ceiling_survives_program_roundtrip_without_shrinking_scratch() {
    let program = program();
    assert_eq!(program.input_maximum_bytes, Some(2048));
    assert!(program.maximum_prepared_output_bytes().unwrap() > 256);
    let laws = program.semantic_laws().unwrap();
    let output = laws
        .iter()
        .find_map(|law| match law {
            KindSemanticLaw::ValueContracts(contracts) => contracts.iter().find(|contract| {
                contract.location == FrontValueLocation::Output(port_id("output"))
            }),
            _ => None,
        })
        .unwrap();
    assert_eq!(output.contract.maximum_bytes, 256);
    assert_eq!(
        PortableExpressionProgram::from_canonical_bytes(&program.canonical_bytes().unwrap())
            .unwrap(),
        program
    );
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    assert!(evaluator.output_capacity() > 256);
    let output = evaluator.evaluate(&wire(&program.input_type, 4)).unwrap();
    assert!(output.len() <= 256);
    assert_eq!(
        StructuredInfoValue::from_canonical_bytes(output)
            .unwrap()
            .value_type(),
        &program.output_type
    );
}

#[test]
fn output_guard_refuses_oversized_typed_result_and_preserves_input_native_type_check() {
    let program = program();
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    let oversized = wire(&program.input_type, 512);
    assert!(oversized.len() <= 2048);
    assert!(matches!(
        evaluator.evaluate(&oversized),
        Err(PortableExpressionEvaluationRefusal::OutputValueConstraint(
            _
        ))
    ));
    let StructuredInfoTypeShape::Record { fields, .. } = program.input_type.shape() else {
        panic!("Wire record")
    };
    let foreign =
        StructuredInfoType::record(kind_id("test/foreign-wire"), fields.to_vec()).unwrap();
    assert!(evaluator.evaluate(&wire(&foreign, 4)).is_err());
}

#[test]
fn implicit_native_record_output_does_not_invent_a_boundary_guard() {
    let program =
        program_from_source(&SOURCE.replace("output: Envelope <= 256B", "output: Envelope"));
    assert!(!matches!(
        program.root.operation,
        PortableExpressionOperation::ValueContract { .. }
    ));
    assert!(program.maximum_prepared_output_bytes().unwrap() > 256);
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    assert!(
        evaluator
            .evaluate(&wire(&program.input_type, 512))
            .unwrap()
            .len()
            > 256
    );
}
