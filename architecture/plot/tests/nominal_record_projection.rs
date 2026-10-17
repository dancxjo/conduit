use conduit_core::*;
use conduit_plot::*;
fn id(ty: &StructuredInfoType, value: u16) -> StructuredInfoValue {
    match ty.shape() {
        StructuredInfoTypeShape::Nominal { representation, .. } => {
            StructuredInfoValue::nominal(ty.clone(), id(representation, value)).unwrap()
        }
        StructuredInfoTypeShape::Leaf(_) => {
            StructuredInfoValue::leaf(ty.clone(), value.to_le_bytes().to_vec()).unwrap()
        }
        _ => panic!("nominal integer"),
    }
}
fn record(ty: &StructuredInfoType, value: u16) -> StructuredInfoValue {
    match ty.shape() {
        StructuredInfoTypeShape::Nominal { representation, .. } => {
            StructuredInfoValue::nominal(ty.clone(), record(representation, value)).unwrap()
        }
        StructuredInfoTypeShape::Record { fields, .. } => StructuredInfoValue::record(
            ty.clone(),
            fields
                .iter()
                .map(|f| StructuredFieldValue::new(f.name(), id(f.value_type(), value)).unwrap())
                .collect(),
        )
        .unwrap(),
        _ => panic!("record"),
    }
}
#[test]
fn reference_and_prepared_projection_preserve_refined_nominal_primitive_record_fields() {
    let source="type Positive = U16 in 1..=9\ntype Fact = {\n id: Positive\n}\ntype Anchor = {\n anchor: Positive\n}\nplot projection (\n >> value: Fact\n result: Anchor >>\n) = ({anchor: .id})\n";
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let fact = &checked
        .native_types
        .iter()
        .find(|t| t.name == "Fact")
        .unwrap()
        .value_type;
    let anchor = &checked
        .native_types
        .iter()
        .find(|t| t.name == "Anchor")
        .unwrap()
        .value_type;
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "projection", &ProfileCatalog::new())
            .unwrap();
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("checked program")
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    let input = record(fact, 3).canonical_bytes().unwrap();
    let expected = record(anchor, 3).canonical_bytes().unwrap();
    let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    assert_eq!(prepared.evaluate(&input).unwrap(), expected);
    assert_eq!(program.evaluate(&input).unwrap(), expected);
    let mut invalid = input.clone();
    invalid.push(0);
    assert!(prepared.evaluate(&invalid).is_err());
    assert!(program.evaluate(&invalid).is_err());
}
