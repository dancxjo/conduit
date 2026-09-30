use super::*;
use alloc::vec;
use conduit_core::{
    CheckedValueContract, StructuredInfoValue, StructuredInfoValueShape, TEXT_INFO_ID,
};
use conduit_kernel::ValueRef;

fn value(slot: u16, bytes: &[u8]) -> ValueRef {
    ValueRef {
        slot,
        generation: 1,
        byte_len: bytes.len() as u32,
    }
}

fn overflow() -> CanonicalValue {
    CanonicalValue::new(b"bounded collection overflow").unwrap()
}

fn operation(maximum_items: u16) -> FlowCollectBack {
    let contract =
        CheckedValueContract::new(conduit_core::kind_id(TEXT_INFO_ID), 8, vec![]).unwrap();
    let output_maximum = PreparedLeafSequenceEncoder::new(
        contract.value_kind.clone(),
        contract.maximum_bytes,
        maximum_items,
    )
    .unwrap()
    .maximum_bytes();
    FlowCollectBack::prepare(&contract, maximum_items, output_maximum, overflow()).unwrap()
}

fn frame(
    input: Option<(&[u8], u16)>,
    closed: bool,
    output_maximum: Option<u32>,
) -> (StepIo<1>, StepInputBytes<'_, 1>) {
    (
        StepIo::test_frame(
            [input.map(|(bytes, slot)| value(slot, bytes))],
            [closed],
            [output_maximum],
            None,
            16,
        ),
        StepInputBytes::test_frame([input.map(|(bytes, _)| bytes)], None),
    )
}

fn admit(operation: &mut FlowCollectBack, bytes: &[u8], slot: u16) {
    let (mut io, inputs) = frame(Some((bytes, slot)), false, Some(operation.output_maximum));
    assert_eq!(operation.step(&mut io, &inputs), StepOutcome::Progress);
    assert!(io.test_consumed(PortId(0)));
    <FlowCollectBack as StepBack<1>>::step_committed(operation);
}

fn staged_collection(operation: &FlowCollectBack) -> StructuredInfoValue {
    let encoded = <FlowCollectBack as StepBack<1>>::prepared_output(operation, PortId(0)).unwrap();
    StructuredInfoValue::from_canonical_bytes(encoded).unwrap()
}

#[test]
fn normal_close_emits_one_ordered_sequence_then_completes() {
    let mut operation = operation(3);
    admit(&mut operation, b"one", 1);
    admit(&mut operation, b"two", 2);

    let (mut io, inputs) = frame(None, true, Some(operation.output_maximum));
    assert_eq!(operation.step(&mut io, &inputs), StepOutcome::Progress);
    assert!(io.test_consumed_closed(PortId(0)));
    <FlowCollectBack as StepBack<1>>::step_committed(&mut operation);

    let (mut io, inputs) = frame(None, false, Some(operation.output_maximum));
    assert_eq!(operation.step(&mut io, &inputs), StepOutcome::Progress);
    let value = staged_collection(&operation);
    let StructuredInfoValueShape::Collection(values) = value.shape() else {
        panic!("flow/collect output must be one bounded sequence")
    };
    assert_eq!(values.len(), 2);
    assert!(matches!(values[0].shape(), StructuredInfoValueShape::Leaf(bytes) if bytes == b"one"));
    assert!(matches!(values[1].shape(), StructuredInfoValueShape::Leaf(bytes) if bytes == b"two"));
    <FlowCollectBack as StepBack<1>>::step_committed(&mut operation);

    let (mut io, inputs) = frame(None, false, Some(operation.output_maximum));
    assert_eq!(operation.step(&mut io, &inputs), StepOutcome::Complete);
}

#[test]
fn empty_normal_close_still_emits_exactly_one_empty_sequence() {
    let mut operation = operation(2);
    let (mut io, inputs) = frame(None, true, Some(operation.output_maximum));
    assert_eq!(operation.step(&mut io, &inputs), StepOutcome::Progress);
    <FlowCollectBack as StepBack<1>>::step_committed(&mut operation);

    let (mut io, inputs) = frame(None, false, Some(operation.output_maximum));
    assert_eq!(operation.step(&mut io, &inputs), StepOutcome::Progress);
    let value = staged_collection(&operation);
    let StructuredInfoValueShape::Collection(values) = value.shape() else {
        panic!("flow/collect output must be one bounded sequence")
    };
    assert!(values.is_empty());
}

