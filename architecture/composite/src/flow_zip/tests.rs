use super::*;
use conduit_core::{kind_id, StructuredInfoValue, StructuredInfoValueShape, TEXT_INFO_ID};
use conduit_kernel::ValueRef;

fn operation() -> FlowZipBack {
    let left = CheckedValueContract {
        value_kind: kind_id(TEXT_INFO_ID),
        maximum_bytes: 16,
        constraints: vec![],
    };
    let right = CheckedValueContract {
        value_kind: kind_id("value/count"),
        maximum_bytes: 8,
        constraints: vec![],
    };
    FlowZipBack::prepare(&left, &right).unwrap()
}

fn reference(slot: u16, bytes: &[u8]) -> ValueRef {
    ValueRef {
        slot,
        generation: 1,
        byte_len: bytes.len() as u32,
    }
}

#[test]
fn opposite_arrival_order_emits_one_exact_pair_and_clears_both_slots() {
    let mut operation = operation();
    let count = conduit_core::encode_count(7);
    let mut io = StepIo::test_frame(
        [None, Some(reference(2, &count))],
        [false; 2],
        [Some(operation.output_maximum), None],
        None,
        16,
    );
    assert_eq!(
        operation.step(
            &mut io,
            &StepInputBytes::test_frame([None, Some(&count)], None)
        ),
        StepOutcome::Progress
    );
    assert!(io.test_consumed(PortId(1)));
    <FlowZipBack as StepBack<2>>::step_committed(&mut operation);

    let mut io = StepIo::test_frame(
        [Some(reference(1, b"hello")), None],
        [false; 2],
        [Some(operation.output_maximum), None],
        None,
        16,
    );
    assert_eq!(
        operation.step(
            &mut io,
            &StepInputBytes::test_frame([Some(b"hello"), None], None)
        ),
        StepOutcome::Progress
    );
    <FlowZipBack as StepBack<2>>::step_committed(&mut operation);

    let mut io = StepIo::test_frame(
        [None, None],
        [false; 2],
        [Some(operation.output_maximum), None],
        None,
        16,
    );
    assert_eq!(
        operation.step(&mut io, &StepInputBytes::test_frame([None, None], None)),
        StepOutcome::Progress
    );
    let encoded = <FlowZipBack as StepBack<2>>::prepared_output(&operation, PortId(0)).unwrap();
    let value = StructuredInfoValue::from_canonical_bytes(encoded).unwrap();
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("zip output must be the canonical tuple")
    };
    assert!(
        matches!(fields[0].value().shape(), StructuredInfoValueShape::Leaf(bytes) if bytes == b"hello")
    );
    assert!(
        matches!(fields[1].value().shape(), StructuredInfoValueShape::Leaf(bytes) if bytes == count)
    );
    <FlowZipBack as StepBack<2>>::step_committed(&mut operation);
    assert!(operation.left_len.is_none());
    assert!(operation.right_len.is_none());
}

#[test]
fn normal_close_discards_one_unmatched_value_and_uses_exact_input_contract() {
    let mut operation = operation();
    operation.left[..4].copy_from_slice(b"solo");
    operation.left_len = Some(4);
    let contracts = <FlowZipBack as StepBack<2>>::terminal_transductions(&operation);
    assert_eq!(contracts[0].unwrap().input, PortId(0));
    assert_eq!(contracts[1].unwrap().input, PortId(1));

    let mut io = StepIo::test_frame(
        [None, None],
        [true, false],
        [Some(operation.output_maximum), None],
        None,
        16,
    );
    assert_eq!(
        operation.step(&mut io, &StepInputBytes::test_frame([None, None], None)),
        StepOutcome::Complete
    );
    assert!(io.test_consumed_closed(PortId(0)));
    assert!(operation.left_len.is_none());
}
