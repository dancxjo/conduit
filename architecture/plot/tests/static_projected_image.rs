//! Emitted-code CPU proof, not firmware placement, ordinary Make or a Play.
use conduit_core::*;
use conduit_plot::{rust_binding::*, *};
#[path = "common/compiled_rust.rs"]
mod compiled_rust;
fn bounds() -> PreparedNativeBounds {
    PreparedNativeBounds {
        canonical_bytes: 4096,
        scopes: 64,
        invariants: 64,
        contracts: 64,
    }
}
fn checked(source: &str) -> CheckedSyntaxDocument {
    check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap()
}
fn program(document: &CheckedSyntaxDocument, name: &str) -> PortableExpressionProgram {
    let expanded = expand_canonical_plot_for_authoring(document, name, &ProfileCatalog::new())
        .unwrap()
        .expanded;
    let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
        panic!("program")
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
fn identity(ty: &StructuredInfoType) -> &KindId {
    match ty.shape() {
        StructuredInfoTypeShape::Record { schema, .. }
        | StructuredInfoTypeShape::Variant { schema, .. }
        | StructuredInfoTypeShape::Nominal { schema, .. } => schema,
        _ => panic!("Native"),
    }
}
fn expected(
    document: &CheckedSyntaxDocument,
    p: &PortableExpressionProgram,
    input: &[u8],
) -> Result<Vec<u8>, StaticProjectedRefusal> {
    let mut validator =
        PreparedNativeAdmission::new(document, identity(&p.input_type), bounds()).unwrap();
    validator
        .begin(input)
        .map_err(StaticProjectedRefusal::Native)?;
    while validator.step().map_err(StaticProjectedRefusal::Native)?
        != PreparedNativeProgress::Admitted
    {}
    p.evaluate(input)
        .map_err(StaticProjectedRefusal::Expression)
}
fn corpus(
    document: &CheckedSyntaxDocument,
    p: &PortableExpressionProgram,
    values: Vec<Vec<u8>>,
    name: &str,
) {
    let image = generate_static_projected_expression(document, p, bounds(), 4096).unwrap();
    assert_eq!(
        image,
        generate_static_projected_expression(document, p, bounds(), 4096).unwrap()
    );
    let allocator =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/common/allocation_probe.rs");
    let mut main = format!("#[path={:?}] mod probe;\n#[global_allocator] static ALLOC: probe::Allocator=probe::Allocator;\nfn main() {{ let (e,cost)=probe::observe(||image::prepare().unwrap());assert_eq!(cost.allocations,0);assert_eq!(cost.reallocations,0);assert!(image::RUNTIME_STATE_BYTES<1024); assert!(e.scratch_bytes()<8192);\n",allocator.to_str().unwrap());
    for value in values {
        let reference = expected(document, p, &value);
        main.push_str(&format!("let mut output=[0xa5;16];let (actual,cost)=probe::observe(||e.evaluate_into(&{value:?},&mut output));assert_eq!(cost.allocations,0);assert_eq!(cost.reallocations,0);\n"));
        match reference {
            Ok(bytes) => main.push_str(&format!("assert_eq!(actual,Ok({}));assert_eq!(&output[..{}],&{bytes:?});assert!(output[{}..].iter().all(|n|*n==0xa5));\n",bytes.len(),bytes.len(),bytes.len())),
            Err(error) => main.push_str(&format!("assert_eq!(format!(\"{{:?}}\",actual),{:?});assert_eq!(output,[0xa5;16]);\n",format!("Err({error:?})"))),
        }
    }
    main.push_str("}\n");
    compiled_rust::verify(&format!("mod image {{\n{image}\n}}"), &main, name);
}
fn field(ty: &StructuredInfoType, name: &str) -> StructuredInfoType {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|f| f.name() == name)
        .unwrap()
        .value_type()
        .clone()
}
fn record(ty: &StructuredInfoType, values: Vec<(&str, StructuredInfoValue)>) -> Vec<u8> {
    StructuredInfoValue::record(
        ty.clone(),
        values
            .into_iter()
            .map(|(n, v)| StructuredFieldValue::new(n, v).unwrap())
            .collect(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
fn leaf(ty: StructuredInfoType, n: u8) -> StructuredInfoValue {
    StructuredInfoValue::leaf(ty, vec![n]).unwrap()
}
#[test]
fn real_display_frame_condition_uses_checked_native_input_without_owned_runtime_metadata() {
    let types = checked(include_str!(
        "../../../plots/device-protocols/i2c-types.conduit"
    ));
    let mut catalog = StartupCatalog::new();
    for (name, path) in [
        ("I2cTransaction", "machine/i2c/transact/request"),
        ("I2cResult", "machine/i2c/transact/result"),
    ] {
        let native = types.native_types.iter().find(|t| t.name == name).unwrap();
        catalog
            .insert_checked_native_type_from_document(path, &types, &native.identity)
            .unwrap();
    }
    let document = check_syntax_document(
        &parse_syntax_document(include_str!(
            "../../../plots/device-protocols/ht16k33.conduit"
        )),
        &catalog,
    )
    .unwrap();
    let mut p = program(&document, "ht16k33-frame-request");
    let mut root = &p.root;
    while let PortableExpressionOperation::ValueContract { value, .. } = &root.operation {
        root = value;
    }
    let PortableExpressionOperation::Conditional { condition, .. } = &root.operation else {
        panic!("conditional")
    };
    p.root = condition.as_ref().clone();
    p.output_type = p.root.value_type.clone();
    let ty = &p.input_type;
    let ram = field(ty, "ram");
    let StructuredInfoTypeShape::Collection { element, .. } = ram.shape() else {
        panic!("collection")
    };
    let mut values = Vec::new();
    for address in [7, 8, 111, 112, 119, 120] {
        values.push(record(
            ty,
            vec![
                ("address", leaf(field(ty, "address"), address)),
                (
                    "ram",
                    StructuredInfoValue::collection(
                        ram.clone(),
                        (0..16).map(|n| leaf(element.clone(), n)).collect(),
                    )
                    .unwrap(),
                ),
            ],
        ));
    }
    values.push(vec![]);
    let mut trailing = values[1].clone();
    trailing.push(0);
    values.push(trailing);
    corpus(&document, &p, values, "projected-ht16k33-frame-condition");
}
#[test]
fn multiple_fields_checked_arithmetic_and_lazy_branches_share_the_scalar_loop() {
    let document=checked("type Operands = {\n enabled: Boolean\n left: U8\n right: U8\n}\nplot calculate (\n value: Operands...| >> result: U8...|\n) = (.enabled ? .left / .right : 0)\n");
    let p = program(&document, "calculate");
    let ty = &p.input_type;
    let values = [(0, 7, 0), (1, 7, 2), (1, 7, 0), (1, 255, 1)]
        .into_iter()
        .map(|(enabled, left, right)| {
            record(
                ty,
                vec![
                    ("enabled", leaf(field(ty, "enabled"), enabled)),
                    ("left", leaf(field(ty, "left"), left)),
                    ("right", leaf(field(ty, "right"), right)),
                ],
            )
        })
        .collect();
    corpus(&document, &p, values, "projected-arithmetic");
    let mut guarded = p.clone();
    guarded.root = PortableExpressionNode {
        value_type: p.output_type.clone(),
        operation: PortableExpressionOperation::ValueContract {
            input: None,
            output: Some(
                CheckedValueContract::new(
                    kind_id("value/u8"),
                    1,
                    vec![ValueConstraint::FixedIntegerRange {
                        minimum: Some(vec![0]),
                        maximum: Some(vec![3]),
                        minimum_endpoint: IntervalEndpoint::Inclusive,
                        maximum_endpoint: IntervalEndpoint::Inclusive,
                    }],
                )
                .unwrap(),
            ),
            value: Box::new(p.root.clone()),
        },
    };
    let values = [(0, 7, 0), (1, 7, 2), (1, 255, 1)]
        .into_iter()
        .map(|(enabled, left, right)| {
            record(
                ty,
                vec![
                    ("enabled", leaf(field(ty, "enabled"), enabled)),
                    ("left", leaf(field(ty, "left"), left)),
                    ("right", leaf(field(ty, "right"), right)),
                ],
            )
        })
        .collect();
    corpus(&document, &guarded, values, "projected-output-predicate");
}
#[test]
fn missing_owner_and_unretained_laws_refuse_emission() {
    let document=checked("type Operands = {\n left: U8\n right: U8\n}\nplot calculate (\n value: Operands...| >> result: U8...|\n) = .left + .right\n");
    let p = program(&document, "calculate");
    assert!(generate_static_projected_expression(
        &checked("type Unrelated = U8\n"),
        &p,
        bounds(),
        4096
    )
    .is_err());
    let mut short = bounds();
    short.canonical_bytes = 1;
    assert!(generate_static_projected_expression(&document, &p, short, 4096).is_err());
    let law = checked("type Address = U8 in 1..=7\n")
        .native_types
        .remove(0)
        .value_type;
    let mut forged = p.clone();
    let PortableExpressionOperation::Binary { left, .. } = &mut forged.root.operation else {
        panic!("binary")
    };
    **left = PortableExpressionNode {
        value_type: law.clone(),
        operation: PortableExpressionOperation::Constant(
            StructuredInfoValue::nominal(
                law,
                leaf(StructuredInfoType::leaf(kind_id("value/u8")).unwrap(), 0),
            )
            .unwrap(),
        ),
    };
    assert!(
        matches!(generate_static_projected_expression(&document,&forged,bounds(),4096),Err(StaticProjectedGenerationRefusal::Expression(PortableExpressionEvaluationRefusal::UnsupportedType(reason))) if reason == "projected-image/intermediate-law-witness-unavailable@1")
    );
    let document=checked("type Operands = {\n left: U8\n right: U8\n where .left < .right\n}\nplot calculate (\n value: Operands...| >> result: U8...|\n) = .left + .right\n");
    assert!(matches!(
        generate_static_projected_expression(
            &document,
            &program(&document, "calculate"),
            bounds(),
            4096
        ),
        Err(StaticProjectedGenerationRefusal::Native(
            ResidentNativeGenerationRefusal::UnsupportedInvariant { .. }
        ))
    ));
}

#[test]
fn field_after_variable_sequence_uses_a_retained_path_without_runtime_allocation() {
    let d=checked("type Operands = {\n a: sequence U8 <= 8\n right: U8\n}\nplot calculate (\n value: Operands...| >> result: U8...|\n) = .right\n");
    let p = program(&d, "calculate");
    let ty = &p.input_type;
    let a = field(ty, "a");
    let StructuredInfoTypeShape::Sequence { element, .. } = a.shape() else {
        panic!("sequence")
    };
    let values = [0, 1, 8, 0]
        .into_iter()
        .map(|n| {
            record(
                ty,
                vec![
                    (
                        "a",
                        StructuredInfoValue::sequence(
                            a.clone(),
                            (0..n).map(|i| leaf(element.clone(), i)).collect(),
                        )
                        .unwrap(),
                    ),
                    ("right", leaf(field(ty, "right"), n + 7)),
                ],
            )
        })
        .collect();
    corpus(&d, &p, values, "projected-variable-field");
}

#[test]
fn bounded_sequence_index_preserves_missing_element_refusal_and_reuse() {
    let d=checked("type Samples = {\n values: sequence U8 <= 8\n}\nplot first (\n input: Samples...| >> output: U8...|\n) = .values.0\n");
    let p = program(&d, "first");
    let ty = &p.input_type;
    let values_type = field(ty, "values");
    let StructuredInfoTypeShape::Sequence { element, .. } = values_type.shape() else {
        panic!("sequence")
    };
    let values = [1, 0, 8, 1]
        .into_iter()
        .map(|n| {
            record(
                ty,
                vec![(
                    "values",
                    StructuredInfoValue::sequence(
                        values_type.clone(),
                        (0..n).map(|i| leaf(element.clone(), i + 5)).collect(),
                    )
                    .unwrap(),
                )],
            )
        })
        .collect();
    corpus(&d, &p, values, "projected-sequence-index");
}
