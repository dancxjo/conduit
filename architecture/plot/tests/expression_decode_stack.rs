use conduit_core::{kind_id, CheckedValueContract, StructuredInfoType, StructuredInfoValue};
use conduit_plot::{
    BinaryOperator, PortableExpressionNode as Node, PortableExpressionOperation as Op,
    PortableExpressionProgram as Program, PortableExpressionProgramRefusal as Refusal,
    PortableExpressionProjection as Projection, UnaryOperator,
};

fn node(operation: Op) -> Node {
    Node {
        value_type: StructuredInfoType::leaf(kind_id("value/u8")).unwrap(),
        operation,
    }
}
fn input() -> Box<Node> {
    Box::new(node(Op::Input))
}
fn program(root: Node) -> Program {
    Program {
        input_type: root.value_type.clone(),
        input_maximum_bytes: Some(1),
        output_type: root.value_type.clone(),
        root,
    }
}

#[test]
fn every_operation_retains_its_canonical_payload_and_order() {
    let operations = vec![
        Op::Input,
        Op::Literal("7".into()),
        Op::Constant(
            StructuredInfoValue::leaf(
                StructuredInfoType::leaf(kind_id("value/u8")).unwrap(),
                vec![7],
            )
            .unwrap(),
        ),
        Op::Projection {
            value: input(),
            member: Projection::Field("field".into()),
        },
        Op::Projection {
            value: input(),
            member: Projection::TupleIndex(2),
        },
        Op::Unary {
            operator: UnaryOperator::Not,
            operand: input(),
        },
        Op::Binary {
            operator: BinaryOperator::Subtract,
            proven: true,
            left: input(),
            right: Box::new(node(Op::Literal("3".into()))),
        },
        Op::Conditional {
            condition: input(),
            when_true: Box::new(node(Op::Literal("1".into()))),
            when_false: Box::new(node(Op::Literal("2".into()))),
        },
        Op::Tuple(vec![node(Op::Input), node(Op::Literal("2".into()))]),
        Op::Record(vec![
            ("first".into(), node(Op::Input)),
            ("second".into(), node(Op::Literal("2".into()))),
        ]),
        Op::Collection(vec![node(Op::Input)]),
        Op::Variant {
            tag: "case".into(),
            payload: input(),
        },
        Op::SemanticCall {
            kind: "example/call".into(),
            arguments: vec![node(Op::Input)],
        },
        Op::ValueContract {
            input: Some(CheckedValueContract::new(kind_id("value/u8"), 1, vec![]).unwrap()),
            output: None,
            value: input(),
        },
    ];
    let capsule = Node {
        value_type: StructuredInfoType::leaf(kind_id(conduit_core::UNIT_INFO_ID)).unwrap(),
        operation: Op::CanonicalLiteral(conduit_core::Unit::Hertz.encode().to_vec()),
    };
    for root in operations.into_iter().map(node).chain([capsule]) {
        let expected = program(root);
        let bytes = expected.canonical_bytes().unwrap();
        assert_eq!(Program::from_canonical_bytes(&bytes).unwrap(), expected);
        assert_eq!(
            Program::from_canonical_hex(&expected.canonical_hex().unwrap()).unwrap(),
            expected
        );
        for end in 0..bytes.len() {
            assert!(
                Program::from_canonical_bytes(&bytes[..end]).is_err(),
                "accepted truncated program at {end}"
            );
        }
        let mut trailing = bytes;
        trailing.push(0);
        assert_eq!(
            Program::from_canonical_bytes(&trailing),
            Err(Refusal::MalformedEncoding)
        );
    }
}

#[test]
fn deepest_permitted_chain_roundtrips_and_next_level_refuses() {
    let mut root = node(Op::Input);
    for _ in 0..conduit_core::MAXIMUM_STRUCTURED_INFO_DEPTH {
        root = node(Op::Unary {
            operator: UnaryOperator::Not,
            operand: Box::new(root),
        });
    }
    let expected = program(root);
    let bytes = expected.canonical_bytes().unwrap();
    assert_eq!(Program::from_canonical_bytes(&bytes).unwrap(), expected);
    // Insert another unary before the terminal input without asking the encoder to
    // emit an over-depth tree. The node type encoding is identical at every level.
    let leaf = program(node(Op::Input)).canonical_bytes().unwrap();
    let prefix_len = leaf.len() - node(Op::Input).value_type.canonical_bytes().unwrap().len() - 9;
    let node_bytes = &leaf[prefix_len..];
    let mut over = bytes;
    let insert = over.len() - node_bytes.len();
    let mut unary_prefix = node_bytes[..node_bytes.len() - 1].to_vec();
    unary_prefix.extend_from_slice(&[3, 0]);
    over.splice(insert..insert, unary_prefix);
    assert_eq!(
        Program::from_canonical_bytes(&over),
        Err(Refusal::MalformedEncoding)
    );
}
