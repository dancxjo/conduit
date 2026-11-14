//! Descriptor identity is local to one immutable constructor/input context.
use super::*;
use alloc::{boxed::Box, string::ToString};
use conduit_core::{kind_id, CheckedValueContract, StructuredFieldType};
fn leaf(n: usize) -> Node {
    Node {
        value_type: StructuredInfoType::leaf(kind_id("value/u64")).unwrap(),
        operation: Expression::Literal(n.to_string()),
    }
}
fn compile(nodes: Vec<Node>) -> (Result<ConstructorTape, CompileRefusal>, usize) {
    let input = StructuredInfoType::record(
        kind_id("test/dedup-input"),
        vec![StructuredFieldType::new("value", leaf(0).value_type).unwrap()],
    )
    .unwrap();
    let root = Node {
        value_type: StructuredInfoType::collection(leaf(0).value_type, Some(nodes.len() as u16))
            .unwrap(),
        operation: Expression::Collection(nodes),
    };
    let prepared = PreparedInput::Structured(input.canonical_bytes().unwrap().into(), None);
    let mut count = 0;
    let tape = ConstructorTape::compile_reported(
        ProgramView {
            input_type: &input,
            input_maximum_bytes: None,
            output_type: &root.value_type,
            root: &root,
        },
        &prepared,
        &mut |_| {
            count += 1;
            Ok(())
        },
    );
    (tape, count)
}
#[test]
fn unique_capacity_is_128_but_every_repeated_invocation_stays_in_code_and_work() {
    let (tape, count) = compile((0..128).map(leaf).collect());
    let tape = tape.unwrap();
    assert_eq!(count, 128);
    assert_eq!(
        tape.code
            .iter()
            .filter(|op| matches!(op, Operation::Value(_)))
            .count(),
        128
    );
    let (refused, count) = compile((0..129).map(leaf).collect());
    assert_eq!(count, 128);
    assert!(matches!(
        refused,
        Err(CompileRefusal::Limit {
            resource: "constructor-values",
            required: 129,
            maximum: 128
        })
    ));
    let (tape, count) = compile((0..129).map(|_| leaf(7)).collect());
    let tape = tape.unwrap();
    assert_eq!(count, 1);
    assert_eq!(
        tape.code
            .iter()
            .filter(|op| matches!(op, Operation::Value(0)))
            .count(),
        129
    );
    let calls = core::cell::Cell::new(0);
    let work =
        crate::expression_prepared::resident_work::structured_work(3, 5, &tape.code, |index| {
            assert_eq!(index, 0);
            calls.set(calls.get() + 1);
            11
        });
    assert_eq!(calls.get(), 129);
    let common = tape
        .code
        .iter()
        .filter(|op| matches!(op, Operation::Bytes { .. } | Operation::InputNode { .. }))
        .count() as u32
        * 2;
    assert_eq!(work, 3 + 5 + 2 + 129 * (11 + 4) + common);
}
#[test]
fn equal_operations_with_different_output_contracts_keep_distinct_descriptors() {
    let guarded = |maximum| Node {
        value_type: leaf(0).value_type,
        operation: Expression::ValueContract {
            input: None,
            output: Some(CheckedValueContract::new(kind_id("value/u64"), maximum, vec![]).unwrap()),
            value: Box::new(leaf(7)),
        },
    };
    let (tape, count) = compile(vec![guarded(8), guarded(7), guarded(8)]);
    let tape = tape.unwrap();
    assert_eq!(count, 2);
    let indices: Vec<_> = tape
        .code
        .iter()
        .filter_map(|op| {
            if let Operation::Value(i) = op {
                Some(*i)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(indices, [0, 1, 0]);
}
