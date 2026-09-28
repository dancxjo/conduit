use super::*;
use alloc::vec;
use conduit_kernel::{HostCallOutcome, ValueRef};

fn value(slot: u16, byte_len: u32) -> ValueRef {
    ValueRef {
        slot,
        generation: 1,
        byte_len,
    }
}

fn completion(request: u32, disposition: HostCallDisposition) -> (RequestId, HostCallOutcome) {
    (
        RequestId(request),
        HostCallOutcome {
            disposition,
            output: None,
            failure: None,
        },
    )
}

fn frame(
    input: Option<ValueRef>,
    closed: bool,
    completion: Option<(RequestId, HostCallOutcome)>,
) -> StepIo<1> {
    StepIo::test_frame([input], [closed], [Some(1)], completion, 12)
}

fn operation() -> TrailingDebounceBack {
    TrailingDebounceBack::from_prepared_durations(vec![value(10, 8), value(11, 8)], 2).unwrap()
}

#[test]
fn trailing_replacement_rearms_and_close_flushes_exact_last_value() {
    let mut op = operation();
    let first = value(1, 1);
    let last = value(2, 1);
    let mut io = frame(Some(first), false, None);
    assert_eq!(
        op.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Progress
    );
    let mut io = frame(Some(last), false, None);
    assert_eq!(
        op.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Progress
    );
    assert_eq!(io.test_host_cancellation(), Some(RequestId(1)));
    assert!(io.test_discards().contains(&Some(first)));
    let mut io = frame(
        None,
        false,
        Some(completion(1, HostCallDisposition::Cancelled)),
    );
    assert_eq!(
        op.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Progress
    );
    assert_eq!(io.test_host_request().map(|r| r.0), Some(RequestId(2)));
    let mut io = frame(None, true, None);
    assert_eq!(
        op.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Progress
    );
    let mut io = frame(
        None,
        false,
        Some(completion(2, HostCallDisposition::Cancelled)),
    );
    assert_eq!(
        op.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Complete
    );
    assert_eq!(io.test_output(PortId(0)), Some(last));
}

#[test]
fn input_wins_simultaneous_completion_and_cancel_race() {
    let mut op = operation();
    let mut io = frame(Some(value(1, 1)), false, None);
    let _ = op.step(&mut io, &StepInputBytes::test_frame([None], None));
    let mut io = frame(
        Some(value(2, 1)),
        false,
        Some(completion(1, HostCallDisposition::Completed)),
    );
    assert_eq!(
        op.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Progress
    );
    assert_eq!(io.test_output(PortId(0)), None);
    assert_eq!(io.test_host_request().map(|r| r.0), Some(RequestId(2)));

    let mut op = operation();
    let mut io = frame(Some(value(1, 1)), false, None);
    let _ = op.step(&mut io, &StepInputBytes::test_frame([None], None));
    let mut io = frame(Some(value(2, 1)), false, None);
    let _ = op.step(&mut io, &StepInputBytes::test_frame([None], None));
    let mut io = frame(
        None,
        false,
        Some(completion(1, HostCallDisposition::Completed)),
    );
    assert_eq!(
        op.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Progress
    );
    assert_eq!(io.test_output(PortId(0)), None);
    assert_eq!(io.test_host_request().map(|r| r.0), Some(RequestId(2)));
}

#[test]
fn finite_pressure_malformed_completion_and_cancel_remain_distinct() {
    let mut op = operation();
    op.maximum_values = 1;
    let mut io = frame(Some(value(1, 1)), false, None);
    assert_eq!(
        op.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Progress
    );
    let mut io = frame(Some(value(2, 1)), false, None);
    assert_eq!(
        op.step(&mut io, &StepInputBytes::test_frame([None], None)),
        failure(FailureCode::WorkBudgetExhausted, 52)
    );

    let mut io = frame(
        None,
        false,
        Some(completion(9, HostCallDisposition::Completed)),
    );
    assert_eq!(
        op.step(&mut io, &StepInputBytes::test_frame([None], None)),
        fail(53)
    );

    <TrailingDebounceBack as StepBack<1>>::cancel(&mut op);
    assert_eq!(op.pending, None);
    assert_eq!(op.candidate, None);
}

#[test]
fn preparation_and_unused_cleanup_are_finite() {
    assert!(matches!(
        TrailingDebounceBack::from_prepared_durations(Vec::new(), 0),
        Err(DebouncePreparationError::Empty)
    ));
    assert!(matches!(
        TrailingDebounceBack::from_prepared_durations(vec![value(10, 8)], 2),
        Err(DebouncePreparationError::InsufficientDurations)
    ));

    let mut op = TrailingDebounceBack::from_prepared_durations(
        vec![value(10, 8), value(11, 8), value(12, 8)],
        3,
    )
    .unwrap();
    let mut io = frame(None, true, None);
    assert_eq!(
        op.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Progress
    );
    assert_eq!(io.test_discards().iter().flatten().count(), 1);
    let mut io = frame(None, false, None);
    assert_eq!(
        op.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Progress
    );
    let mut io = frame(None, false, None);
    assert_eq!(
        op.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Progress
    );
    let mut io = frame(None, false, None);
    assert_eq!(
        op.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Complete
    );
}
