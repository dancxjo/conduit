use super::*;
use conduit_core::kind_id;
use conduit_kernel::ValueRef;
fn back() -> FlowMergeFiniteBack {
    FlowMergeFiniteBack::prepare(&StructuredInfoType::leaf(kind_id("value/u64")).unwrap(), 8)
        .unwrap()
}
fn reference(slot: u16) -> ValueRef {
    ValueRef {
        slot,
        generation: 1,
        byte_len: 8,
    }
}
fn commit(back: &mut FlowMergeFiniteBack) {
    <FlowMergeFiniteBack as StepBack<2>>::step_committed(back);
}
#[test]
fn simultaneous_inputs_alternate_without_reordering_each_stream() {
    let mut back = back();
    let left = 1_u64.to_le_bytes();
    let right = 2_u64.to_le_bytes();
    for expected in [0, 1, 0, 1] {
        let mut io = StepIo::test_frame(
            [Some(reference(0)), Some(reference(1))],
            [false; 2],
            [Some(8), None],
            None,
            16,
        );
        assert_eq!(
            back.step(
                &mut io,
                &StepInputBytes::test_frame([Some(&left), Some(&right)], None)
            ),
            StepOutcome::Progress
        );
        assert!(io.test_consumed(PortId(expected)));
        assert!(!io.test_consumed(PortId(1 - expected)));
        assert_eq!(
            <FlowMergeFiniteBack as StepBack<2>>::prepared_output(&back, PortId(0)).unwrap(),
            if expected == 0 { &left } else { &right }
        );
        commit(&mut back);
    }
}
#[test]
fn pressure_consumes_nothing_and_one_closed_input_does_not_retire_the_other() {
    let mut back = back();
    let bytes = 7_u64.to_le_bytes();
    let allocations = crate::test_support::allocation::allocations_during(|| {
        for _ in 0..1000 {
            let mut io =
                StepIo::test_frame([Some(reference(0)), None], [false; 2], [None; 2], None, 16);
            assert_eq!(
                back.step(
                    &mut io,
                    &StepInputBytes::test_frame([Some(&bytes), None], None)
                ),
                StepOutcome::Await
            );
            assert!(!io.test_consumed(PortId(0)));
        }
        let mut io = StepIo::test_frame([None; 2], [true, false], [None; 2], None, 16);
        assert_eq!(
            back.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
            StepOutcome::Progress
        );
        assert!(io.test_consumed_closed(PortId(0)));
        commit(&mut back);
        let mut io = StepIo::test_frame(
            [None, Some(reference(1))],
            [false; 2],
            [Some(8), None],
            None,
            16,
        );
        assert_eq!(
            back.step(
                &mut io,
                &StepInputBytes::test_frame([None, Some(&bytes)], None)
            ),
            StepOutcome::Progress
        );
        assert!(io.test_consumed(PortId(1)));
        commit(&mut back);
        let mut io = StepIo::test_frame([None; 2], [false, true], [Some(8), None], None, 16);
        assert_eq!(
            back.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
            StepOutcome::Complete
        );
        commit(&mut back);
        let mut io = StepIo::test_frame([None; 2], [false; 2], [Some(8), None], None, 16);
        assert_eq!(
            back.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
            StepOutcome::Complete
        );
    });
    assert_eq!(allocations, 0);
}
#[test]
fn malformed_input_and_cancelled_work_never_publish_values() {
    let mut back = back();
    let mut io = StepIo::test_frame(
        [Some(reference(0)), None],
        [false; 2],
        [Some(8), None],
        None,
        16,
    );
    assert!(matches!(
        back.step(
            &mut io,
            &StepInputBytes::test_frame([Some(&[1]), None], None)
        ),
        StepOutcome::Fail(_)
    ));
    assert!(!io.test_consumed(PortId(0)));
    <FlowMergeFiniteBack as StepBack<2>>::cancel(&mut back);
    assert_eq!(
        back.step(
            &mut io,
            &StepInputBytes::test_frame([Some(&[1]), None], None)
        ),
        StepOutcome::Complete
    );
    assert!(<FlowMergeFiniteBack as StepBack<2>>::prepared_output(&back, PortId(0)).is_none());
}

