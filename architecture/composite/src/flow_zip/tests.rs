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

#[test]
fn typed_pairing_preserves_nested_schema_under_pressure_without_play_allocation() {
    use conduit_core::{StructuredFieldType, StructuredFieldValue, StructuredInfoType};
    let leaf = StructuredInfoType::leaf(kind_id("value/count")).unwrap();
    let packet = StructuredInfoType::record(
        kind_id("type/Packet@1"),
        vec![StructuredFieldType::new("count", leaf.clone()).unwrap()],
    )
    .unwrap();
    let input = StructuredInfoValue::record(
        packet.clone(),
        vec![StructuredFieldValue::new(
            "count",
            StructuredInfoValue::leaf(leaf.clone(), conduit_core::encode_count(7).to_vec())
                .unwrap(),
        )
        .unwrap()],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let expected = packet.canonical_bytes().unwrap();
    let left = CheckedValueContract::new(
        packet.profile().unwrap().value_kind().clone(),
        input.len() as u32,
        vec![],
    )
    .unwrap();
    let right = CheckedValueContract::new(kind_id("value/count"), 8, vec![]).unwrap();
    let mut operation = FlowZipBack::prepare_typed(&left, packet, &right, leaf).unwrap();
    let count = conduit_core::encode_count(8);
    let allocations = crate::test_support::allocation::allocations_during(|| {
        for (side, bytes) in [(0, input.as_slice()), (1, count.as_slice())] {
            let mut references = [None; 2];
            let mut inputs = [None; 2];
            references[side] = Some(reference(side as u16, bytes));
            inputs[side] = Some(bytes);
            let mut io = StepIo::test_frame(references, [false; 2], [None; 2], None, 16);
            assert_eq!(
                operation.step(&mut io, &StepInputBytes::test_frame(inputs, None)),
                StepOutcome::Progress
            );
            <FlowZipBack as StepBack<2>>::step_committed(&mut operation);
        }
        for _ in 0..1000 {
            let mut io = StepIo::test_frame([None; 2], [false; 2], [None; 2], None, 16);
            assert_eq!(
                operation.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
                StepOutcome::Await
            );
        }
        let mut io = StepIo::test_frame(
            [None; 2],
            [false; 2],
            [Some(operation.output_maximum), None],
            None,
            16,
        );
        assert_eq!(
            operation.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
            StepOutcome::Progress
        );
        let bytes = <FlowZipBack as StepBack<2>>::prepared_output(&operation, PortId(0)).unwrap();
        let view = conduit_core::validate_canonical_structured_value(bytes).unwrap();
        let nested = view.record_field("item-00000").unwrap().unwrap();
        assert_eq!(nested.type_bytes(), expected);
        assert_eq!(
            nested
                .record_field("count")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/count")
                .unwrap(),
            conduit_core::encode_count(7)
        );
        <FlowZipBack as StepBack<2>>::step_committed(&mut operation);
        assert!(operation.left_len.is_none());
        assert!(operation.right_len.is_none());
    });
    assert_eq!(allocations, 0);
}

#[test]
fn typed_pair_preparation_requires_exact_member_kind_contracts() {
    use conduit_core::StructuredInfoType;
    let text = CheckedValueContract::new(kind_id(TEXT_INFO_ID), 16, vec![]).unwrap();
    let count = CheckedValueContract::new(kind_id("value/count"), 8, vec![]).unwrap();
    assert!(matches!(
        FlowZipBack::prepare_typed(
            &text,
            StructuredInfoType::leaf(kind_id("value/count")).unwrap(),
            &count,
            StructuredInfoType::leaf(kind_id("value/count")).unwrap()
        ),
        Err(conduit_core::StructuredInfoRefusal::WrongType)
    ));
}

#[test]
fn finite_typed_pair_close_flush_is_bounded_under_pressure_without_allocation() {
    let ty = conduit_core::StructuredInfoType::leaf(kind_id("value/count")).unwrap();
    let contract = CheckedValueContract::new(kind_id("value/count"), 8, vec![]).unwrap();
    let mut operation =
        FlowZipBack::prepare_typed_finite(&contract, ty.clone(), &contract, ty.clone()).unwrap();
    let left = conduit_core::encode_count(7);
    let right = conduit_core::encode_count(8);
    let mut encoder =
        conduit_core::PreparedTypedTuplePairEncoder::new(ty.clone(), 8, ty, 8).unwrap();
    let expected = encoder.encode(&left, &right).unwrap().to_vec();
    assert_eq!(
        <FlowZipBack as StepBack<2>>::terminal_transductions(&operation)[0]
            .unwrap()
            .abnormal,
        AssignedAbnormalTransduction::NotAccepted
    );
    let allocations = crate::test_support::allocation::allocations_during(|| {
        for (side, bytes) in [(0, left.as_slice()), (1, right.as_slice())] {
            let mut references = [None; 2];
            let mut inputs = [None; 2];
            references[side] = Some(reference(side as u16, bytes));
            inputs[side] = Some(bytes);
            let mut io = StepIo::test_frame(references, [false; 2], [None; 2], None, 16);
            assert_eq!(
                operation.step(&mut io, &StepInputBytes::test_frame(inputs, None)),
                StepOutcome::Progress
            );
            <FlowZipBack as StepBack<2>>::step_committed(&mut operation);
        }
        for _ in 0..1000 {
            let mut io = StepIo::test_frame([None; 2], [true, false], [None; 2], None, 16);
            assert_eq!(
                operation.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
                StepOutcome::Await
            );
            assert!(!io.test_consumed_closed(PortId(0)));
        }
        let mut io = StepIo::test_frame(
            [None; 2],
            [true, false],
            [Some(operation.output_maximum), None],
            None,
            16,
        );
        assert_eq!(
            operation.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
            StepOutcome::Progress
        );
        assert!(io.test_consumed_closed(PortId(0)));
        assert_eq!(
            <FlowZipBack as StepBack<2>>::prepared_output(&operation, PortId(0)),
            Some(expected.as_slice())
        );
        <FlowZipBack as StepBack<2>>::step_committed(&mut operation);
        let mut io = StepIo::test_frame([None; 2], [false; 2], [None; 2], None, 16);
        assert_eq!(
            operation.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
            StepOutcome::Complete
        );
        assert!(<FlowZipBack as StepBack<2>>::prepared_output(&operation, PortId(0)).is_none());
    });
    assert_eq!(allocations, 0);
}

#[test]
fn finite_close_retires_queued_unmatched_values_one_per_step() {
    let ty = StructuredInfoType::leaf(kind_id("value/count")).unwrap();
    let contract = CheckedValueContract::new(kind_id("value/count"), 8, vec![]).unwrap();
    let mut operation =
        FlowZipBack::prepare_typed_finite(&contract, ty.clone(), &contract, ty).unwrap();
    let bytes = conduit_core::encode_count(1);
    for closing in [true, false] {
        let mut io = StepIo::test_frame(
            [Some(reference(0, &bytes)), None],
            [false, closing],
            [None; 2],
            None,
            16,
        );
        assert_eq!(
            operation.step(
                &mut io,
                &StepInputBytes::test_frame([Some(&bytes), None], None)
            ),
            StepOutcome::Progress
        );
        assert!(io.test_consumed(PortId(0)));
        assert_eq!(io.test_consumed_closed(PortId(1)), closing);
        assert!(<FlowZipBack as StepBack<2>>::prepared_output(&operation, PortId(0)).is_none());
        <FlowZipBack as StepBack<2>>::step_committed(&mut operation);
    }
    let mut io = StepIo::test_frame([None; 2], [false; 2], [None; 2], None, 16);
    assert_eq!(
        operation.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
        StepOutcome::Complete
    );
}

#[test]
fn empty_feedback_close_waits_for_initial_state_without_growth() {
    let ty = StructuredInfoType::leaf(kind_id("value/count")).unwrap();
    let contract = CheckedValueContract::new(kind_id("value/count"), 8, vec![]).unwrap();
    let mut operation =
        FlowZipBack::prepare_typed_feedback(&contract, ty.clone(), &contract, ty).unwrap();
    let bytes = conduit_core::encode_count(1);
    let allocations = crate::test_support::allocation::allocations_during(|| {
        // An empty event stream still waits for its Source initialization.
        let mut io = StepIo::test_frame([None; 2], [false, true], [None; 2], None, 16);
        assert_eq!(
            operation.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
            StepOutcome::Progress
        );
        assert!(io.test_consumed_closed(PortId(1)));
        <FlowZipBack as StepBack<2>>::step_committed(&mut operation);
        for _ in 0..1000 {
            let mut io = StepIo::test_frame([None; 2], [false; 2], [None; 2], None, 16);
            assert_eq!(
                operation.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
                StepOutcome::Await
            );
        }
        let mut io = StepIo::test_frame(
            [Some(reference(0, &bytes)), None],
            [false; 2],
            [None; 2],
            None,
            16,
        );
        assert_eq!(
            operation.step(
                &mut io,
                &StepInputBytes::test_frame([Some(&bytes), None], None)
            ),
            StepOutcome::Progress
        );
        <FlowZipBack as StepBack<2>>::step_committed(&mut operation);
        let mut io = StepIo::test_frame([None; 2], [false; 2], [None; 2], None, 16);
        assert_eq!(
            operation.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
            StepOutcome::Complete
        );
        assert!(<FlowZipBack as StepBack<2>>::prepared_output(&operation, PortId(0)).is_none());
    });
    assert_eq!(allocations, 0);
}
