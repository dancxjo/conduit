use conduit_core::{KindId, StructuredInfoType, MAXIMUM_STRUCTURED_LEAF_BYTES};
use conduit_plot::{
    BinaryOperator, PortableExpressionNode, PortableExpressionOperation, PortableExpressionProgram,
    PreparedPortableExpressionEvaluator,
};

#[test]
fn maximum_text_leaves_do_not_become_recursive_stack_storage() {
    std::thread::Builder::new()
        .stack_size(128 * 1024)
        .spawn(|| {
            let text = StructuredInfoType::leaf(KindId::from("value/text")).unwrap();
            let input = PortableExpressionNode {
                value_type: text.clone(),
                operation: PortableExpressionOperation::Input,
            };
            let program = PortableExpressionProgram {
                input_type: text.clone(),
                output_type: text.clone(),
                root: input.clone(),
            };
            let mut identity = PreparedPortableExpressionEvaluator::new(&program).unwrap();
            let maximum = vec![b'a'; MAXIMUM_STRUCTURED_LEAF_BYTES];
            let capacity = identity.output_capacity();
            assert_eq!(identity.evaluate(&maximum).unwrap(), maximum);
            assert_eq!(identity.output_capacity(), capacity);
            assert!(identity
                .evaluate(&vec![b'a'; MAXIMUM_STRUCTURED_LEAF_BYTES + 1])
                .is_err());

            let boolean = StructuredInfoType::leaf(KindId::from("value/bool")).unwrap();
            let comparison = PortableExpressionProgram {
                input_type: text,
                output_type: boolean.clone(),
                root: PortableExpressionNode {
                    value_type: boolean,
                    operation: PortableExpressionOperation::Binary {
                        operator: BinaryOperator::Equal,
                        proven: false,
                        left: Box::new(input.clone()),
                        right: Box::new(input),
                    },
                },
            };
            let mut comparison = PreparedPortableExpressionEvaluator::new(&comparison).unwrap();
            assert_eq!(comparison.evaluate(&maximum).unwrap(), [1]);
        })
        .unwrap()
        .join()
        .unwrap();
}
