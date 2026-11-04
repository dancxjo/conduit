//! Whole emitted protocol expressions: CPU/no_std evidence, not an installed Play.
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
fn ht() -> CheckedSyntaxDocument {
    let types = checked(include_str!(
        "../../../plots/device-protocols/i2c-types.conduit"
    ));
    let mut catalog = StartupCatalog::new();
    for (name, path) in [
        ("I2cTransaction", "machine/i2c/transact/request"),
        ("I2cResult", "machine/i2c/transact/result"),
    ] {
        catalog
            .insert_checked_native_type_from_document(
                path,
                &types,
                &types
                    .native_types
                    .iter()
                    .find(|n| n.name == name)
                    .unwrap()
                    .identity,
            )
            .unwrap();
    }
    check_syntax_document(
        &parse_syntax_document(include_str!(
            "../../../plots/device-protocols/ht16k33.conduit"
        )),
        &catalog,
    )
    .unwrap()
}
fn program(d: &CheckedSyntaxDocument, name: &str) -> PortableExpressionProgram {
    let expanded = expand_canonical_plot_for_authoring(d, name, &ProfileCatalog::new())
        .unwrap()
        .expanded;
    let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
        panic!("program")
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
fn admit(
    d: &CheckedSyntaxDocument,
    ty: &StructuredInfoType,
    bytes: &[u8],
) -> Result<(), PreparedNativeRefusal> {
    let id = match ty.shape() {
        StructuredInfoTypeShape::Record { schema, .. }
        | StructuredInfoTypeShape::Variant { schema, .. }
        | StructuredInfoTypeShape::Nominal { schema, .. } => schema,
        _ => panic!("Native"),
    };
    let mut a = PreparedNativeAdmission::new(d, id, bounds())?;
    a.begin(bytes)?;
    while a.step()? != PreparedNativeProgress::Admitted {}
    Ok(())
}
fn corpus(
    d: &CheckedSyntaxDocument,
    p: &PortableExpressionProgram,
    values: Vec<Vec<u8>>,
    name: &str,
) {
    let image = generate_static_structured_expression(d, p, bounds(), 4096).unwrap();
    assert_eq!(
        image,
        generate_static_structured_expression(d, p, bounds(), 4096).unwrap()
    );
    let allocator =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/common/allocation_probe.rs");
    let mut main=format!("#[path={:?}]mod probe;#[global_allocator]static ALLOC:probe::Allocator=probe::Allocator;fn main(){{let(mut e,c)=probe::observe(||image::prepare().unwrap());assert_eq!(c.allocations,0);assert_eq!(c.reallocations,0);assert!(image::RUNTIME_STATE_BYTES<4096);let mut scratch=[0u8;4096];let mut output=[0xa5u8;4096];\n",allocator.to_str().unwrap());
    for input in values {
        let expected = admit(d, &p.input_type, &input)
            .map_err(StaticStructuredRefusal::Input)
            .and_then(|()| {
                p.evaluate(&input)
                    .map_err(StaticStructuredRefusal::Expression)
            })
            .and_then(|v| {
                admit(d, &p.output_type, &v).map_err(StaticStructuredRefusal::Output)?;
                Ok(v)
            });
        main.push_str(&format!("output.fill(0xa5);let(actual,c)=probe::observe(||e.evaluate_into(&{input:?},&mut output,&mut scratch));assert_eq!(c.allocations,0);assert_eq!(c.reallocations,0);\n"));
        match expected {
            Ok(bytes) => {
                main.push_str(&format!("assert_eq!(actual,Ok({}));assert_eq!(&output[..{}],&{bytes:?});assert!(output[{}..].iter().all(|n|*n==0xa5));\n",bytes.len(),bytes.len(),bytes.len()));
                main.push_str(&format!("output.fill(0xa5);assert!(e.evaluate_into(&{input:?},&mut output[..{}],&mut scratch).is_err());assert_eq!(output,[0xa5;4096]);assert!(e.evaluate_into(&{input:?},&mut output,&mut []).is_err());assert_eq!(output,[0xa5;4096]);\n",bytes.len()-1));
            }
            Err(e) => main.push_str(&format!(
                "assert_eq!(format!(\"{{:?}}\",actual),{:?});assert_eq!(output,[0xa5;4096]);\n",
                format!("Err({e:?})")
            )),
        }
    }
    main.push_str("}\n");
    compiled_rust::verify(&format!("mod image{{\n{image}\n}}"), &main, name);
}
fn member(ty: &StructuredInfoType, name: &str) -> StructuredInfoType {
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
fn leaf(ty: StructuredInfoType, n: u8) -> StructuredInfoValue {
    StructuredInfoValue::leaf(ty, vec![n]).unwrap()
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
#[test]
fn complete_display_frame_request_matches_native_admission_and_reference_without_runtime_allocation(
) {
    let d = ht();
    let p = program(&d, "ht16k33-frame-request");
    let ty = &p.input_type;
    let ram = member(ty, "ram");
    let StructuredInfoTypeShape::Collection { element, .. } = ram.shape() else {
        panic!("collection")
    };
    let mut values = Vec::new();
    for address in [7, 8, 111, 112, 119, 120] {
        values.push(record(
            ty,
            vec![
                ("address", leaf(member(ty, "address"), address)),
                (
                    "ram",
                    StructuredInfoValue::collection(
                        ram.clone(),
                        (0..16).map(|n| leaf(element.clone(), n * 15)).collect(),
                    )
                    .unwrap(),
                ),
            ],
        ));
    }
    values.push(vec![]);
    let mut trailing = values[3].clone();
    trailing.push(0);
    values.push(trailing);
    corpus(&d, &p, values, "structured-ht16k33-frame-request");
}
#[test]
fn complete_display_initialization_preserves_all_refusal_branches_and_input_copy() {
    let d = ht();
    let p = program(&d, "ht16k33-initialize");
    let ty = &p.input_type;
    let values = [
        (112, 0, 0, 0),
        (119, 15, 3, 1),
        (111, 7, 0, 1),
        (112, 16, 0, 1),
        (112, 7, 4, 1),
    ]
    .into_iter()
    .map(|(address, brightness, blink, enabled)| {
        record(
            ty,
            vec![
                ("address", leaf(member(ty, "address"), address)),
                ("brightness", leaf(member(ty, "brightness"), brightness)),
                ("blink", leaf(member(ty, "blink"), blink)),
                ("enabled", leaf(member(ty, "enabled"), enabled)),
            ],
        )
    })
    .collect();
    corpus(&d, &p, values, "structured-ht16k33-initialize");
}
#[test]
fn constructed_output_native_predicates_are_retained_and_failure_does_not_publish() {
    let d=checked("type Input = {\n value: U8\n}\ntype Output = {\n value: U8\n}\ntype RefinedOutput = {\n value: U8 in 1..=7\n}\nplot copy (\n input: Input...| >> output: Output...|\n) = {value: .value}\n");
    let mut p = program(&d, "copy");
    // Deliberate portable-program component fixture: ordinary Source expansion
    // refuses refined construction without an installed validator. This emitter
    // must retain that validator; it does not bypass ordinary authoring admission.
    p.output_type = d
        .native_types
        .iter()
        .find(|n| n.name == "RefinedOutput")
        .unwrap()
        .value_type
        .clone();
    p.root.value_type = p.output_type.clone();
    let ty = &p.input_type;
    let values = [0, 1, 7, 8, 255, 1]
        .into_iter()
        .map(|n| record(ty, vec![("value", leaf(member(ty, "value"), n))]))
        .collect();
    corpus(&d, &p, values, "structured-output-native-predicate");
}

#[test]
fn unsupported_variable_geometry_and_invalid_resident_control_refuse() {
    let d = ht();
    assert!(matches!(
        generate_static_structured_expression(&d, &program(&d, "ht16k33-action"), bounds(), 4096),
        Err(StaticStructuredGenerationRefusal::Child(_))
    ));
    let p = program(&d, "ht16k33-frame-request");
    let mut short = bounds();
    short.canonical_bytes = 1;
    assert!(generate_static_structured_expression(&d, &p, short, 4096).is_err());
    let mut image = generate_static_structured_expression(&d, &p, bounds(), 4096).unwrap();
    image.push_str(
        "pub fn check_bad() { static BAD: &[conduit_plot::StaticStructuredProgram] = &[\n",
    );
    for code in [
        "Jump(0)",
        "Jump(2)",
        "Bytes{start:0,length:1}",
        "InputNode{start:u32::MAX,length:2}",
        "Value(0)",
        "BranchFalse{value:0,target:1}",
    ] {
        image.push_str(&format!("conduit_plot::StaticStructuredProgram{{instructions:&[conduit_plot::StructuredTapeInstruction::{code}],constants:&[],maximum_input_bytes:4096,maximum_output_bytes:4096}},\n"));
    }
    image.push_str("]; for p in BAD { let result=conduit_plot::StaticStructuredExpression::<{input_native::CONTRACT_COUNT},{output_native::CONTRACT_COUNT},0>::new(input_native::prepare().unwrap(),output_native::prepare().unwrap(),[],p);assert!(result.is_err()); }}\n");
    compiled_rust::verify(
        &format!("mod image{{\n{image}\n}}"),
        "fn main(){image::check_bad();}",
        "structured-invalid-control",
    );
}
