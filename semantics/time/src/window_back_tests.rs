use super::*;
use alloc::vec;
use conduit_core::{
    CheckedValueContract, StructuredInfoValue, StructuredInfoValueShape, TEXT_INFO_ID,
};
use conduit_kernel::{HostCallOutcome, ValueRef};

fn value(slot: u16, bytes: &[u8]) -> ValueRef {
    ValueRef {
        slot,
        generation: 1,
        byte_len: bytes.len() as u32,
    }
}

fn completion(disposition: HostCallDisposition) -> (RequestId, HostCallOutcome) {
    (
        RequestId(1),
        HostCallOutcome {
            disposition,
            output: None,
            failure: None,
        },
    )
}

fn operation(maximum_items: u16) -> ProcessingTimeWindowBack {
    let value_contract =
        CheckedValueContract::new(conduit_core::kind_id(TEXT_INFO_ID), 8, vec![]).unwrap();
    let output_maximum =
        PreparedLeafSequenceEncoder::new(value_contract.value_kind.clone(), 8, maximum_items)
            .unwrap()
            .maximum_bytes();
    ProcessingTimeWindowBack::prepare(
        value(90, &[0; 8]),
        &value_contract,
        output_maximum,
        maximum_items,
    )
    .unwrap()
}

fn frame(
    input: Option<(&[u8], u16)>,
    closed: bool,
    completion: Option<(RequestId, HostCallOutcome)>,
    output_maximum: u32,
) -> (StepIo<1>, StepInputBytes<'_, 1>) {
    (
        StepIo::test_frame(
            [input.map(|(bytes, slot)| value(slot, bytes))],
            [closed],
            [Some(output_maximum)],
            completion,
            16,
        ),
        StepInputBytes::test_frame([input.map(|(bytes, _)| bytes)], None),
    )
}

#[test]
fn boundary_emits_admission_order_and_wins_a_simultaneous_arrival_tie() {
    let mut operation = operation(3);
    for (bytes, slot) in [(b"one".as_slice(), 1), (b"two".as_slice(), 2)] {
        let (mut io, inputs) = frame(Some((bytes, slot)), false, None, operation.output_maximum);
        assert_eq!(operation.step(&mut io, &inputs), StepOutcome::Progress);
        assert!(io.test_consumed(PortId(0)));
        if slot == 1 {
            assert!(io.test_host_request().is_some());
        } else {
            assert!(io.test_host_request().is_none());
        }
        <ProcessingTimeWindowBack as StepBack<1>>::step_committed(&mut operation);
    }

    let (mut io, inputs) = frame(
        Some((b"next", 3)),
        false,
        Some(completion(HostCallDisposition::Completed)),
        operation.output_maximum,
    );
    assert_eq!(operation.step(&mut io, &inputs), StepOutcome::Progress);
    assert!(!io.test_consumed(PortId(0)));
    assert!(io.test_host_completion_consumed());
    assert!(io.test_prepared_output().is_some());
    let encoded =
        <ProcessingTimeWindowBack as StepBack<1>>::prepared_output(&operation, PortId(0)).unwrap();
    let window = StructuredInfoValue::from_canonical_bytes(encoded).unwrap();
    let StructuredInfoValueShape::Collection(values) = window.shape() else {
        panic!("window output must be one bounded sequence")
    };
    assert!(matches!(values[0].shape(), StructuredInfoValueShape::Leaf(bytes) if bytes == b"one"));
    assert!(matches!(values[1].shape(), StructuredInfoValueShape::Leaf(bytes) if bytes == b"two"));
    <ProcessingTimeWindowBack as StepBack<1>>::step_committed(&mut operation);
    assert_eq!(operation.count, 0);
}

#[test]
fn a_full_window_backpressures_without_eviction() {
    let mut operation = operation(2);
    for (bytes, slot) in [(b"one".as_slice(), 1), (b"two".as_slice(), 2)] {
        let (mut io, inputs) = frame(Some((bytes, slot)), false, None, operation.output_maximum);
        assert_eq!(operation.step(&mut io, &inputs), StepOutcome::Progress);
        <ProcessingTimeWindowBack as StepBack<1>>::step_committed(&mut operation);
    }
    let (mut io, inputs) = frame(Some((b"three", 3)), false, None, operation.output_maximum);
    assert_eq!(operation.step(&mut io, &inputs), StepOutcome::Await);
    assert!(!io.test_consumed(PortId(0)));
    assert_eq!(operation.count, 2);
}

#[test]
fn normal_close_cancels_the_boundary_flushes_once_and_then_completes() {
    let mut operation = operation(2);
    let (mut io, inputs) = frame(Some((b"last", 1)), false, None, operation.output_maximum);
    assert_eq!(operation.step(&mut io, &inputs), StepOutcome::Progress);
    <ProcessingTimeWindowBack as StepBack<1>>::step_committed(&mut operation);

    let (mut io, inputs) = frame(None, true, None, operation.output_maximum);
    assert_eq!(operation.step(&mut io, &inputs), StepOutcome::Progress);
    assert!(io.test_consumed_closed(PortId(0)));
    assert_eq!(io.test_host_cancellation(), Some(RequestId(1)));

    let (mut io, inputs) = frame(
        None,
        false,
        Some(completion(HostCallDisposition::Cancelled)),
        operation.output_maximum,
    );
    assert_eq!(operation.step(&mut io, &inputs), StepOutcome::Progress);
    assert!(io.test_prepared_output().is_some());
    <ProcessingTimeWindowBack as StepBack<1>>::step_committed(&mut operation);

    let (mut io, inputs) = frame(None, false, None, operation.output_maximum);
    assert_eq!(operation.step(&mut io, &inputs), StepOutcome::Complete);
    assert!(io
        .test_discards()
        .iter()
        .flatten()
        .any(|value| value.slot == 90));
}
