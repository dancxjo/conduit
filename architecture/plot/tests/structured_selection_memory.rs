//! Prepared selection uses its enclosing evaluator's admitted output storage.
#[path = "prepared_structured_payload/allocation.rs"]
mod allocation;
use conduit_core::{
    ConfigurationValue, StructuredFieldValue, StructuredInfoType as Type,
    StructuredInfoTypeShape as T, StructuredInfoValue as Value,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog,
};

fn program(indexed: bool) -> PortableExpressionProgram {
    let source = "type Payload = {\n samples: sequence U8 <= 1024\n}\n\
        type Envelope = {\n payload: Payload\n}\n\
        type Selection = {\n items: collection Payload = 2\n index: U64\n}\n\
        plot member (\n input: Envelope >> result: Payload\n) = (.payload)\n\
        plot indexed (\n input: Selection >> result: Payload\n) = (sequence/at(.items, .index))\n";
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let expanded = expand_canonical_plot_for_authoring(
        &checked,
        if indexed { "indexed" } else { "member" },
        &ProfileCatalog::new(),
    )
    .unwrap()
    .expanded;
    let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
        panic!("program")
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
fn member<'a>(ty: &'a Type, name: &str) -> &'a Type {
    let T::Record { fields, .. } = ty.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|f| f.name() == name)
        .unwrap()
        .value_type()
}
fn record<const N: usize>(ty: &Type, fields: [(&str, Value); N]) -> Value {
    Value::record(
        ty.clone(),
        fields
            .into_iter()
            .map(|(name, value)| StructuredFieldValue::new(name, value).unwrap())
            .collect(),
    )
    .unwrap()
}
fn payload(ty: &Type, length: usize, seed: u8) -> Value {
    let samples = member(ty, "samples");
    let T::Sequence { element, .. } = samples.shape() else {
        panic!("samples")
    };
    record(
        ty,
        [(
            "samples",
            Value::sequence(
                samples.clone(),
                (0..length)
                    .map(|i| {
                        Value::leaf(element.clone(), vec![(i as u8).wrapping_add(seed)]).unwrap()
                    })
                    .collect(),
            )
            .unwrap(),
        )],
    )
}
fn input(
    program: &PortableExpressionProgram,
    indexed: bool,
    index: u64,
    length: usize,
) -> (Vec<u8>, Vec<u8>) {
    let first = payload(&program.output_type, length, 3);
    let second = payload(&program.output_type, length, 17);
    let expected = if indexed && index == 1 {
        &second
    } else {
        &first
    }
    .canonical_bytes()
    .unwrap();
    let input = if indexed {
        record(
            &program.input_type,
            [
                (
                    "items",
                    Value::collection(
                        member(&program.input_type, "items").clone(),
                        vec![first, second],
                    )
                    .unwrap(),
                ),
                (
                    "index",
                    Value::leaf(
                        member(&program.input_type, "index").clone(),
                        index.to_le_bytes().to_vec(),
                    )
                    .unwrap(),
                ),
            ],
        )
    } else {
        record(&program.input_type, [("payload", first)])
    };
    (input.canonical_bytes().unwrap(), expected)
}
#[test]
fn structured_projection_retains_one_result_buffer_and_remains_allocation_free() {
    exercise(false);
}
#[test]
fn dynamic_structured_index_retains_no_second_selection_buffer() {
    exercise(true);
}
fn exercise(indexed: bool) {
    let program = program(indexed);
    let (mut evaluator, retained) = allocation::rounded_retained_bytes(|| {
        PreparedPortableExpressionEvaluator::new(&program).unwrap()
    });
    // Native allocation-unit regression bounds, not physical MCU admission.
    // Dynamic indexing also retains the source collection (two payloads).
    let maximum = if indexed {
        3 * evaluator.output_capacity() + 4096
    } else {
        evaluator.output_capacity() + 2048
    };
    assert!(
        retained < maximum,
        "indexed={indexed}: {retained} retained, {maximum} maximum"
    );
    eprintln!(
        "indexed={indexed}: {retained} retained bytes; {} output capacity",
        evaluator.output_capacity()
    );
    for length in [0, 1, 1024] {
        for index in 0..=u64::from(indexed) {
            let (bytes, expected) = input(&program, indexed, index, length);
            assert_eq!(
                allocation::allocations(|| {
                    assert_eq!(evaluator.evaluate(&bytes).unwrap(), expected);
                }),
                0
            );
            assert_eq!(
                allocation::allocations(|| {
                    assert!(evaluator.evaluate(&bytes[..bytes.len() - 1]).is_err());
                }),
                0
            );
            assert_eq!(evaluator.evaluate(&bytes).unwrap(), expected);
        }
    }
    if indexed {
        let (bytes, _) = input(&program, true, 2, 1);
        assert_eq!(
            allocation::allocations(|| {
                assert!(evaluator.evaluate(&bytes).is_err());
            }),
            0
        );
        let (bytes, expected) = input(&program, true, 0, 1);
        assert_eq!(evaluator.evaluate(&bytes).unwrap(), expected);
    }
}
