use conduit_core::*;
use conduit_plot::*;
fn vector(ty: &StructuredInfoType, number: u16) -> StructuredInfoValue {
    match ty.shape() {
        StructuredInfoTypeShape::Nominal { representation, .. } => {
            StructuredInfoValue::nominal(ty.clone(), vector(representation, number)).unwrap()
        }
        StructuredInfoTypeShape::Collection { element, length } => StructuredInfoValue::collection(
            ty.clone(),
            (0..length).map(|_| vector(element, number)).collect(),
        )
        .unwrap(),
        StructuredInfoTypeShape::Leaf(_) => {
            StructuredInfoValue::leaf(ty.clone(), number.to_le_bytes().to_vec()).unwrap()
        }
        _ => panic!("nominal fixed vector"),
    }
}
#[test]
fn nominal_fixed_collection_projection_preserves_exact_element_and_refuses_foreign_roots() {
    let source="type Positive = U16 in 1..=9\ntype Other = U16 in 1..=8\ntype Values = collection Positive = 3\ntype Foreign = collection Positive = 3\ntype ForeignElements = collection Other = 3\nplot select (\n >> value: Values\n result: Positive >>\n) = (.2)\nplot outside (\n >> value: Values\n result: Positive >>\n) = (.3)\n";
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let ty = |name: &str| {
        &checked
            .native_types
            .iter()
            .find(|t| t.name == name)
            .unwrap()
            .value_type
    };
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "select", &ProfileCatalog::new()).unwrap();
    let ConfigurationValue::Text(hex) = &expanded.expanded.gears[0].configuration[0].value else {
        panic!("program")
    };
    let program = PortableExpressionProgram::from_canonical_hex(hex).unwrap();
    assert_eq!(&program.input_type, ty("Values"));
    assert_eq!(&program.output_type, ty("Positive"));
    let input = vector(ty("Values"), 7).canonical_bytes().unwrap();
    let expected = vector(ty("Positive"), 7).canonical_bytes().unwrap();
    let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    assert_eq!(prepared.evaluate(&input).unwrap(), expected);
    assert_eq!(program.evaluate(&input).unwrap(), expected);
    for bad in [
        vector(ty("Foreign"), 7).canonical_bytes().unwrap(),
        vector(ty("ForeignElements"), 7).canonical_bytes().unwrap(),
        input[..input.len() - 1].to_vec(),
    ] {
        assert!(prepared.evaluate(&bad).is_err());
        assert!(program.evaluate(&bad).is_err());
    }
    assert!(
        expand_canonical_plot_for_authoring(&checked, "outside", &ProfileCatalog::new()).is_err()
    );
}
