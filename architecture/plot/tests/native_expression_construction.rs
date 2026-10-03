use conduit_core::{ConfigurationValue, StructuredInfoValue, StructuredInfoValueShape};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, ProfileCatalog, StartupCatalog,
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
const SOURCE: &str = "type Choice =\n    known I64\n    | unknown\n\nplot choose (\n    >> value: I64\n    result: Choice >>\n) = (. >= 0 ? Choice.known(.) : Choice.unknown)\n";
#[test]
fn qualified_constructors_preserve_case_payload_and_exact_variant_identity() {
    let program = compile(SOURCE).unwrap();
    for (input, expected) in [(-1_i64, "unknown"), (0, "known"), (i64::MAX, "known")] {
        let output = StructuredInfoValue::from_canonical_bytes(
            &program.evaluate(&input.to_le_bytes()).unwrap(),
        )
        .unwrap();
        assert_eq!(output.value_type(), &program.output_type);
        let StructuredInfoValueShape::Variant { tag, payload } = output.shape() else {
            panic!("choice")
        };
        assert_eq!(tag, expected);
        if tag == "known" {
            let StructuredInfoValueShape::Leaf(bytes) = payload.shape() else {
                panic!("payload")
            };
            assert_eq!(bytes, &input.to_le_bytes());
        }
    }
}
#[test]
fn foreign_cases_missing_cases_and_wrong_payloads_refuse_at_checking() {
    for source in [
        SOURCE.replace("Choice.unknown", "Choice.missing"),
        SOURCE.replace("Choice.known(.)", "Choice.known(true)"),
        format!(
            "type Other =\n    unknown\n\n{}",
            SOURCE.replace("Choice.unknown", "Other.unknown")
        ),
    ] {
        assert!(compile(&source).is_err());
    }
}
#[test]
fn native_record_construction_retains_declared_schema_and_field_types() {
    let program=compile("type Frame = {\n value: I64\n valid: Boolean\n}\nplot choose (\n >> value: I64\n result: Frame >>\n) = ({value: ., valid: . >= 0})\n").unwrap();
    let output =
        StructuredInfoValue::from_canonical_bytes(&program.evaluate(&1_i64.to_le_bytes()).unwrap())
            .unwrap();
    assert_eq!(output.value_type(), &program.output_type);
    assert!(compile("type Frame = {\n value: I64\n}\nplot choose (\n >> value: I64\n result: Frame >>\n) = ({value: true})\n").is_err());
}

#[test]
fn construction_cannot_bypass_native_refinements_or_record_laws() {
    for source in [
        "type Frame = {\n value: I64\n where .value >= 0\n}\nplot choose (\n >> value: I64\n result: Frame >>\n) = ({value: .})\n",
        "type Frame = {\n value: I64 in 0..\n}\nplot choose (\n >> value: I64\n result: Frame >>\n) = ({value: .})\n",
    ] {
        assert!(compile(source).unwrap_err().contains("law validator"));
    }
}

#[test]
fn generic_play_preparation_refuses_unimplemented_variant_construction() {
    let program = compile(SOURCE).unwrap();
    assert!(matches!(
        conduit_plot::PreparedPortableExpressionEvaluator::new(&program),
        Err(conduit_plot::PortableExpressionEvaluationRefusal::UnsupportedType(_))
    ));
}

#[test]
fn imported_shape_without_law_metadata_cannot_authorize_construction() {
    let native = check_syntax_document(
        &parse_syntax_document("type External = {\n value: I64\n where .value >= 0\n}\n"),
        &StartupCatalog::new(),
    )
    .unwrap();
    let mut catalog = StartupCatalog::new();
    catalog
        .insert_structured_type("External", native.native_types[0].value_type.clone())
        .unwrap();
    let checked = check_syntax_document(
        &parse_syntax_document(
            "plot choose (\n >> value: I64\n result: External >>\n) = ({value: .})\n",
        ),
        &catalog,
    )
    .unwrap();
    assert!(
        expand_canonical_plot_for_authoring(&checked, "choose", &ProfileCatalog::new())
            .unwrap_err()
            .message
            .contains("law validator")
    );
}

#[test]
fn refined_constants_keep_existing_arithmetic_proof_and_refuse_invalid_values() {
    let source="type AlmostU32 = U32 where . < 4_294_967_295\nplot choose (\n >> value: AlmostU32\n result: U32 >>\n) = (. + 1)\n";
    assert!(compile(source).is_ok());
    assert!(compile(&source.replace(". + 1", ". + 4_294_967_295"))
        .unwrap_err()
        .contains("law validator"));
}
