//! Generic checked opaque byte windows; no transport or class constants.
use conduit_core::{StructuredInfoType, kind_id};
use conduit_plot::{
    PortableExpressionNode as Node, PortableExpressionOperation as Op, PortableExpressionProgram,
    PreparedPortableExpressionEvaluator,
};
fn program(offset: u64, length: u64) -> PortableExpressionProgram {
    let bytes = StructuredInfoType::leaf(kind_id("value/bytes")).unwrap();
    let integer = StructuredInfoType::leaf(kind_id("value/u64")).unwrap();
    PortableExpressionProgram {
        input_type: bytes.clone(),
        output_type: bytes.clone(),
        root: Node {
            value_type: bytes.clone(),
            operation: Op::SemanticCall {
                kind: "bytes/window".into(),
                arguments: vec![
                    Node {
                        value_type: bytes,
                        operation: Op::Input,
                    },
                    Node {
                        value_type: integer.clone(),
                        operation: Op::Literal(offset.to_string()),
                    },
                    Node {
                        value_type: integer,
                        operation: Op::Literal(length.to_string()),
                    },
                ],
            },
        },
    }
}
#[test]
fn portable_and_prepared_window_preserve_exact_binary_extent_without_growth() {
    let input = [0, 255, 128, 17, 0];
    for (offset, length, expected) in [
        (0, 5, &input[..]),
        (1, 3, &input[1..4]),
        (5, 0, &input[5..]),
    ] {
        let program = program(offset, length);
        assert_eq!(program.evaluate(&input).unwrap(), expected);
        let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
        let capacity = prepared.output_capacity();
        for _ in 0..16 {
            assert_eq!(prepared.evaluate(&input).unwrap(), expected);
            assert_eq!(prepared.output_capacity(), capacity);
        }
    }
}
#[test]
fn invalid_extent_and_overflow_refuse_in_both_evaluators() {
    for (offset, length) in [(6, 0), (4, 2), (u64::MAX, 1), (0, u64::MAX)] {
        let program = program(offset, length);
        assert!(program.evaluate(&[0; 5]).is_err());
        let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
        assert!(prepared.evaluate(&[0; 5]).is_err());
    }
}
#[test]
fn forged_result_kind_and_missing_argument_cannot_prepare() {
    let mut wrong = program(0, 1);
    wrong.root.value_type = StructuredInfoType::leaf(kind_id("value/text")).unwrap();
    wrong.output_type = wrong.root.value_type.clone();
    assert!(PreparedPortableExpressionEvaluator::new(&wrong).is_err());
    let mut missing = program(0, 1);
    let Op::SemanticCall { arguments, .. } = &mut missing.root.operation else {
        unreachable!()
    };
    arguments.pop();
    assert!(PreparedPortableExpressionEvaluator::new(&missing).is_err());
}
