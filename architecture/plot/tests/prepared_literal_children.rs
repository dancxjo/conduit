//! Literal children keep Source bytes without retaining input evaluators.
#[path = "prepared_structured_payload/allocation.rs"]
mod allocation;
use conduit_plot::*;

fn program() -> PortableExpressionProgram {
    let source = "type Frame = {\n value: U8\n padding: collection U8 = 16\n}\n\
        plot envelope (\n value: U8 >> result: Frame\n) = ({value: ., padding: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]})";
    compile(source, "envelope")
}

fn compile(source: &str, entry: &str) -> PortableExpressionProgram {
    let syntax = parse_syntax_document(source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, entry, &ProfileCatalog::new()).unwrap();
    let conduit_core::ConfigurationValue::Text(encoded) =
        &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("expression");
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}

#[test]
fn packet_source_keeps_dynamic_branches_and_exact_reference_bytes() {
    use conduit_core::{
        StructuredFieldValue as Field, StructuredInfoTypeShape as Shape,
        StructuredInfoValue as Value,
    };
    let program = compile(
        include_str!("../../../plots/device-protocols/tm1637.conduit"),
        "tm1637-packets",
    );
    let (mut prepared, retained) = allocation::rounded_retained_bytes(|| {
        PreparedPortableExpressionEvaluator::new(&program).unwrap()
    });
    eprintln!("packet-source rounded retained bytes: {retained}");
    if cfg!(target_pointer_width = "64") {
        assert!(
            retained <= 20 * 1024,
            "packet preparation storage regressed"
        );
    }
    let Shape::Record { fields, .. } = program.input_type.shape() else {
        panic!("Frame");
    };
    for value in 0..=255u8 {
        for enabled in [false, true] {
            let values = fields
                .iter()
                .map(|field| {
                    let ty = field.value_type();
                    let content = if field.name() == "registers" {
                        let Shape::Collection { element, length: 6 } = ty.shape() else {
                            panic!("registers");
                        };
                        Value::collection(
                            ty.clone(),
                            (0..6)
                                .map(|shift| {
                                    Value::leaf(element.clone(), vec![value.rotate_left(shift)])
                                        .unwrap()
                                })
                                .collect(),
                        )
                        .unwrap()
                    } else {
                        Value::leaf(
                            ty.clone(),
                            vec![if field.name() == "enabled" {
                                u8::from(enabled)
                            } else {
                                value
                            }],
                        )
                        .unwrap()
                    };
                    Field::new(field.name(), content).unwrap()
                })
                .collect();
            let frame = Value::record(program.input_type.clone(), values)
                .unwrap()
                .canonical_bytes()
                .unwrap();
            let expected = program.evaluate(&frame).unwrap();
            assert_eq!(
                allocation::allocations(|| assert_eq!(
                    prepared.evaluate(&frame).unwrap(),
                    expected
                )),
                0
            );
        }
    }
}

#[test]
fn literal_padding_matches_reference_with_no_recurring_allocation() {
    let program = program();
    let (mut prepared, retained) = allocation::rounded_retained_bytes(|| {
        PreparedPortableExpressionEvaluator::new(&program).unwrap()
    });
    eprintln!("literal-padding rounded retained bytes: {retained}");
    if cfg!(target_pointer_width = "64") {
        assert!(retained <= 4096, "literal child storage regressed");
    }
    for value in 0..=255 {
        let expected = program.evaluate(&[value]).unwrap();
        assert_eq!(
            allocation::allocations(|| {
                assert_eq!(prepared.evaluate(&[value]).unwrap(), expected);
            }),
            0
        );
    }
    assert!(prepared.evaluate(&[]).is_err());
    assert!(prepared.evaluate(&[0, 0]).is_err());
}

#[test]
fn structured_child_keeps_conditional_failure_lazy() {
    let program = compile(
        "type Frame = {\n value: U8\n}\nplot lazy (\n value: U8 >> result: Frame\n) = ({value: (. == 0 ? 0 : (. / 0))})",
        "lazy",
    );
    let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    for input in [[0], [1], [0]] {
        let expected = program.evaluate(&input);
        assert_eq!(
            allocation::allocations(|| {
                match &expected {
                    Ok(bytes) => assert_eq!(prepared.evaluate(&input).unwrap(), bytes),
                    Err(reason) => assert_eq!(&prepared.evaluate(&input).unwrap_err(), reason),
                }
            }),
            0
        );
    }
}
