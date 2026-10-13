//! Feedback close flush/drain boundaries, with admitted fixed storage.
use super::*;

#[test]
fn formed_pair_left_close_preserves_pressure_then_drains_until_real_right_close() {
    let ty = StructuredInfoType::leaf(kind_id("value/count")).unwrap();
    let contract = CheckedValueContract::new(kind_id("value/count"), 8, vec![]).unwrap();
    let mut operation =
        FlowZipBack::prepare_typed_feedback(&contract, ty.clone(), &contract, ty.clone()).unwrap();
    let left = conduit_core::encode_count(7);
    let right = conduit_core::encode_count(8);
    let late = conduit_core::encode_count(9);
    let mut encoder = PreparedTypedTuplePairEncoder::new(ty.clone(), 8, ty, 8).unwrap();
    let expected = encoder.encode(&left, &right).unwrap().to_vec();
    let allocations = crate::test_support::allocation::allocations_during(|| {
        for side in 0..2 {
            let mut values = [None; 2];
            let mut bytes = [None; 2];
            let data = if side == 0 {
                left.as_slice()
            } else {
                right.as_slice()
            };
            values[side] = Some(reference(side as u16, data));
            bytes[side] = Some(data);
            let mut io = StepIo::test_frame(values, [false; 2], [None; 2], None, 16);
            assert_eq!(
                operation.step(&mut io, &StepInputBytes::test_frame(bytes, None)),
                StepOutcome::Progress
            );
            assert!(io.test_consumed(PortId(side as u16)));
            <FlowZipBack as StepBack<2>>::step_committed(&mut operation);
        }
        for _ in 0..8 {
            let mut io = StepIo::test_frame([None; 2], [true, false], [None; 2], None, 16);
            assert_eq!(
                operation.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
                StepOutcome::Await
            );
            assert!(!io.test_consumed_closed(PortId(0)));
            assert!(!operation.drain_right_until_close);
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
        for payload in [None, Some(late.as_slice()), None] {
            let mut io = StepIo::test_frame(
                [None, payload.map(|data| reference(2, data))],
                [false; 2],
                [Some(operation.output_maximum), None],
                None,
                16,
            );
            assert_eq!(
                operation.step(&mut io, &StepInputBytes::test_frame([None, payload], None)),
                if payload.is_some() {
                    StepOutcome::Progress
                } else {
                    StepOutcome::Await
                }
            );
            assert_eq!(io.test_consumed(PortId(1)), payload.is_some());
            assert!(<FlowZipBack as StepBack<2>>::prepared_output(&operation, PortId(0)).is_none());
            <FlowZipBack as StepBack<2>>::step_committed(&mut operation);
        }
        let mut io = StepIo::test_frame([None; 2], [false, true], [None; 2], None, 16);
        assert_eq!(
            operation.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
            StepOutcome::Progress
        );
        assert!(io.test_consumed_closed(PortId(1)));
        <FlowZipBack as StepBack<2>>::step_committed(&mut operation);
        let mut io = StepIo::test_frame([None; 2], [false; 2], [None; 2], None, 16);
        assert_eq!(
            operation.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
            StepOutcome::Complete
        );
    });
    assert_eq!(allocations, 0);
}

#[test]
fn right_abnormal_during_left_first_drain_is_failure_not_normal_completion() {
    let ty = StructuredInfoType::leaf(kind_id("value/count")).unwrap();
    let contract = CheckedValueContract::new(kind_id("value/count"), 8, vec![]).unwrap();
    let mut operation =
        FlowZipBack::prepare_typed_feedback(&contract, ty.clone(), &contract, ty).unwrap();
    let terminal = conduit_kernel::CanonicalValue::new(&[1]).unwrap();
    let allocations = crate::test_support::allocation::allocations_during(|| {
        let mut io = StepIo::test_frame([None; 2], [true, false], [None; 2], None, 16);
        assert_eq!(
            operation.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
            StepOutcome::Progress
        );
        <FlowZipBack as StepBack<2>>::step_committed(&mut operation);
        let mut io = StepIo::test_frame([None; 2], [false; 2], [None; 2], None, 16)
            .test_with_input_abnormal(PortId(1), terminal);
        assert_eq!(
            operation.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
            StepOutcome::Fail(Failure {
                code: FailureCode::InvalidInput,
                detail: 917
            })
        );
        assert!(!io.test_consumed_closed(PortId(1)));
        assert!(!operation.right_closed);
        assert!(!operation.terminal);
        assert!(<FlowZipBack as StepBack<2>>::prepared_output(&operation, PortId(0)).is_none());
    });
    assert_eq!(allocations, 0);
}
