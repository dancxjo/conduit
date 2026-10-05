use super::*;
use conduit_core::kind_id;
use conduit_kernel::ValueRef;
fn back() -> FlowConcatFiniteBack {
    FlowConcatFiniteBack::prepare(&StructuredInfoType::leaf(kind_id("value/u64")).unwrap(), 8)
        .unwrap()
}
fn reference(slot: u16) -> ValueRef {
    ValueRef {
        slot,
        generation: 1,
        byte_len: 8,
    }
}
fn commit(back: &mut FlowConcatFiniteBack) {
    <FlowConcatFiniteBack as StepBack<2>>::step_committed(back);
}
#[test]
fn simultaneous_inputs_keep_right_queued_while_left_remains_open() {
    let mut back = back();
    let left = 1_u64.to_le_bytes();
    let right = 2_u64.to_le_bytes();
    for expected in [0, 0, 0, 0] {
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
            <FlowConcatFiniteBack as StepBack<2>>::prepared_output(&back, PortId(0)).unwrap(),
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
    <FlowConcatFiniteBack as StepBack<2>>::cancel(&mut back);
    assert_eq!(
        back.step(
            &mut io,
            &StepInputBytes::test_frame([Some(&[1]), None], None)
        ),
        StepOutcome::Complete
    );
    assert!(<FlowConcatFiniteBack as StepBack<2>>::prepared_output(&back, PortId(0)).is_none());
}

#[test]
fn left_close_commits_before_any_right_payload() {
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
    // Left closure must commit before the queued right payload is consumed.
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
    assert!(<FlowConcatFiniteBack as StepBack<2>>::prepared_output(&back, PortId(0)).is_none());
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
fn a_right_value_cannot_overtake_an_open_but_temporarily_empty_left() {
    let mut back = back();
    let bytes = 99_u64.to_le_bytes();
    let allocations = crate::test_support::allocation::allocations_during(|| {
        for _ in 0..10_000 {
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
                StepOutcome::Await
            );
            assert!(!io.test_consumed(PortId(1)));
            assert!(
                <FlowConcatFiniteBack as StepBack<2>>::prepared_output(&back, PortId(0)).is_none()
            );
        }
    });
    assert_eq!(allocations, 0);
}
