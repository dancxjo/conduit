//! Native register evaluation is compared with the existing byte-based Back.
#[path = "prepared_structured_payload/allocation.rs"]
mod allocation;
use conduit_core::{kind_id, StructuredInfoType};
use conduit_plot::{
    BinaryOperator as B, PortableExpressionNode as Node, PortableExpressionOperation as Op,
    PortableExpressionProgram as Program, PreparedNativeU64Expression as Native,
    PreparedPortableExpressionEvaluator as Reference, UnaryOperator as U,
};
fn ty(name: &str) -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id(name)).unwrap()
}
fn word(operation: Op) -> Node {
    Node {
        value_type: ty("value/u64"),
        operation,
    }
}
fn boolean(operation: Op) -> Node {
    Node {
        value_type: ty("value/bool"),
        operation,
    }
}
fn input() -> Node {
    word(Op::Input)
}
fn literal(value: u64) -> Node {
    word(Op::Literal(value.to_string()))
}
fn binary(operator: B, proven: bool, left: Node, right: Node, condition: bool) -> Node {
    let operation = Op::Binary {
        operator,
        proven,
        left: Box::new(left),
        right: Box::new(right),
    };
    if condition {
        boolean(operation)
    } else {
        word(operation)
    }
}
fn select(condition: Node, yes: Node, no: Node) -> Node {
    word(Op::Conditional {
        condition: Box::new(condition),
        when_true: Box::new(yes),
        when_false: Box::new(no),
    })
}
fn program(root: Node) -> Program {
    Program {
        input_type: ty("value/u64"),
        input_maximum_bytes: Some(8),
        output_type: root.value_type.clone(),
        root,
    }
}
fn compare(program: &Program, inputs: &[u64]) {
    let native = Native::new(program).unwrap();
    let mut reference = Reference::new(program).unwrap();
    let allocations = allocation::allocations(|| {
        for input in inputs {
            let expected = reference
                .evaluate(&input.to_le_bytes())
                .map(|b| u64::from_le_bytes(b.try_into().unwrap()));
            assert_eq!(native.evaluate(*input), expected, "input={input}");
        }
    });
    assert_eq!(allocations, 0);
}
#[test]
fn fixed_width_arithmetic_preserves_checked_proven_and_shift_overflow_laws() {
    let inputs = [
        0,
        1,
        2,
        3,
        7,
        255,
        256,
        1 << 31,
        1 << 32,
        1 << 63,
        u64::MAX - 1,
        u64::MAX,
    ];
    for operator in [
        B::Add,
        B::Subtract,
        B::Multiply,
        B::Divide,
        B::Remainder,
        B::BitAnd,
        B::BitOr,
        B::BitXor,
        B::ShiftLeft,
        B::ShiftRight,
    ] {
        for right in [0, 1, 2, 7, 31, 32, 63, 64, 65, u64::MAX] {
            for proven in [false, true] {
                compare(
                    &program(binary(operator, proven, input(), literal(right), false)),
                    &inputs,
                );
            }
        }
    }
}
#[test]
fn comparisons_boolean_operations_and_conditionals_keep_native_values_and_lazy_errors() {
    for operator in [
        B::Equal,
        B::NotEqual,
        B::Less,
        B::LessOrEqual,
        B::Greater,
        B::GreaterOrEqual,
    ] {
        let condition = binary(operator, false, input(), literal(4), true);
        compare(
            &program(select(condition, literal(11), literal(22))),
            &[0, 1, 3, 4, 5, u64::MAX],
        );
    }
    for operator in [B::BooleanAnd, B::BooleanOr, B::Equal, B::NotEqual] {
        let a = binary(B::Less, false, input(), literal(4), true);
        let b = binary(B::NotEqual, false, input(), literal(0), true);
        let condition = binary(operator, false, a, b, true);
        compare(
            &program(select(condition, literal(11), literal(22))),
            &[0, 1, 3, 4, 5, u64::MAX],
        );
    }
    let condition = boolean(Op::Unary {
        operator: U::Not,
        operand: Box::new(binary(B::Equal, false, input(), literal(0), true)),
    });
    let bad = binary(B::Divide, false, literal(1), literal(0), false);
    compare(
        &program(select(condition, literal(11), bad)),
        &[0, 1, 2, u64::MAX],
    );
    compare(
        &program(word(Op::Unary {
            operator: U::Negate,
            operand: Box::new(input()),
        })),
        &[0, 1, u64::MAX],
    );
}
#[test]
fn the_profile_refuses_other_semantic_types_and_unbounded_native_work() {
    let mut other = program(input());
    other.input_type = ty("value/u32");
    assert!(Native::new(&other).is_err());
    other = program(input());
    other.input_type = ty("test/different-u64-meaning");
    assert!(Native::new(&other).is_err());
    let mut root = input();
    for _ in 0..65 {
        root = binary(B::Add, false, root, literal(1), false);
    }
    assert!(Native::new(&program(root)).is_err());
    assert_eq!(Native::new(&program(input())).unwrap().node_count(), 1);
}
