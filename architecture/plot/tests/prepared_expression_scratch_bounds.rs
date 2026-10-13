//! AST provenance preserves finite scratch without truncating constructed values.
#[path = "prepared_structured_payload/allocation.rs"]
mod allocation;
use conduit_core::{kind_id, StructuredFieldType, StructuredInfoType};
use conduit_plot::{
    PortableExpressionNode as Node, PortableExpressionOperation as Op,
    PortableExpressionProgram as Program, PortableExpressionProjection as Projection,
    PreparedPortableExpressionEvaluator as Evaluator,
};
fn bytes() -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id("value/bytes")).unwrap()
}
fn record(name: &str, fields: &[(&str, StructuredInfoType)]) -> StructuredInfoType {
    StructuredInfoType::record(
        kind_id(name),
        fields
            .iter()
            .map(|(name, ty)| StructuredFieldType::new(*name, ty.clone()).unwrap())
            .collect(),
    )
    .unwrap()
}
fn input() -> Node {
    Node {
        value_type: bytes(),
        operation: Op::Input,
    }
}
fn make_program(root: Node, bound: Option<u32>) -> Program {
    Program {
        input_maximum_bytes: bound,
        input_type: bytes(),
        output_type: root.value_type.clone(),
        root,
    }
}
#[test]
fn bounded_identity_preserves_exact_wire_without_evaluation_allocations() {
    let program = make_program(input(), Some(2048));
    let mut evaluator = Evaluator::new(&program).unwrap();
    assert_eq!(evaluator.output_capacity(), 2048);
    assert_eq!(program.maximum_prepared_output_bytes().unwrap(), 2048);
    let wire = vec![0xa5; 2048];
    assert_eq!(
        allocation::allocations(|| for _ in 0..64 {
            assert_eq!(evaluator.evaluate(&wire).unwrap(), wire);
        }),
        0
    );
    assert!(evaluator.evaluate(&vec![0; 2049]).is_err());
    let unbounded = Evaluator::new(&make_program(input(), None)).unwrap();
    assert!(unbounded.output_capacity() > 2048);
    assert_eq!(
        make_program(input(), None)
            .maximum_prepared_output_bytes()
            .unwrap() as usize,
        unbounded.output_capacity()
    );
}
#[test]
fn construction_and_selection_can_exceed_the_admitted_input_envelope() {
    let inner = record("test/duplicated", &[("a", bytes()), ("b", bytes())]);
    let constructed = Node {
        value_type: inner.clone(),
        operation: Op::Record(vec![("a".into(), input()), ("b".into(), input())]),
    };
    let outer = record("test/outer", &[("selected", inner.clone())]);
    let root = Node {
        value_type: inner,
        operation: Op::Projection {
            value: Box::new(Node {
                value_type: outer,
                operation: Op::Record(vec![("selected".into(), constructed)]),
            }),
            member: Projection::Field("selected".into()),
        },
    };
    let program = make_program(root, Some(64));
    let wire = vec![0x5a; 64];
    let expected = program.evaluate(&wire).unwrap();
    assert!(expected.len() > 64);
    let maximum = program.maximum_prepared_output_bytes().unwrap() as usize;
    assert!(maximum > 64 && maximum >= expected.len());
    let mut evaluator = Evaluator::new(&program).unwrap();
    assert!(evaluator.output_capacity() >= expected.len());
    assert_eq!(
        allocation::allocations(|| for _ in 0..32 {
            assert_eq!(evaluator.evaluate(&wire).unwrap(), expected);
        }),
        0
    );
}

#[test]
fn seventy_two_packet_selection_reserves_input_bounded_scratch() {
    use conduit_core::{StructuredFieldValue, StructuredInfoValue};
    let packet = record("test/packet", &[("wire", bytes())]);
    let collection = StructuredInfoType::collection(packet.clone(), Some(72)).unwrap();
    let packets = (0..72)
        .map(|index| {
            StructuredInfoValue::record(
                packet.clone(),
                vec![StructuredFieldValue::new(
                    "wire",
                    StructuredInfoValue::leaf(bytes(), vec![index; 192]).unwrap(),
                )
                .unwrap()],
            )
            .unwrap()
        })
        .collect();
    let input = StructuredInfoValue::collection(collection.clone(), packets)
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let program = Program {
        input_maximum_bytes: Some(32768),
        input_type: collection.clone(),
        output_type: packet.clone(),
        root: Node {
            value_type: packet,
            operation: Op::SemanticCall {
                kind: "sequence/at".into(),
                arguments: vec![
                    Node {
                        value_type: collection,
                        operation: Op::Input,
                    },
                    Node {
                        value_type: StructuredInfoType::leaf(kind_id("value/u64")).unwrap(),
                        operation: Op::Literal("71".into()),
                    },
                ],
            },
        },
    };
    let expected = program.evaluate(&input).unwrap();
    let maximum = program.maximum_prepared_output_bytes().unwrap() as usize;
    assert!(maximum <= 32768 && maximum >= expected.len());
    let (mut evaluators, retained) = allocation::rounded_retained_bytes(|| {
        (0..9)
            .map(|_| Evaluator::new(&program).unwrap())
            .collect::<Vec<_>>()
    });
    assert!(
        retained < 2 * 1024 * 1024,
        "nine selectors retained {retained} bytes"
    );
    assert_eq!(
        allocation::allocations(|| for evaluator in &mut evaluators {
            assert_eq!(evaluator.evaluate(&input).unwrap(), expected);
        }),
        0
    );
}
