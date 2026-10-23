#![cfg(feature = "kernel-step")]
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId, ValueRef,
};
use conduit_time::TickTriggerBack;
fn input() -> ValueRef {
    ValueRef {
        slot: 0,
        generation: 1,
        byte_len: 8,
    }
}
#[test]
fn occurrence_has_no_payload_and_pressure_preserves_the_input() {
    let mut back = TickTriggerBack::default();
    let bytes = 42_u64.to_le_bytes();
    let mut io = StepIo::test_frame([Some(input())], [false], [None], None, 8);
    assert_eq!(
        back.step(&mut io, &StepInputBytes::test_frame([Some(&bytes)], None)),
        StepOutcome::Await
    );
    assert!(!io.test_consumed(PortId(0)));
    let mut io = StepIo::test_frame([Some(input())], [false], [Some(0)], None, 8);
    assert_eq!(
        back.step(&mut io, &StepInputBytes::test_frame([Some(&bytes)], None)),
        StepOutcome::Progress
    );
    assert!(io.test_consumed(PortId(0)));
    assert_eq!(io.test_canonical_output().unwrap().1.as_slice(), &[]);
    let mut io = StepIo::test_frame([None], [true], [Some(0)], None, 8);
    assert_eq!(
        back.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Complete
    );
    assert!(io.test_consumed_closed(PortId(0)));
}
#[test]
fn malformed_tick_and_cancelled_occurrence_are_distinct() {
    let mut back = TickTriggerBack::default();
    let mut io = StepIo::test_frame([Some(input())], [false], [Some(0)], None, 8);
    assert_eq!(
        back.step(&mut io, &StepInputBytes::test_frame([Some(&[0; 7])], None)),
        StepOutcome::Fail(Failure {
            code: FailureCode::InvalidInput,
            detail: 2
        })
    );
    <TickTriggerBack as StepBack<1>>::cancel(&mut back);
    assert_eq!(
        back.step(&mut io, &StepInputBytes::test_frame([Some(&[0; 8])], None)),
        StepOutcome::Fail(Failure {
            code: FailureCode::Cancelled,
            detail: 1
        })
    );
}