#[test]
fn value_after_exact_bound_is_the_prepared_abnormal_terminal() {
    let mut operation = operation(2);
    admit(&mut operation, b"one", 1);
    admit(&mut operation, b"two", 2);

    let (mut io, inputs) = frame(Some((b"three", 3)), false, Some(operation.output_maximum));
    assert_eq!(
        operation.step(&mut io, &inputs),
        StepOutcome::Abnormal {
            port: PortId(0),
            terminal: overflow(),
        }
    );
    assert!(io.test_consumed(PortId(0)));
}

#[test]
fn output_pressure_retains_the_prepared_result_without_reopening_input() {
    let mut operation = operation(2);
    admit(&mut operation, b"held", 1);
    let capacity = operation.allocation_capacity();

    let (mut io, inputs) = frame(None, true, Some(operation.output_maximum));
    assert_eq!(operation.step(&mut io, &inputs), StepOutcome::Progress);
    <FlowCollectBack as StepBack<1>>::step_committed(&mut operation);

    for _ in 0..3 {
        let (mut io, inputs) = frame(None, false, None);
        assert_eq!(operation.step(&mut io, &inputs), StepOutcome::Await);
        assert!(io.test_prepared_output().is_none());
        assert_eq!(operation.allocation_capacity(), capacity);
    }

    let (mut io, inputs) = frame(None, false, Some(operation.output_maximum));
    assert_eq!(operation.step(&mut io, &inputs), StepOutcome::Progress);
    assert!(io.test_prepared_output().is_some());
    assert_eq!(operation.allocation_capacity(), capacity);
}

#[test]
fn cancellation_discards_partial_and_pending_output_distinctly() {
    let mut partial = operation(2);
    admit(&mut partial, b"partial", 1);
    <FlowCollectBack as StepBack<1>>::cancel(&mut partial);
    assert_eq!(partial.count, 0);
    let (mut io, inputs) = frame(None, false, Some(partial.output_maximum));
    assert_eq!(partial.step(&mut io, &inputs), StepOutcome::Complete);
    assert!(io.test_prepared_output().is_none());

    let mut pending = operation(2);
    admit(&mut pending, b"pending", 1);
    let (mut io, inputs) = frame(None, true, Some(pending.output_maximum));
    assert_eq!(pending.step(&mut io, &inputs), StepOutcome::Progress);
    <FlowCollectBack as StepBack<1>>::step_committed(&mut pending);
    <FlowCollectBack as StepBack<1>>::cancel(&mut pending);
    let (mut io, inputs) = frame(None, false, Some(pending.output_maximum));
    assert_eq!(pending.step(&mut io, &inputs), StepOutcome::Complete);
    assert!(io.test_prepared_output().is_none());
}

#[test]
fn preparation_refuses_a_mismatched_output_envelope() {
    let contract =
        CheckedValueContract::new(conduit_core::kind_id(TEXT_INFO_ID), 8, vec![]).unwrap();
    assert!(matches!(
        FlowCollectBack::prepare(&contract, 2, 1, overflow()),
        Err(FlowCollectPreparationError::OutputContractMismatch { actual: 1, .. })
    ));
}

#[test]
fn terminal_contract_keeps_abnormal_and_cancellation_distinct() {
    let operation = operation(2);
    let contract = <FlowCollectBack as StepBack<1>>::terminal_transduction(&operation).unwrap();
    assert!(matches!(
        contract.normal_close,
        AssignedNormalCloseTransduction::FlushThenPropagate(
            AssignedFiniteTerminalEmission {
                maximum_items: 1,
                maximum_bytes,
            }
        ) if maximum_bytes == operation.output_maximum
    ));
    assert_eq!(
        contract.abnormal,
        AssignedAbnormalTransduction::PropagateAfterDrain
    );
    assert!(matches!(
        contract.cancellation,
        AssignedCancellationTransduction::DomainSpecific { .. }
    ));
}
