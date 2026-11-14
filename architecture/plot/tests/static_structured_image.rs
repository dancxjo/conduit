//! Whole emitted protocol expressions: CPU/no_std evidence, not an installed Play.
use conduit_core::*;
use conduit_plot::{rust_binding::*, *};
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[path = "common/compiled_rust.rs"]
mod compiled_rust;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
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
    if matches!(ty.shape(), StructuredInfoTypeShape::Leaf(kind) if kind.as_str() == "value/empty") {
        return if bytes.is_empty() {
            Ok(())
        } else {
            Err(PreparedNativeRefusal::Shape)
        };
    }
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
    let (owned, cost) =
        allocation_probe::observe(|| PreparedPortableExpressionEvaluator::new(p).unwrap());
    eprintln!("{name}: owned_preparation={cost:?}");
    drop(owned);
    let generated = generate_static_structured_expression_with_work(d, p, bounds(), 4096).unwrap();
    let reported_work = generated.computation_work;
    let image = generated.source;
    assert_eq!(
        image,
        generate_static_structured_expression(d, p, bounds(), 4096).unwrap()
    );
    let allocator =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/common/allocation_probe.rs");
    let mut main=format!("#[path={:?}]mod probe;#[global_allocator]static ALLOC:probe::Allocator=probe::Allocator;fn main(){{let(mut e,c)=probe::observe(||image::prepare().unwrap());assert_eq!(c.allocations,0);assert_eq!(c.reallocations,0);assert!(image::RUNTIME_STATE_BYTES<4096);let mut scratch=[0u8;4096];let mut output=[0xa5u8;4096];\n",allocator.to_str().unwrap());
    main.push_str(&format!(
        "assert_eq!(e.computation_work(),{reported_work},\"compiler/runtime work drift\");\n"
    ));
    main.push_str(&format!("println!(\"{name}: resident_state_bytes={{}}, output_stage_bytes={{}}\",image::RUNTIME_STATE_BYTES,e.scratch_bytes());\n"));
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
        if let Some(node) = input.strip_prefix(p.input_type.canonical_bytes().unwrap().as_slice()) {
            let schema = p.input_type.canonical_bytes().unwrap();
            main.push_str(&format!("let saved=output;output.fill(0xa5);let(split,c)=probe::observe(||e.evaluate_parts_into(&{schema:?},&{node:?},&mut output,&mut scratch));assert_eq!(c.allocations,0);assert_eq!(c.reallocations,0);assert_eq!(split,actual);assert_eq!(output,saved);output.fill(0xa5);assert!(e.evaluate_parts_into(&[0],&{node:?},&mut output,&mut scratch).is_err());assert_eq!(output,[0xa5;4096]);output=saved;\n"));
            if let Ok(bytes) = &expected {
                main.push_str(&format!("output.fill(0xa5);assert!(e.evaluate_parts_into(&{schema:?},&{node:?},&mut output[..{}],&mut scratch).is_err());assert_eq!(output,[0xa5;4096]);assert!(e.evaluate_parts_into(&{schema:?},&{node:?},&mut output,&mut []).is_err());assert_eq!(output,[0xa5;4096]);output=saved;\n",bytes.len()-1));
            }
        }
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
    let borrowed =
        generate_borrowed_static_structured_expression_with_work(d, p, bounds(), 4096).unwrap();
    assert_eq!(borrowed.computation_work, reported_work);
    compiled_rust::verify(
        &format!("mod image{{\n{}\n}}", borrowed.source),
        &main,
        &format!("{name}-borrowed"),
    );
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
fn invalid_resident_control_and_unadmitted_capacity_refuse() {
    let d = ht();
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

fn sample(
    ty: &StructuredInfoType,
    address: u8,
    phase: u8,
    tag: &str,
    bytes: &[u8],
    field: &str,
) -> StructuredInfoValue {
    use StructuredInfoTypeShape as S;
    match ty.shape() {
        S::Leaf(_) => leaf(
            ty.clone(),
            match field {
                "address" => address,
                "phase" => phase,
                _ => 0,
            },
        ),
        S::Record { fields, .. } => StructuredInfoValue::record(
            ty.clone(),
            fields
                .iter()
                .map(|f| {
                    StructuredFieldValue::new(
                        f.name(),
                        sample(f.value_type(), address, phase, tag, bytes, f.name()),
                    )
                    .unwrap()
                })
                .collect(),
        )
        .unwrap(),
        S::Variant { cases, .. } => {
            let selected_tag = if tag.starts_with("bus:") && cases.iter().any(|c| c.tag() == "bus")
            {
                "bus"
            } else {
                tag.strip_prefix("bus:").unwrap_or(tag)
            };
            let case = cases
                .iter()
                .find(|c| c.tag() == selected_tag)
                .or_else(|| cases.iter().find(|c| c.tag() == "none"))
                .unwrap_or(&cases[0]);
            let payload = case.payload_type();
            let value = if matches!(payload.shape(), S::Leaf(_)) {
                let bytes = if payload == &StructuredInfoType::leaf(kind_id(EMPTY_INFO_ID)).unwrap()
                {
                    vec![]
                } else {
                    vec![7]
                };
                StructuredInfoValue::leaf(payload.clone(), bytes).unwrap()
            } else {
                sample(payload, address, phase, tag, bytes, field)
            };
            StructuredInfoValue::variant(ty.clone(), case.tag(), value).unwrap()
        }
        S::Sequence { element, .. } => StructuredInfoValue::sequence(
            ty.clone(),
            bytes.iter().map(|n| leaf(element.clone(), *n)).collect(),
        )
        .unwrap(),
        _ => panic!("unused shape"),
    }
}
#[test]
fn remaining_display_state_and_acknowledgement_expressions_preserve_all_bus_outcomes() {
    let d = ht();
    let tags = [
        "completed",
        "short",
        "device-error",
        "not-acknowledged",
        "arbitration-lost",
        "timed-out",
        "provider-lost",
        "unsupported",
        "stale-attachment",
        "refused",
        "bus:completed",
        "bus:short",
        "unsupported-address",
        "invalid-brightness",
    ];
    for name in [
        "ht16k33-action",
        "ht16k33-transition",
        "ht16k33-write-outcome",
    ] {
        let p = program(&d, name);
        let mut values = Vec::new();
        for phase in 0..=7 {
            for tag in tags {
                for bytes in [&[][..], &[1, 2, 3][..]] {
                    values.push(
                        sample(&p.input_type, 112, phase, tag, bytes, "")
                            .canonical_bytes()
                            .unwrap(),
                    );
                }
            }
        }
        values.push(vec![]);
        corpus(&d, &p, values, &format!("structured-{name}-variable"));
    }
}

#[test]
fn real_dht11_decoder_all_outcomes_match_reference_with_integer_widening() {
    let d = checked(include_str!(
        "../../../plots/device-protocols/dht11.conduit"
    ));
    let p = program(&d, "dht11-decode");
    let ty = &p.input_type;
    let wire_ty = member(ty, "wire");
    let StructuredInfoTypeShape::Collection { element, .. } = wire_ty.shape() else {
        panic!("wire");
    };
    let mut values = Vec::new();
    // Valid positive/negative/fractional frames, short/long lengths, checksum
    // and each payload rule. These are Source observations, not a second decoder.
    let frames = [
        (5, [53, 0, 24, 0, 77]),
        (5, [53, 0, 10, 130, 193]),
        (5, [95, 0, 60, 0, 155]),
        (5, [5, 9, 19, 137, 170]),
        (0, [0; 5]),
        (4, [53, 0, 24, 0, 77]),
        (6, [53, 0, 24, 0, 77]),
        (5, [53, 0, 24, 0, 76]),
        (5, [0; 5]),
        (5, [53, 10, 24, 0, 87]),
        (5, [53, 0, 24, 16, 93]),
        (5, [53, 0, 24, 10, 87]),
        (5, [96, 0, 24, 0, 120]),
        (5, [95, 1, 24, 0, 120]),
        (5, [53, 0, 61, 0, 114]),
        (5, [53, 0, 60, 1, 114]),
        (5, [53, 0, 20, 128, 201]),
    ];
    for (length, wire) in frames {
        values.push(record(
            ty,
            vec![
                ("length", leaf(member(ty, "length"), length)),
                (
                    "wire",
                    StructuredInfoValue::collection(
                        wire_ty.clone(),
                        wire.into_iter()
                            .map(|octet| leaf(element.clone(), octet))
                            .collect(),
                    )
                    .unwrap(),
                ),
            ],
        ));
    }
    for (index, input) in values.iter().enumerate() {
        let result = p.evaluate(input).unwrap();
        let value = validate_canonical_structured_value(&result).unwrap();
        if index < 4 {
            assert!(value.variant_payload("frame").unwrap().is_some());
        } else {
            let failure = value.variant_payload("unavailable").unwrap().unwrap();
            let tag = match index {
                4 | 5 => "short-frame",
                6 => "malformed-length",
                7 => "checksum",
                _ => "invalid-payload",
            };
            assert!(
                failure.variant_payload(tag).unwrap().is_some(),
                "case {index}: {tag}"
            );
        }
    }
    values.push(vec![]);
    let mut trailing = values[0].clone();
    trailing.push(0);
    values.push(trailing);
    corpus(&d, &p, values, "structured-dht11-decode-widening");
}

#[test]
fn built_in_empty_constructor_input_preserves_canonical_validation_without_allocation() {
    let d = checked(
        &[
            include_str!("../../../plots/device-protocols/pulse-types.conduit"),
            include_str!("../../../plots/device-protocols/dht11-capture.conduit"),
        ]
        .join("\n"),
    );
    let p = program(&d, "dht11-capture-request");
    let empty = vec![];
    let trailing = vec![0];
    let wrong = StructuredInfoValue::leaf(
        StructuredInfoType::leaf(kind_id("value/u8")).unwrap(),
        vec![0],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    corpus(
        &d,
        &p,
        vec![empty.clone(), trailing, wrong, vec![0xff], vec![], empty],
        "structured-empty-input",
    );
}

#[path = "static_structured_image/parent_correspondence.rs"]
mod parent_correspondence;
#[path = "static_structured_image/work_accounting.rs"]
mod work_accounting;

#[path = "static_structured_image/borrowed_children.rs"]
mod borrowed_children;

#[path = "static_structured_image/descriptor_reuse.rs"]
mod descriptor_reuse;
