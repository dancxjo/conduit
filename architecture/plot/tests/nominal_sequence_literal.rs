//! Named sequence literals retain identity and bounds in both evaluators.
use conduit_core::{ConfigurationValue, StructuredInfoValue};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog,
};

fn compile(body: &str) -> Result<PortableExpressionProgram, String> {
    let source = format!("type Labels = sequence Text <= 2\nplot labels (\n >> value: Boolean\n result: Labels >>\n) = ({body})\n");
    let checked = check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new())
        .map_err(|e| format!("{e:?}"))?;
    let expanded = expand_canonical_plot_for_authoring(&checked, "labels", &ProfileCatalog::new())
        .map_err(|e| format!("{e:?}"))?;
    let ConfigurationValue::Text(hex) = &expanded.expanded.gears[0].configuration[0].value else {
        panic!("program")
    };
    PortableExpressionProgram::from_canonical_hex(hex).map_err(|e| format!("{e:?}"))
}

#[test]
fn named_empty_and_unicode_sequences_have_exact_prepared_parity() {
    for body in ["[]", "[\"alpha\", \"β\"]"] {
        let program = compile(body).unwrap();
        let encoded = program.evaluate(&[1]).unwrap();
        let value = StructuredInfoValue::from_canonical_bytes(&encoded).unwrap();
        assert_eq!(value.value_type(), &program.output_type);
        let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
        assert_eq!(prepared.evaluate(&[1]).unwrap(), encoded);
    }
}

#[test]
fn named_sequence_keeps_its_cardinality_and_element_contracts() {
    assert!(compile("[\"a\", \"b\", \"c\"]").is_err());
    assert!(compile("[true]").is_err());
}
