use conduit_core::{ConfigurationValue, StructuredInfoValue};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog,
};
fn compile(source: &str) -> Result<PortableExpressionProgram, String> {
    let syntax = parse_syntax_document(source);
    if !syntax.diagnostics.is_empty() {
        return Err(format!("{:?}", syntax.diagnostics));
    }
    let checked = check_syntax_document(&syntax, &StartupCatalog::new())
        .map_err(|error| format!("{error:?}"))?;
    let expanded = expand_canonical_plot_for_authoring(&checked, "choose", &ProfileCatalog::new())
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(expanded.expanded.gears.len(), 1);
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("program")
    };
    PortableExpressionProgram::from_canonical_hex(encoded).map_err(|error| format!("{error:?}"))
}
// Explicit material observation must not retag a value into another nominal contract.
#[test]
fn text_material_preserves_utf8_and_empty_material_in_both_evaluators() {
    let p = compile("type Word = Text <= 32B\nplot choose (\n >> value: Word\n result: Text >>\n) = text/material(.)\n").unwrap();
    let conduit_core::StructuredInfoTypeShape::Nominal { representation, .. } =
        p.input_type.shape()
    else {
        panic!("nominal input");
    };
    let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    for text in ["", "vocative", "Olá"] {
        let value = StructuredInfoValue::nominal(
            p.input_type.clone(),
            StructuredInfoValue::leaf(representation.clone(), text.as_bytes().to_vec()).unwrap(),
        )
        .unwrap();
        let encoded = value.canonical_bytes().unwrap();
        assert_eq!(p.evaluate(&encoded).unwrap(), text.as_bytes());
        assert_eq!(prepared.evaluate(&encoded).unwrap(), text.as_bytes());
    }
}
#[test]
fn text_material_refuses_nontext_and_does_not_admit_a_foreign_nominal_output() {
    for input in ["Bytes", "U64", "Boolean"] {
        assert!(compile(&format!(
            "plot choose (\n >> value: {input}\n result: Text >>\n) = text/material(.)\n"
        ))
        .is_err());
    }
    assert!(compile("type Word = Text <= 32B\ntype Other = Text <= 32B\nplot choose (\n >> value: Word\n result: Other >>\n) = text/material(.)\n").is_err());
}

#[test]
fn material_equality_compares_full_bytes_across_distinct_native_contracts() {
    use conduit_core::{StructuredFieldValue, StructuredInfoTypeShape};
    let p = compile("type ParserSubtype = Text <= 32B\ntype PortableSubtype = Text <= 32B not in [\"\"]\ntype Pair = {\n parser: ParserSubtype\n portable: PortableSubtype\n}\nplot choose (\n >> value: Pair\n result: Boolean >>\n) = text/material(.parser) == text/material(.portable)\n").unwrap();
    let StructuredInfoTypeShape::Record { fields, .. } = p.input_type.shape() else {
        panic!("pair");
    };
    let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    for (parser, portable, expected) in [
        ("voc", "voc", 1),
        ("voc", "vocative", 0),
        ("", "voc", 0),
        ("Olá", "Ola", 0),
    ] {
        let values = fields
            .iter()
            .zip([parser, portable])
            .map(|(field, text)| {
                let StructuredInfoTypeShape::Nominal { representation, .. } =
                    field.value_type().shape()
                else {
                    panic!("text nominal");
                };
                StructuredFieldValue::new(
                    field.name(),
                    StructuredInfoValue::nominal(
                        field.value_type().clone(),
                        StructuredInfoValue::leaf(representation.clone(), text.as_bytes().to_vec())
                            .unwrap(),
                    )
                    .unwrap(),
                )
                .unwrap()
            })
            .collect();
        let encoded = StructuredInfoValue::record(p.input_type.clone(), values)
            .unwrap()
            .canonical_bytes()
            .unwrap();
        assert_eq!(p.evaluate(&encoded).unwrap(), [expected]);
        assert_eq!(prepared.evaluate(&encoded).unwrap(), [expected]);
    }
}
