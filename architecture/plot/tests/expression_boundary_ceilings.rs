//! Declared port admission and physical scratch are independent finite facts.
use conduit_core::*;
use conduit_plot::*;

fn identity(declared: Option<u32>) -> (PortableExpressionProgram, Vec<u8>) {
    let byte = StructuredInfoType::leaf(kind_id("value/u8")).unwrap();
    let ty = StructuredInfoType::record(
        kind_id("test/input-ceiling"),
        vec![StructuredFieldType::new("value", byte.clone()).unwrap()],
    )
    .unwrap();
    let value = StructuredInfoValue::record(
        ty.clone(),
        vec![
            StructuredFieldValue::new("value", StructuredInfoValue::leaf(byte, vec![7]).unwrap())
                .unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let program = PortableExpressionProgram {
        input_type: ty.clone(),
        input_maximum_bytes: declared,
        output_type: ty.clone(),
        root: PortableExpressionNode {
            value_type: ty.clone(),
            operation: PortableExpressionOperation::Input,
        },
    };
    (program, value)
}

#[test]
fn admitted_structured_input_ceiling_is_not_replaced_by_smaller_scratch() {
    for declared in [512, 4096] {
        let (program, value) = identity(Some(declared));
        let physical = program.maximum_prepared_input_bytes().unwrap();
        assert!(physical < declared);
        let laws = program.semantic_laws().unwrap();
        let input = laws.iter().find_map(|law| match law {
            KindSemanticLaw::ValueContracts(contracts) => contracts
                .iter()
                .find(|contract| contract.location == FrontValueLocation::Input(port_id("input"))),
            _ => None,
        });
        assert_eq!(input.unwrap().contract.maximum_bytes, declared);
        let mut evaluator = PreparedPortableExpressionEvaluator::new(&program).unwrap();
        assert_eq!(evaluator.evaluate(&value).unwrap(), value);
        let decoded =
            PortableExpressionProgram::from_canonical_bytes(&program.canonical_bytes().unwrap())
                .unwrap();
        assert_eq!(decoded.semantic_laws().unwrap(), laws);
    }
}

#[test]
fn missing_authored_input_limit_does_not_invent_a_scratch_sized_contract() {
    let (program, mut value) = identity(None);
    let physical = program.maximum_prepared_input_bytes().unwrap();
    assert!(physical < 4096);
    let laws = program.semantic_laws().unwrap();
    assert!(!laws.iter().any(|law| matches!(law,
        KindSemanticLaw::ValueContracts(contracts) if contracts.iter().any(|contract|
            contract.location == FrontValueLocation::Input(port_id("input"))))));
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    assert_eq!(evaluator.evaluate(&value).unwrap(), value);
    value.push(0); // Extra bytes still violate the exact Native representation.
    assert!(evaluator.evaluate(&value).is_err());
}

#[test]
fn selector_scratch_does_not_invent_an_input_contract() {
    let (program, wire) = identity(None);
    let selector = StructuredSelector::field(program.input_type, "value").unwrap();
    let laws = structured_selector_semantic_laws(&selector).unwrap();
    let KindSemanticLaw::ValueContracts(contracts) = &laws[0] else {
        panic!("output bound")
    };
    assert_eq!(contracts.len(), 1);
    assert_eq!(
        contracts[0].location,
        FrontValueLocation::Output(port_id("output"))
    );
    let input = StructuredInfoValue::from_canonical_bytes(&wire).unwrap();
    let StructuredSelection::Matched(output) = selector.select(&input).unwrap() else {
        panic!("field")
    };
    assert!(
        output.canonical_bytes().unwrap().len() <= contracts[0].contract.maximum_bytes as usize
    );
    let byte = StructuredInfoType::leaf(kind_id("value/u8")).unwrap();
    let wrong = StructuredInfoValue::leaf(byte, vec![7]).unwrap();
    assert!(selector.select(&wrong).is_err());
}
