//! Encoding parity and conservative optimization eligibility, not a Host offer.
#[path = "prepared_structured_payload/allocation.rs"]
mod allocation;
use conduit_core::{kind_id, StructuredFieldType, StructuredInfoType as Type};
use conduit_plot::{
    PortableExpressionNode as Node, PortableExpressionOperation as Operation,
    PortableExpressionProgram as Program, PreparedPortableExpressionEvaluator as Evaluator,
};

fn octet() -> Type {
    Type::leaf(kind_id("value/u8")).unwrap()
}
fn input() -> Node {
    Node {
        value_type: octet(),
        operation: Operation::Input,
    }
}
fn program(root: Node) -> Program {
    Program {
        input_maximum_bytes: None,
        input_type: octet(),
        output_type: root.value_type.clone(),
        root,
    }
}
fn parity(program: &Program, values: &[u8]) {
    let mut prepared = Evaluator::new(program).unwrap();
    for value in values {
        let expected = program.evaluate(&[*value]).unwrap();
        assert_eq!(
            allocation::allocations(|| assert_eq!(prepared.evaluate(&[*value]).unwrap(), expected)),
            0
        );
    }
    assert!(prepared.evaluate(&[]).is_err());
    let expected = program.evaluate(&[7]).unwrap();
    assert_eq!(prepared.evaluate(&[7]).unwrap(), expected);
}

#[test]
fn tuple_fields_have_the_same_canonical_names_and_order() {
    let pair = Type::record(
        kind_id("test/tape-pair"),
        ["item-00000", "item-00001"]
            .into_iter()
            .map(|name| StructuredFieldType::new(name, octet()).unwrap())
            .collect(),
    )
    .unwrap();
    let root = Node {
        value_type: pair,
        operation: Operation::Tuple(vec![
            input(),
            Node {
                value_type: octet(),
                operation: Operation::Literal("7".into()),
            },
        ]),
    };
    parity(&program(root), &(0..=255).collect::<Vec<_>>());
}

#[test]
fn exceeding_tape_value_capacity_keeps_the_ordinary_prepared_path() {
    let root = Node {
        value_type: Type::collection(octet(), Some(129)).unwrap(),
        operation: Operation::Collection(vec![input(); 129]),
    };
    parity(&program(root), &[0, 1, 127, 255]);
}

#[test]
fn deep_constructors_keep_ordinary_eligibility_and_canonical_parity() {
    let mut root = input();
    for _ in 0..conduit_core::MAXIMUM_STRUCTURED_INFO_DEPTH - 1 {
        root = Node {
            value_type: Type::collection(root.value_type.clone(), Some(1)).unwrap(),
            operation: Operation::Collection(vec![root]),
        };
    }
    parity(&program(root), &[0, 255]);
}

#[test]
fn tape_eligibility_never_accepts_duplicate_record_fields() {
    let record = Type::record(
        kind_id("test/tape-record"),
        ["left", "right"]
            .into_iter()
            .map(|name| StructuredFieldType::new(name, octet()).unwrap())
            .collect(),
    )
    .unwrap();
    let invalid = program(Node {
        value_type: record,
        operation: Operation::Record(vec![("left".into(), input()), ("left".into(), input())]),
    });
    assert!(Evaluator::new(&invalid).is_err());
    assert!(invalid.evaluate(&[1]).is_err());
}

#[test]
fn conditional_record_field_keeps_following_constant_field_on_both_branches() {
    let boolean = Type::leaf(kind_id(conduit_core::BOOL_INFO_ID)).unwrap();
    let record = Type::record(kind_id("test/tape-conditional-join"),
        ["left", "right"].into_iter().map(|name|
            StructuredFieldType::new(name, octet()).unwrap()).collect()).unwrap();
    let literal = |value: &str| Node { value_type: octet(),
        operation: Operation::Literal(value.into()) };
    let conditional = Node { value_type: octet(), operation: Operation::Conditional {
        condition: Box::new(Node { value_type: boolean.clone(), operation: Operation::Input }),
        when_true: Box::new(literal("11")), when_false: Box::new(literal("22")),
    }};
    let program = Program { input_maximum_bytes: None, input_type: boolean,
        output_type: record.clone(), root: Node { value_type: record,
        operation: Operation::Record(vec![("left".into(), conditional),
            ("right".into(), literal("33"))]) }};
    let mut prepared = Evaluator::new(&program).unwrap();
    for value in [1, 0, 1] {
        let expected = program.evaluate(&[value]).unwrap();
        assert_eq!(prepared.evaluate(&[value]).unwrap(), expected, "branch {value}");
    }
}
