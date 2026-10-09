//! Nominal scalar projections preserve exactly the same encoding as literals.
use conduit_core::{
    ConfigurationValue, StructuredFieldValue, StructuredInfoTypeShape, StructuredInfoValue,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog,
};

#[test]
fn projected_nominal_text_compares_equal_to_an_identical_contextual_literal() {
    let source = "type Symbol = Text <= 64B not in [\"\"]\ntype Definition = {\n symbol: Symbol\n}\nplot compare (\n >> value: Definition\n result: Boolean >>\n) = (.symbol == \"tʰ\")\n";
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "compare", &ProfileCatalog::new()).unwrap();
    let ConfigurationValue::Text(hex) = &expanded.expanded.gears[0].configuration[0].value else {
        panic!("program")
    };
    let program = PortableExpressionProgram::from_canonical_hex(hex).unwrap();
    let StructuredInfoTypeShape::Record { fields, .. } = program.input_type.shape() else {
        panic!("record")
    };
    let field = &fields[0];
    let StructuredInfoTypeShape::Nominal { representation, .. } = field.value_type().shape() else {
        panic!("nominal")
    };
    let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    for (symbol, expected) in [("tʰ", 1), ("t", 0), ("d͡ʒ", 0)] {
        let value = StructuredInfoValue::nominal(
            field.value_type().clone(),
            StructuredInfoValue::leaf(representation.clone(), symbol.as_bytes().to_vec()).unwrap(),
        )
        .unwrap();
        let input = StructuredInfoValue::record(
            program.input_type.clone(),
            vec![StructuredFieldValue::new("symbol", value).unwrap()],
        )
        .unwrap()
        .canonical_bytes()
        .unwrap();
        assert_eq!(program.evaluate(&input).unwrap(), [expected]);
        assert_eq!(prepared.evaluate(&input).unwrap(), [expected]);
    }
}
