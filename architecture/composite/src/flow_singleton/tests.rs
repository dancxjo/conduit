use super::*;
use conduit_core::kind_id;
use conduit_kernel::ValueRef;
fn back() -> FlowSingletonBack {
    FlowSingletonBack::prepare(&StructuredInfoType::leaf(kind_id("value/u64")).unwrap(), 8).unwrap()
}
fn reference() -> ValueRef {
    ValueRef {
        slot: 0,
        generation: 1,
        byte_len: 8,
    }
}
fn commit(back: &mut FlowSingletonBack) {
    <FlowSingletonBack as StepBack<1>>::step_committed(back);
}
#[test]
fn pressure_and_rolled_back_steps_never_consume_or_commit_emission() {
    let mut back = back();
    let bytes = 7u64.to_le_bytes();
    for _ in 0..64 {
        let mut io = StepIo::test_frame([Some(reference())], [false], [None], None, 16);
        assert_eq!(
            back.step(&mut io, &StepInputBytes::test_frame([Some(&bytes)], None)),
            StepOutcome::Await
        );
        assert!(!io.test_consumed(PortId(0)));
        assert!(!back.emitted);
    }
    // First successful staging is intentionally not committed: model a denied
    // atomic publication without falsely recording the singleton as emitted.
    let mut first = StepIo::test_frame([Some(reference())], [false], [Some(8)], None, 16);
    assert_eq!(
        back.step(
            &mut first,
            &StepInputBytes::test_frame([Some(&bytes)], None)
        ),
        StepOutcome::Progress
    );
    assert!(!back.emitted);
    let mut retry = StepIo::test_frame([Some(reference())], [false], [Some(8)], None, 16);
    assert_eq!(
        back.step(
            &mut retry,
            &StepInputBytes::test_frame([Some(&bytes)], None)
        ),
        StepOutcome::Progress
    );
    assert!(retry.test_consumed(PortId(0)));
    commit(&mut back);
    assert!(back.emitted);
    let mut close = StepIo::test_frame([None], [false], [Some(8)], None, 16);
    assert_eq!(
        back.step(&mut close, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Complete
    );
}
#[test]
fn malformed_missing_and_cancelled_values_never_publish_or_mark_emitted() {
    let mut malformed = back();
    let bytes = [1u8; 7];
    let mut io = StepIo::test_frame([Some(reference())], [false], [Some(8)], None, 16);
    assert!(matches!(
        malformed.step(&mut io, &StepInputBytes::test_frame([Some(&bytes)], None)),
        StepOutcome::Fail(_)
    ));
    assert!(!io.test_consumed(PortId(0)));
    assert!(!malformed.emitted);
    let mut missing = back();
    let mut io = StepIo::test_frame([None], [true], [Some(8)], None, 16);
    assert!(matches!(
        missing.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Fail(_)
    ));
    let mut cancelled = back();
    <FlowSingletonBack as StepBack<1>>::cancel(&mut cancelled);
    let mut io = StepIo::test_frame([Some(reference())], [false], [Some(8)], None, 16);
    assert_eq!(
        cancelled.step(
            &mut io,
            &StepInputBytes::test_frame([Some(&8u64.to_le_bytes())], None)
        ),
        StepOutcome::Complete
    );
    assert!(!io.test_consumed(PortId(0)));
    assert!(!cancelled.emitted);
}
