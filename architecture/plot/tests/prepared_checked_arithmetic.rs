//! Arithmetic-only policy tests; these programs do not assert Native law custody.
use conduit_core::{kind_id, StructuredFieldType, StructuredInfoType};
use conduit_plot::{
    BinaryOperator as B, PortableExpressionEvaluationRefusal as Refusal,
    PortableExpressionNode as Node, PortableExpressionOperation as Op,
    PortableExpressionProgram as Program, PortableExpressionProjection as Projection,
    PreparedPortableExpressionEvaluator as Evaluator,
};
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn ty(kind: &str) -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id(kind)).unwrap()
}
fn literal(kind: &str, value: &str) -> Node {
    Node {
        value_type: ty(kind),
        operation: Op::Literal(value.into()),
    }
}
fn input(kind: &str) -> Node {
    Node {
        value_type: ty(kind),
        operation: Op::Input,
    }
}
fn binary(operator: B, proven: bool, left: Node, right: Node, output: &str) -> Node {
    Node {
        value_type: ty(output),
        operation: Op::Binary {
            operator,
            proven,
            left: Box::new(left),
            right: Box::new(right),
        },
    }
}
fn program(root: Node) -> Program {
    Program {
        input_type: ty("value/u8"),
        input_maximum_bytes: Some(1),
        output_type: root.value_type.clone(),
        root,
    }
}
fn add() -> Node {
    binary(
        B::Add,
        true,
        input("value/u8"),
        literal("value/u8", "1"),
        "value/u8",
    )
}
#[test]
fn checked_policy_refuses_proven_overflow_underflow_and_multiply_without_changing_program() {
    for (operator, input, rhs, expected) in [
        (B::Add, 255, "1", 0),
        (B::Subtract, 0, "1", 255),
        (B::Multiply, 128, "2", 0),
    ] {
        let source = program(binary(
            operator,
            true,
            self::input("value/u8"),
            literal("value/u8", rhs),
            "value/u8",
        ));
        let original = source.clone();
        let bytes = source.canonical_bytes().unwrap();
        let mut legacy = Evaluator::new(&source).unwrap();
        let mut checked = Evaluator::new_checked_arithmetic(&source).unwrap();
        assert_eq!(legacy.evaluate(&[input]).unwrap(), &[expected]);
        assert!(matches!(
            checked.evaluate(&[input]),
            Err(Refusal::Arithmetic)
        ));
        assert_eq!(source, original);
        assert_eq!(source.canonical_bytes().unwrap(), bytes);
    }
    for (operator, value, rhs) in [
        (B::Add, i64::MAX, "1"),
        (B::Subtract, i64::MIN, "1"),
        (B::Multiply, i64::MAX, "2"),
        (B::Divide, i64::MIN, "-1"),
        (B::Divide, 1, "0"),
        (B::Remainder, 1, "0"),
    ] {
        let root = binary(
            operator,
            true,
            input("value/i64"),
            literal("value/i64", rhs),
            "value/i64",
        );
        let source = Program {
            input_type: ty("value/i64"),
            input_maximum_bytes: Some(8),
            output_type: root.value_type.clone(),
            root,
        };
        let mut checked = Evaluator::new_checked_arithmetic(&source).unwrap();
        assert!(matches!(
            checked.evaluate(&value.to_le_bytes()),
            Err(Refusal::Arithmetic)
        ));
    }
}
#[test]
fn unproven_arithmetic_and_unsupported_preparation_keep_legacy_refusals() {
    let source = program(binary(
        B::Add,
        false,
        input("value/u8"),
        literal("value/u8", "1"),
        "value/u8",
    ));
    assert!(matches!(
        Evaluator::new(&source).unwrap().evaluate(&[255]),
        Err(Refusal::Arithmetic)
    ));
    assert!(matches!(
        Evaluator::new_checked_arithmetic(&source)
            .unwrap()
            .evaluate(&[255]),
        Err(Refusal::Arithmetic)
    ));
    let unknown = Program {
        input_type: ty("unknown/intrinsic"),
        input_maximum_bytes: Some(1),
        output_type: ty("unknown/intrinsic"),
        root: input("unknown/intrinsic"),
    };
    assert!(Evaluator::new(&unknown).is_err());
    assert!(Evaluator::new_checked_arithmetic(&unknown).is_err());
}
#[test]
fn selected_conditional_branch_checks_arithmetic_and_unselected_branch_stays_lazy() {
    let condition = binary(
        B::Equal,
        false,
        input("value/u8"),
        literal("value/u8", "255"),
        "value/bool",
    );
    let source = program(Node {
        value_type: ty("value/u8"),
        operation: Op::Conditional {
            condition: Box::new(condition),
            when_true: Box::new(add()),
            when_false: Box::new(literal("value/u8", "7")),
        },
    });
    let mut checked = Evaluator::new_checked_arithmetic(&source).unwrap();
    assert_eq!(checked.evaluate(&[0]).unwrap(), &[7]);
    assert!(matches!(checked.evaluate(&[255]), Err(Refusal::Arithmetic)));
    let division = binary(
        B::Divide,
        true,
        literal("value/u8", "1"),
        input("value/u8"),
        "value/u8",
    );
    let source = program(Node {
        value_type: ty("value/u8"),
        operation: Op::Conditional {
            condition: Box::new(binary(
                B::Equal,
                false,
                input("value/u8"),
                literal("value/u8", "0"),
                "value/bool",
            )),
            when_true: Box::new(literal("value/u8", "7")),
            when_false: Box::new(division),
        },
    });
    assert_eq!(
        Evaluator::new_checked_arithmetic(&source)
            .unwrap()
            .evaluate(&[0])
            .unwrap(),
        &[7]
    );
}
fn record(value: Node) -> Node {
    Node {
        value_type: StructuredInfoType::record(
            kind_id("test/arithmetic-only-record"),
            vec![StructuredFieldType::new("sum", ty("value/u8")).unwrap()],
        )
        .unwrap(),
        operation: Op::Record(vec![("sum".into(), value)]),
    }
}
#[test]
fn policy_survives_structured_equality_member_and_sequence_child_preparation() {
    let constructor = record(add());
    let member = Node {
        value_type: ty("value/u8"),
        operation: Op::Projection {
            value: Box::new(record(add())),
            member: Projection::Field("sum".into()),
        },
    };
    let equality = binary(
        B::Equal,
        false,
        record(add()),
        record(literal("value/u8", "0")),
        "value/bool",
    );
    let source = Node {
        value_type: StructuredInfoType::collection(ty("value/u8"), Some(2)).unwrap(),
        operation: Op::Collection(vec![add(), literal("value/u8", "1")]),
    };
    let selection = Node {
        value_type: ty("value/u8"),
        operation: Op::SemanticCall {
            kind: "sequence/at".into(),
            arguments: vec![source, literal("value/u64", "0")],
        },
    };
    for root in [constructor, member, equality, selection] {
        let source = program(root);
        assert!(Evaluator::new(&source).unwrap().evaluate(&[255]).is_ok());
        let mut checked = Evaluator::new_checked_arithmetic(&source).unwrap();
        assert!(matches!(checked.evaluate(&[255]), Err(Refusal::Arithmetic)));
    }
}
#[test]
fn checked_profile_reuses_prepared_storage_without_play_allocation() {
    let source = program(add());
    let mut checked = Evaluator::new_checked_arithmetic(&source).unwrap();
    let ((), observed) = allocation_probe::observe(|| {
        for _ in 0..3 {
            assert_eq!(checked.evaluate(&[1]).unwrap(), &[2]);
            assert!(matches!(checked.evaluate(&[255]), Err(Refusal::Arithmetic)));
            assert_eq!(checked.evaluate(&[2]).unwrap(), &[3]);
        }
    });
    assert_eq!(observed.allocations, 0);
    assert_eq!(observed.reallocations, 0);
}
