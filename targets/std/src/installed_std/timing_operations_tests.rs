use super::{DebounceOperation, TimeoutOperation};
use conduit_kernel::{
    scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome},
    HostCallDisposition, HostCallOutcome, PortId, RequestId, ValueRef,
};

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
    output_ready: bool,
) -> StepIo<1> {
    StepIo::test_frame(
        [input],
        [closed],
        [output_ready.then_some(8)],
        completion,
        8,
    )
}

fn debounce() -> DebounceOperation {
    DebounceOperation {
        durations: vec![value(10, 8), value(11, 8), value(12, 8)],
        next_request: 0,
        maximum_values: 3,
        accepted_values: 0,
        pending: None,
        cancellation: None,
        candidate: None,
        terminal_releases: Vec::with_capacity(8),
        closing: false,
        complete_after_emit: false,
    }
}

fn timeout() -> TimeoutOperation {
    TimeoutOperation {
        durations: vec![value(20, 8), value(21, 8), value(22, 8)],
        false_values: vec![value(30, 1), value(31, 1), value(32, 1)],
        true_values: vec![value(40, 1), value(41, 1)],
        next_request: 0,
        next_false: 0,
        next_true: 0,
        maximum_values: 2,
        accepted_values: 0,
        pending: None,
        cancellation: None,
        terminal_releases: Vec::with_capacity(6),
        timed_out: false,
        closing: false,
        arm_after_emit: false,
    }
}

#[test]
fn debounce_burst_resets_exact_request_and_flushes_pending_value_on_close() {
    let mut operation = debounce();
    let first = value(1, 1);
    let last = value(2, 1);
    let mut io = frame(Some(first), false, None, true);
    assert_eq!(
        operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Progress
    );
    assert!(io.test_retained(PortId(0)));
    assert_eq!(
        io.test_host_request().map(|request| request.0),
        Some(RequestId(1))
    );
    let mut io = frame(Some(last), false, None, true);
    assert_eq!(
        operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Progress
    );
    assert!(io.test_retained(PortId(0)));
    assert!(io.test_discards().contains(&Some(first)));
    assert_eq!(io.test_host_cancellation(), Some(RequestId(1)));
    let mut io = frame(
        None,
        false,
        Some(completion(1, HostCallDisposition::Cancelled)),
        true,
    );
    assert_eq!(
        operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Progress
    );
    assert_eq!(
        io.test_host_request().map(|request| request.0),
        Some(RequestId(2))
    );
    let mut io = frame(None, true, None, true);
    assert_eq!(
        operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Progress
    );
    assert_eq!(io.test_host_cancellation(), Some(RequestId(2)));
    assert!(io.test_discards().contains(&Some(value(12, 8))));
    let mut io = frame(
        None,
        false,
        Some(completion(2, HostCallDisposition::Cancelled)),
        true,
    );
    assert_eq!(
        operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Complete
    );
    assert_eq!(io.test_output(PortId(0)), Some(last));
}

#[test]
fn timeout_distinguishes_expiry_recovery_reset_and_terminal_cancellation() {
    let mut operation = timeout();
    let mut io = frame(None, false, None, true);
    assert_eq!(
        operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Progress
    );
    assert_eq!(io.test_output(PortId(0)), Some(value(30, 1)));
    assert_eq!(
        io.test_host_request().map(|request| request.0),
        Some(RequestId(1))
    );
    let mut io = frame(
        None,
        false,
        Some(completion(1, HostCallDisposition::Completed)),
        true,
    );
    assert_eq!(
        operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Progress
    );
    assert_eq!(io.test_output(PortId(0)), Some(value(40, 1)));
    let mut io = frame(Some(value(50, 8)), false, None, true);
    assert_eq!(
        operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Progress
    );
    assert_eq!(io.test_output(PortId(0)), Some(value(31, 1)));
    assert_eq!(
        io.test_host_request().map(|request| request.0),
        Some(RequestId(2))
    );
    let mut released = Vec::new();
    let mut io = frame(None, true, None, false);
    assert_eq!(
        operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Progress
    );
    assert_eq!(io.test_host_cancellation(), Some(RequestId(2)));
    released.extend(io.test_discards().iter().flatten().copied());
    let mut io = frame(
        None,
        false,
        Some(completion(2, HostCallDisposition::Cancelled)),
        false,
    );
    assert_eq!(
        operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Progress
    );
    released.extend(io.test_discards().iter().flatten().copied());
    let mut io = frame(None, false, None, false);
    assert_eq!(
        operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Complete
    );
    released.extend(io.test_discards().iter().flatten().copied());
    released.sort_by_key(|value| value.slot);
    assert_eq!(released, vec![value(22, 8), value(32, 1), value(41, 1)]);
}