#[test]
fn exposed_close_precedes_the_other_inputs_preferred_payload() {
    let mut back = back();
    let bytes = 1_u64.to_le_bytes();
    let mut first = StepIo::test_frame(
        [Some(reference(0)), None],
        [false; 2],
        [Some(8), None],
        None,
        16,
    );
    assert_eq!(
        back.step(
            &mut first,
            &StepInputBytes::test_frame([Some(&bytes), None], None)
        ),
        StepOutcome::Progress
    );
    commit(&mut back);
    // Right is now preferred, but left's exposed closure cannot be deferred.
    let mut closed = StepIo::test_frame(
        [None, Some(reference(1))],
        [true, false],
        [Some(8), None],
        None,
        16,
    );
    assert_eq!(
        back.step(
            &mut closed,
            &StepInputBytes::test_frame([None, Some(&bytes)], None)
        ),
        StepOutcome::Progress
    );
    assert!(closed.test_consumed_closed(PortId(0)));
    assert!(!closed.test_consumed(PortId(1)));
    assert!(<FlowMergeFiniteBack as StepBack<2>>::prepared_output(&back, PortId(0)).is_none());
    commit(&mut back);
    let mut next = StepIo::test_frame(
        [None, Some(reference(1))],
        [false; 2],
        [Some(8), None],
        None,
        16,
    );
    assert_eq!(
        back.step(
            &mut next,
            &StepInputBytes::test_frame([None, Some(&bytes)], None)
        ),
        StepOutcome::Progress
    );
    assert!(next.test_consumed(PortId(1)));
}

#[test]
fn static_schema_merge_preserves_validation_pressure_and_exact_output() {
    use conduit_core::{StructuredFieldType, StructuredFieldValue, StructuredInfoValue};
    let leaf = StructuredInfoType::leaf(kind_id("value/bool")).unwrap();
    let schema = StructuredInfoType::record(
        kind_id("test/static-merge"),
        vec![StructuredFieldType::new("flag", leaf.clone()).unwrap()],
    )
    .unwrap();
    let canonical = Box::leak(schema.canonical_bytes().unwrap().into_boxed_slice());
    let good = StructuredInfoValue::record(
        schema.clone(),
        vec![StructuredFieldValue::new(
            "flag",
            StructuredInfoValue::leaf(leaf.clone(), vec![1]).unwrap(),
        )
        .unwrap()],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let mut bad_leaf = good.clone();
    *bad_leaf.last_mut().unwrap() = 2;
    let oversized = vec![0; 513];
    let mut trailing = good.clone();
    trailing.push(0);
    let mut wrong = good.clone();
    wrong[0] ^= 1;
    assert!(FlowMergeFiniteBack::prepare_static(&leaf, canonical, 512).is_err());
    for input in [
        &good[..],
        &bad_leaf,
        &oversized,
        &trailing,
        &wrong,
        &good[..good.len() - 1],
    ] {
        for ready in [false, true] {
            let mut owned = FlowMergeFiniteBack::prepare(&schema, 512).unwrap();
            let mut linked = FlowMergeFiniteBack::prepare_static(&schema, canonical, 512).unwrap();
            let value = ValueRef {
                slot: 0,
                generation: 1,
                byte_len: input.len() as u32,
            };
            let capacity = if ready { Some(512) } else { None };
            let mut a =
                StepIo::test_frame([Some(value), None], [false; 2], [capacity, None], None, 16);
            let mut b =
                StepIo::test_frame([Some(value), None], [false; 2], [capacity, None], None, 16);
            let allocations = crate::test_support::allocation::allocations_during(|| {
                let bytes = StepInputBytes::test_frame([Some(input), None], None);
                assert_eq!(owned.step(&mut a, &bytes), linked.step(&mut b, &bytes));
                assert_eq!(a.test_consumed(PortId(0)), b.test_consumed(PortId(0)));
                assert_eq!(
                    <FlowMergeFiniteBack as StepBack<2>>::prepared_output(&owned, PortId(0)),
                    <FlowMergeFiniteBack as StepBack<2>>::prepared_output(&linked, PortId(0))
                );
                commit(&mut owned);
                commit(&mut linked);
            });
            assert_eq!(allocations, 0);
        }
    }
}
