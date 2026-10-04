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
            StepOutcome::Progress
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
