use super::*;
use conduit_core::kind_id;
use conduit_kernel::ValueRef;

fn operation() -> FlowZipExactBack {
    let ty = StructuredInfoType::leaf(kind_id("value/count")).unwrap();
    let contract = CheckedValueContract::new(kind_id("value/count"), 8, vec![]).unwrap();
    FlowZipExactBack::prepare(&contract, ty.clone(), &contract, ty).unwrap()
}
fn step(
    back: &mut FlowZipExactBack,
    values: [Option<&[u8]>; 2],
    closed: [bool; 2],
    ready: bool,
) -> StepOutcome {
    let refs = core::array::from_fn(|side| {
        values[side].map(|bytes| ValueRef {
            slot: side as u16,
            generation: 1,
            byte_len: bytes.len() as u32,
        })
    });
    let mut io = StepIo::test_frame(refs, closed, [ready.then_some(1024), None], None, 16);
    let result = back.step(&mut io, &StepInputBytes::test_frame(values, None));
    if matches!(result, StepOutcome::Progress | StepOutcome::Complete) {
        <FlowZipExactBack as StepBack<2>>::step_committed(back);
    }
    result
}
#[test]
fn exact_pair_survives_either_early_close_and_output_pressure_without_allocating() {
    for early in 0..2 {
        let mut back = operation();
        let left = conduit_core::encode_count(7);
        let right = conduit_core::encode_count(8);
        let ty = StructuredInfoType::leaf(kind_id("value/count")).unwrap();
        let mut encoder = PreparedTypedTuplePairEncoder::new(ty.clone(), 8, ty, 8).unwrap();
        let expected = encoder.encode(&left, &right).unwrap().to_vec();
        let allocations = crate::test_support::allocation::allocations_during(|| {
            let mut values = [None; 2];
            values[early] = Some(if early == 0 {
                left.as_slice()
            } else {
                right.as_slice()
            });
            assert_eq!(
                step(&mut back, values, [false; 2], false),
                StepOutcome::Progress
            );
            let mut closed = [false; 2];
            closed[early] = true;
            assert_eq!(
                step(&mut back, [None; 2], closed, false),
                StepOutcome::Progress
            );
            assert_eq!(
                step(&mut back, [None; 2], [false; 2], false),
                StepOutcome::Await
            );
            let mut values = [None; 2];
            values[1 - early] = Some(if early == 0 {
                right.as_slice()
            } else {
                left.as_slice()
            });
            assert_eq!(
                step(&mut back, values, [false; 2], false),
                StepOutcome::Progress
            );
            for _ in 0..3 {
                assert_eq!(
                    step(&mut back, [None; 2], [false; 2], false),
                    StepOutcome::Await
                );
            }
            // Do not commit yet: inspect the staged exact canonical pair.
            let mut io = StepIo::test_frame([None; 2], [false; 2], [Some(1024), None], None, 16);
            assert_eq!(
                back.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
                StepOutcome::Progress
            );
            let bytes =
                <FlowZipExactBack as StepBack<2>>::prepared_output(&back, PortId(0)).unwrap();
            assert_eq!(bytes, expected.as_slice());
            <FlowZipExactBack as StepBack<2>>::step_committed(&mut back);
            let mut closed = [false; 2];
            closed[1 - early] = true;
            assert_eq!(
                step(&mut back, [None; 2], closed, true),
                StepOutcome::Complete
            );
            assert_eq!(
                step(&mut back, [None; 2], [false; 2], true),
                StepOutcome::Complete
            );
        });
        assert_eq!(allocations, 0);
    }
}
#[test]
fn exact_pair_refuses_unequal_lengths_and_cancellation_hides_output() {
    for exhausted in 0..2 {
        let mut back = operation();
        let mut closed = [false; 2];
        closed[exhausted] = true;
        assert_eq!(
            step(&mut back, [None; 2], closed, true),
            StepOutcome::Progress
        );
        let value = conduit_core::encode_count(9);
        let mut values = [None; 2];
        values[1 - exhausted] = Some(value.as_slice());
        assert_eq!(
            step(&mut back, values, [false; 2], true),
            StepOutcome::Progress
        );
        assert!(matches!(
            step(&mut back, [None; 2], [false; 2], true),
            StepOutcome::Fail(Failure {
                code: FailureCode::InvalidInput,
                detail: 918
            })
        ));
    }
    let mut back = operation();
    assert_eq!(
        step(&mut back, [None; 2], [true, false], true),
        StepOutcome::Progress
    );
    assert_eq!(
        step(&mut back, [None; 2], [false, true], true),
        StepOutcome::Complete
    );
    assert_eq!(
        step(&mut back, [None; 2], [false; 2], true),
        StepOutcome::Complete
    );
    <FlowZipExactBack as StepBack<2>>::cancel(&mut back);
    assert!(<FlowZipExactBack as StepBack<2>>::prepared_output(&back, PortId(0)).is_none());
}

#[test]
fn exact_pair_abnormal_and_cancel_never_flush_a_buffered_pair() {
    let value = conduit_core::encode_count(1);
    for abnormal in [false, true] {
        let mut back = operation();
        assert_eq!(
            step(&mut back, [Some(&value), None], [false; 2], true),
            StepOutcome::Progress
        );
        assert_eq!(
            step(&mut back, [None, Some(&value)], [false; 2], true),
            StepOutcome::Progress
        );
        if abnormal {
            let terminal = conduit_kernel::CanonicalValue::new(&[1]).unwrap();
            let mut io = StepIo::test_frame([None; 2], [false; 2], [Some(1024), None], None, 16)
                .test_with_input_abnormal(PortId(0), terminal);
            assert!(matches!(
                back.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
                StepOutcome::Fail(_)
            ));
        } else {
            <FlowZipExactBack as StepBack<2>>::cancel(&mut back);
            assert_eq!(
                step(&mut back, [None; 2], [false; 2], true),
                StepOutcome::Complete
            );
        }
        assert!(<FlowZipExactBack as StepBack<2>>::prepared_output(&back, PortId(0)).is_none());
    }
}

#[test]
fn exact_pair_unequal_close_is_consumed_with_the_refusal() {
    for closing in 0..2 {
        let mut back = operation();
        let value = conduit_core::encode_count(9);
        let mut values = [None; 2];
        values[1 - closing] = Some(value.as_slice());
        assert_eq!(
            step(&mut back, values, [false; 2], true),
            StepOutcome::Progress
        );
        let mut closed = [false; 2];
        closed[closing] = true;
        let mut io = StepIo::test_frame([None; 2], closed, [Some(1024), None], None, 16);
        assert!(matches!(
            back.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
            StepOutcome::Fail(Failure {
                code: FailureCode::InvalidInput,
                detail: 918
            })
        ));
        assert!(io.test_consumed_closed(PortId(closing as u16)));
        assert!(<FlowZipExactBack as StepBack<2>>::prepared_output(&back, PortId(0)).is_none());
    }
}
