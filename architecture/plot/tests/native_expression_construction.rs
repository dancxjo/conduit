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
fn prepared_qualified_variants_match_portable_evaluation() {
    let program = compile(SOURCE).unwrap();
    let mut prepared = conduit_plot::PreparedPortableExpressionEvaluator::new(&program).unwrap();
    for input in [-1_i64, 0, i64::MAX] {
        let bytes = input.to_le_bytes();
        assert_eq!(
            prepared.evaluate(&bytes).unwrap(),
            program.evaluate(&bytes).unwrap()
        );
    }
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

#[test]
fn nominal_text_constants_preserve_identity_and_prove_their_bounds() {
    let source = "type Word = Text <= 4B\nplot choose (\n >> value: Word\n result: Boolean >>\n) = (. == \"one\")\n";
    let program = compile(source).unwrap();
    let conduit_core::StructuredInfoTypeShape::Nominal { representation, .. } =
        program.input_type.shape()
    else {
        panic!("Word")
    };
    let value = StructuredInfoValue::nominal(
        program.input_type.clone(),
        StructuredInfoValue::leaf(representation.clone(), b"one".to_vec()).unwrap(),
    )
    .unwrap();
    assert_eq!(
        program.evaluate(&value.canonical_bytes().unwrap()).unwrap(),
        vec![1]
    );
    assert!(compile(&source.replace("\"one\"", "\"hello\""))
        .unwrap_err()
        .contains("law validator"));
    assert!(compile("type Word = Text <= 4B\ntype Other = Text <= 4B\nplot choose (\n >> value: Other\n result: Word >>\n) = (.)\n").is_err());
}

#[test]
fn variant_record_payload_keeps_its_native_schema_identity() {
    let program = compile("type Payload = {\n value: I64\n}\ntype Choice =\n known Payload\n | missing\nplot choose (\n >> value: I64\n result: Choice >>\n) = (Choice.known({value: .}))\n").unwrap();
    let output =
        StructuredInfoValue::from_canonical_bytes(&program.evaluate(&7_i64.to_le_bytes()).unwrap())
            .unwrap();
    let StructuredInfoValueShape::Variant { payload, .. } = output.shape() else {
        panic!("variant")
    };
    let conduit_core::StructuredInfoTypeShape::Record { schema, .. } = payload.value_type().shape()
    else {
        panic!("payload")
    };
    assert!(schema.as_str().starts_with("type/Payload@"));
}

#[test]
fn refined_record_construction_preserves_exact_forwarded_field_contracts() {
    let source = "type Query = {\n address: U8 in 8..=119\n}\ntype Request = {\n address: U8 in 8..=119\n read: U8 in 0..=32\n}\nplot choose (\n >> query: Query\n result: Request >>\n) = ({address: .address, read: 1})\n";
    assert!(compile(source).is_ok());
    assert!(compile(&source.replace("read: 1", "read: 33"))
        .unwrap_err()
        .contains("law validator"));
    assert!(
        compile(&source.replace("address: .address", "address: .address + 1"))
            .unwrap_err()
            .contains("law validator")
    );
    assert!(compile(&source.replacen("8..=119", "0..=255", 1))
        .unwrap_err()
        .contains("law validator"));
}

#[test]
fn imported_checked_metadata_preserves_refinement_construction_rules() {
    let native = check_syntax_document(
        &parse_syntax_document("type External = {\n read: U8 in 0..=32\n}\n"),
        &StartupCatalog::new(),
    )
    .unwrap();
    let mut catalog = StartupCatalog::new();
    catalog
        .insert_checked_native_type("external/request", &native.native_types[0])
        .unwrap();
    for (value, accepted) in [(32, true), (33, false)] {
        let source = format!("with external/request as Request\nplot choose (\n >> input: U8\n result: Request >>\n) = ({{read: {value}}})\n");
        let checked = check_syntax_document(&parse_syntax_document(&source), &catalog).unwrap();
        assert_eq!(
            expand_canonical_plot_for_authoring(&checked, "choose", &ProfileCatalog::new()).is_ok(),
            accepted
        );
    }
}

#[test]
fn nested_conditional_and_variant_fields_preserve_every_active_refinement() {
    let source = "type Query = {\n address: U8 in 8..=119\n}\ntype Wrapped = {\n payload: Query\n}\nplot choose (\n >> query: Query\n result: Wrapped >>\n) = ({payload: .address == 8 ? . : {address: 8}})\n";
    assert!(compile(source).is_ok());
    assert!(compile(&source.replace("{address: 8}", "{address: 0}"))
        .unwrap_err()
        .contains("law validator"));
    let variant = "type Query = {\n address: U8 in 8..=119\n}\ntype Choice =\n known Query\n | missing\nplot choose (\n >> query: Query\n result: Choice >>\n) = (.address == 8 ? known(.) : missing(unit))\n";
    assert!(compile(variant).is_ok());
    assert!(compile(&variant.replace("known(.)", "known({address: 0})"))
        .unwrap_err()
        .contains("law validator"));
}
