//! Constructor input locations retain canonical validation and exact semantics.
#[path = "prepared_structured_payload/allocation.rs"]
mod allocation;
use conduit_core::{
    kind_id, validate_canonical_structured_value, StructuredFieldType as FieldType,
    StructuredFieldValue as Field, StructuredInfoType as Type, StructuredInfoValue as Value,
};
use conduit_plot::{
    PortableExpressionNode as Node, PortableExpressionOperation as Op,
    PortableExpressionProgram as Program, PortableExpressionProjection as Projection,
    PreparedPortableExpressionEvaluator as Evaluator,
};

fn leaf(kind: &str) -> Type {
    Type::leaf(kind_id(kind)).unwrap()
}
fn record(name: &str, fields: Vec<(&str, Type)>) -> Type {
    Type::record(
        kind_id(name),
        fields
            .into_iter()
            .map(|(name, ty)| FieldType::new(name, ty).unwrap())
            .collect(),
    )
    .unwrap()
}
fn input(ty: &Type) -> Node {
    Node {
        value_type: ty.clone(),
        operation: Op::Input,
    }
}
fn select(source: Node, member: Projection, ty: &Type) -> Node {
    Node {
        value_type: ty.clone(),
        operation: Op::Projection {
            value: Box::new(source),
            member,
        },
    }
}
fn field(source: Node, name: &str, ty: &Type) -> Node {
    select(source, Projection::Field(name.into()), ty)
}
fn program(input_type: &Type, children: Vec<(&str, Node)>) -> Program {
    let output_type = record(
        "test/fixed-output",
        children
            .iter()
            .map(|(name, node)| (*name, node.value_type.clone()))
            .collect(),
    );
    Program {
        input_maximum_bytes: None,
        input_type: input_type.clone(),
        output_type: output_type.clone(),
        root: Node {
            value_type: output_type,
            operation: Op::Record(
                children
                    .into_iter()
                    .map(|(name, node)| (name.into(), node))
                    .collect(),
            ),
        },
    }
}
fn values(ty: &Type, fields: Vec<(&str, Value)>) -> Vec<u8> {
    Value::record(
        ty.clone(),
        fields
            .into_iter()
            .map(|(name, value)| Field::new(name, value).unwrap())
            .collect(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
fn parity(prepared: &mut Evaluator, program: &Program, bytes: &[u8]) {
    let expected = program.evaluate(bytes).unwrap();
    assert_eq!(
        allocation::allocations(|| {
            assert_eq!(prepared.evaluate(bytes).unwrap(), expected);
        }),
        0
    );
}

#[test]
fn fixed_record_tuple_collection_and_nominal_nodes_match_the_reference() {
    let octet = leaf("value/u8");
    let word = leaf("value/u16");
    let boolean = leaf("value/bool");
    let flag = Type::nominal(kind_id("test/flag"), boolean.clone()).unwrap();
    let array = Type::collection(word.clone(), Some(3)).unwrap();
    let pair = record(
        "test/pair",
        vec![("item-00000", flag.clone()), ("item-00001", octet.clone())],
    );
    let ty = record(
        "test/fixed-input",
        vec![
            ("alpha", array.clone()),
            ("beta", pair.clone()),
            ("tail", octet.clone()),
        ],
    );
    let program = program(
        &ty,
        vec![
            ("array", field(input(&ty), "alpha", &array)),
            (
                "flag",
                select(
                    field(input(&ty), "beta", &pair),
                    Projection::TupleIndex(0),
                    &flag,
                ),
            ),
            (
                "word",
                select(
                    field(input(&ty), "alpha", &array),
                    Projection::TupleIndex(2),
                    &word,
                ),
            ),
            ("z", field(input(&ty), "tail", &octet)),
        ],
    );
    let mut prepared = Evaluator::new(&program).unwrap();
    for byte in 0..=255u8 {
        let bytes = values(
            &ty,
            vec![
                (
                    "alpha",
                    Value::collection(
                        array.clone(),
                        (0..3)
                            .map(|n| {
                                Value::leaf(
                                    word.clone(),
                                    (u16::from(byte) * 257)
                                        .wrapping_add(n)
                                        .to_le_bytes()
                                        .to_vec(),
                                )
                                .unwrap()
                            })
                            .collect(),
                    )
                    .unwrap(),
                ),
                (
                    "beta",
                    Value::record(
                        pair.clone(),
                        vec![
                            Field::new(
                                "item-00000",
                                Value::nominal(
                                    flag.clone(),
                                    Value::leaf(boolean.clone(), vec![byte & 1]).unwrap(),
                                )
                                .unwrap(),
                            )
                            .unwrap(),
                            Field::new(
                                "item-00001",
                                Value::leaf(octet.clone(), vec![byte]).unwrap(),
                            )
                            .unwrap(),
                        ],
                    )
                    .unwrap(),
                ),
                (
                    "tail",
                    Value::leaf(octet.clone(), vec![byte.rotate_left(3)]).unwrap(),
                ),
            ],
        );
        parity(&mut prepared, &program, &bytes);
        if byte == 0 {
            for length in 0..bytes.len() {
                assert!(prepared.evaluate(&bytes[..length]).is_err());
            }
            let selected = validate_canonical_structured_value(&bytes)
                .unwrap()
                .record_field("beta")
                .unwrap()
                .unwrap()
                .record_field("item-00000")
                .unwrap()
                .unwrap();
            let offset = selected.value_node().as_ptr() as usize - bytes.as_ptr() as usize;
            let mut malformed = bytes.clone();
            malformed[offset + 5] = 2; // invalid Boolean, though the fixed extent still fits
            assert!(prepared.evaluate(&malformed).is_err());
            let mut foreign = bytes.clone();
            let schema = b"test/fixed-input";
            let offset = foreign
                .windows(schema.len())
                .position(|bytes| bytes == schema)
                .unwrap();
            foreign[offset] = b'x'; // valid framing and widths, foreign exact Type
            assert!(validate_canonical_structured_value(&foreign).is_ok());
            assert!(prepared.evaluate(&foreign).is_err());
            parity(&mut prepared, &program, &bytes);
        }
    }
}

#[test]
fn variable_preceding_fields_keep_runtime_selection_and_validate_every_input() {
    let octet = leaf("value/u8");
    let sequence = Type::bounded_sequence(octet.clone(), 0, 4).unwrap();
    let ty = record(
        "test/variable-input",
        vec![("before", sequence.clone()), ("selected", octet.clone())],
    );
    let program = program(&ty, vec![("copy", field(input(&ty), "selected", &octet))]);
    let mut prepared = Evaluator::new(&program).unwrap();
    for length in 0..=4 {
        let sequence = Value::sequence(
            sequence.clone(),
            (0..length)
                .map(|_| Value::leaf(octet.clone(), vec![7]).unwrap())
                .collect(),
        )
        .unwrap();
        let bytes = values(
            &ty,
            vec![
                ("before", sequence),
                (
                    "selected",
                    Value::leaf(octet.clone(), vec![length]).unwrap(),
                ),
            ],
        );
        parity(&mut prepared, &program, &bytes);
    }
}

#[test]
fn incorrect_projection_type_never_becomes_an_eligible_location() {
    let octet = leaf("value/u8");
    let ty = record("test/typed-input", vec![("selected", octet)]);
    let wrong = program(
        &ty,
        vec![("copy", field(input(&ty), "selected", &leaf("value/u16")))],
    );
    assert!(Evaluator::new(&wrong).is_err());
}
