use conduit_core::{ConfigurationValue, StructuredInfoValue, StructuredInfoValueShape};
use conduit_plot::*;

fn program(source: &str) -> Result<PortableExpressionProgram, CanonicalExpansionDiagnostic> {
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, "make", &ProfileCatalog::new())?;
    let ConfigurationValue::Text(bytes) = &expanded.expanded.gears[0].configuration[0].value else {
        panic!("expression")
    };
    Ok(PortableExpressionProgram::from_canonical_hex(bytes).unwrap())
}
#[test]
fn nominal_exact_collection_preserves_profile_in_prepared_and_reference_execution() {
    let p=program("type Samples = collection U16 = 2\nplot make (\n >> value: U16\n result: Samples >>\n) = ([1,2])\n").unwrap();
    let expected = p.evaluate(&0u16.to_le_bytes()).unwrap();
    let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    let capacity = prepared.output_capacity();
    assert_eq!(prepared.evaluate(&0u16.to_le_bytes()).unwrap(), expected);
    assert_eq!(prepared.output_capacity(), capacity);
    let actual = StructuredInfoValue::from_canonical_bytes(&expected).unwrap();
    assert_eq!(actual.value_type(), &p.output_type);
    let StructuredInfoValueShape::Collection(items) = actual.shape() else {
        panic!("collection")
    };
    assert_eq!(items.len(), 2);
}
#[test]
fn nominal_collection_refuses_wrong_length_and_invalid_refined_element() {
    assert!(program("type Samples = collection U16 = 2\nplot make (\n >> value: U16\n result: Samples >>\n) = ([1])\n").is_err());
    assert!(program("type Positive = U16 in 1..=3\ntype Samples = collection Positive = 2\nplot make (\n >> value: U16\n result: Samples >>\n) = ([0,2])\n").is_err());
}
