use super::*;
use conduit_kernel::{scheduler::StepInputBytes, HostCallOutcome};
fn value(slot: u16, len: u32) -> ValueRef {
    ValueRef {
        slot,
        generation: 1,
        byte_len: len,
    }
}
fn run(back: &mut MeasurementWindowStep, io: &mut StepIo<2>) -> StepOutcome {
    back.step(io, &StepInputBytes::test_frame([None, None], None))
}
fn commit(back: &mut MeasurementWindowStep) {
    <MeasurementWindowStep as StepBack<2>>::step_committed(back)
}
fn empty(id: RequestId) -> Option<(RequestId, HostCallOutcome)> {
    Some((
        id,
        HostCallOutcome {
            disposition: HostCallDisposition::Completed,
            output: None,
            failure: None,
        },
    ))
}
#[test]
fn staged_requests_and_completions_require_commit_and_final_output_pressure_is_lossless() {
    let mut back =
        MeasurementWindowStep::new(value(9, 32), HostCallId(0), HostCallId(1), [100, 100, 200]);
    let make = || {
        StepIo::test_frame(
            [Some(value(0, 20)), None],
            [false, false],
            [None, None],
            None,
            16,
        )
    };
    let mut first = make();
    assert_eq!(run(&mut back, &mut first), StepOutcome::Progress);
    let request = first.test_host_request().unwrap();
    // Failed I/O commit must leave the same pending transition reproducible.
    let mut retry = make();
    assert_eq!(run(&mut back, &mut retry), StepOutcome::Progress);
    assert_eq!(retry.test_host_request(), Some(request));
    commit(&mut back);
    let mut completion = StepIo::test_frame(
        [None, None],
        [false, false],
        [None, None],
        empty(RequestId(0)),
        16,
    );
    assert_eq!(run(&mut back, &mut completion), StepOutcome::Progress);
    assert!(back.take_committed_request().is_none());
    commit(&mut back);
    assert_eq!(back.take_committed_request(), Some(RequestId(0)));
    let mut close_profile = StepIo::test_frame([None, None], [true, false], [None, None], None, 16);
    assert_eq!(run(&mut back, &mut close_profile), StepOutcome::Progress);
    commit(&mut back);
    let mut close_samples = StepIo::test_frame([None, None], [false, true], [None, None], None, 16);
    assert_eq!(run(&mut back, &mut close_samples), StepOutcome::Progress);
    assert_eq!(close_samples.test_host_request().unwrap().0, RequestId(1));
    commit(&mut back);
    let outcome = Some((
        RequestId(1),
        HostCallOutcome {
            disposition: HostCallDisposition::Completed,
            output: Some(BoundedValueRef::new(value(3, 120), 200).unwrap()),
            failure: None,
        },
    ));
    let mut blocked = StepIo::test_frame([None, None], [false, false], [None, None], outcome, 16);
    assert_eq!(run(&mut back, &mut blocked), StepOutcome::Await);
    assert!(!blocked.test_host_completion_consumed());
    commit(&mut back);
    assert!(back.take_committed_request().is_none());
    let mut available =
        StepIo::test_frame([None, None], [false, false], [Some(200), None], outcome, 16);
    assert_eq!(run(&mut back, &mut available), StepOutcome::Complete);
    assert_eq!(available.test_output(PortId(0)), Some(value(3, 120)));
    assert!(back.take_committed_request().is_none());
    commit(&mut back);
    assert_eq!(back.take_committed_request(), Some(RequestId(1)));
}
#[test]
fn mismatched_and_late_cancelled_completions_do_not_commit() {
    let mut back =
        MeasurementWindowStep::new(value(9, 32), HostCallId(0), HostCallId(1), [100, 100, 200]);
    let mut first = StepIo::test_frame(
        [Some(value(0, 20)), None],
        [false, false],
        [None, None],
        None,
        16,
    );
    run(&mut back, &mut first);
    commit(&mut back);
    let mut wrong = StepIo::test_frame(
        [None, None],
        [false, false],
        [None, None],
        empty(RequestId(99)),
        16,
    );
    assert!(matches!(run(&mut back, &mut wrong), StepOutcome::Fail(_)));
    assert!(back.take_committed_request().is_none());
    <MeasurementWindowStep as StepBack<2>>::cancel(&mut back);
    let mut late = StepIo::test_frame(
        [None, None],
        [false, false],
        [None, None],
        empty(RequestId(0)),
        16,
    );
    assert!(matches!(run(&mut back, &mut late), StepOutcome::Fail(_)));
    assert!(!late.test_host_completion_consumed());
}
