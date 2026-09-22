use super::{DelayBack, ThrottleBack};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
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

fn input_step<O: StepBack<1>>(
    operation: &mut O,
    value: ValueRef,
    output_ready: bool,
) -> (StepOutcome, StepIo<1>) {
    let mut io = StepIo::test_frame([Some(value)], [false], [output_ready.then_some(8)], None, 8);
    let outcome = operation.step(&mut io, &StepInputBytes::test_frame([None], None));
    (outcome, io)
}

#[test]
fn delay_retains_finite_values_and_drains_them_in_order_after_close() {
    let mut operation = DelayBack {
        durations: vec![value(10, 8), value(11, 8)],
        values: Vec::with_capacity(2),
        terminal_releases: Vec::with_capacity(2),
        next_request: 0,
        next_value: 0,
        maximum_values: 2,
        pending: None,
        accepted_values: 0,
        closing: false,
        continue_after_emit: false,
    };
    let first = value(1, 1);
    let second = value(2, 1);
    let (outcome, io) = input_step(&mut operation, first, true);
    assert_eq!(outcome, StepOutcome::Progress);
    assert!(io.test_retained(PortId(0)));
    assert_eq!(
        io.test_host_request().map(|request| request.0),
        Some(RequestId(1))
    );
    let (outcome, io) = input_step(&mut operation, second, true);
    assert_eq!(outcome, StepOutcome::Progress);
    assert!(io.test_retained(PortId(0)));
    let mut io = StepIo::test_frame([None], [true], [None], None, 8);
    assert_eq!(
        operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Progress
    );
    let mut io = StepIo::test_frame(
        [None],
        [false],
        [Some(8)],
        Some(completion(1, HostCallDisposition::Completed)),
        8,
    );
    assert_eq!(
        operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Progress
    );
    assert_eq!(io.test_output(PortId(0)), Some(first));
    assert_eq!(
        io.test_host_request().map(|request| request.0),
        Some(RequestId(2))
    );
    let mut io = StepIo::test_frame(
        [None],
        [false],
        [Some(8)],
        Some(completion(2, HostCallDisposition::Completed)),
        8,
    );
    assert_eq!(
        operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Complete
    );
    assert_eq!(io.test_output(PortId(0)), Some(second));
}

#[test]
fn leading_throttle_drops_during_interval_and_cancels_exact_timer_on_close() {
    let mut operation = ThrottleBack {
        durations: vec![value(10, 8), value(11, 8)],
        terminal_releases: Vec::with_capacity(2),
        next_request: 0,
        maximum_values: 2,
        accepted_values: 0,
        pending: None,
        cancellation: None,
        arm_after_emit: false,
        closing: false,
    };
    let (outcome, io) = input_step(&mut operation, value(1, 1), true);
    assert_eq!(outcome, StepOutcome::Progress);
    assert_eq!(io.test_output(PortId(0)), Some(value(1, 1)));
    assert_eq!(
        io.test_host_request().map(|request| request.0),
        Some(RequestId(1))
    );
    let (outcome, io) = input_step(&mut operation, value(2, 1), true);
    assert_eq!(outcome, StepOutcome::Progress);
    assert!(io.test_consumed(PortId(0)));
    assert!(io.test_output(PortId(0)).is_none());
    let mut io = StepIo::test_frame([None], [true], [None], None, 8);
    assert_eq!(
        operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Progress
    );
    assert_eq!(io.test_host_cancellation(), Some(RequestId(1)));
    let mut io = StepIo::test_frame(
        [None],
        [false],
        [None],
        Some(completion(1, HostCallDisposition::Cancelled)),
        8,
    );
    assert_eq!(
        operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Complete
    );
    assert!(io.test_discards().contains(&Some(value(11, 8))));
}

#[test]
fn leading_throttle_reopens_only_after_correlated_completion() {
    let mut operation = ThrottleBack {
        durations: vec![value(10, 8), value(11, 8)],
        terminal_releases: Vec::with_capacity(2),
        next_request: 0,
        maximum_values: 2,
        accepted_values: 0,
        pending: None,
        cancellation: None,
        arm_after_emit: false,
        closing: false,
    };
    let first = value(1, 1);
    let second = value(2, 1);
    let (outcome, io) = input_step(&mut operation, first, true);
    assert_eq!(outcome, StepOutcome::Progress);
    assert_eq!(io.test_output(PortId(0)), Some(first));
    let mut io = StepIo::test_frame(
        [None],
        [false],
        [Some(8)],
        Some(completion(1, HostCallDisposition::Completed)),
        8,
    );
    assert_eq!(
        operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Progress
    );
    let (outcome, io) = input_step(&mut operation, second, true);
    assert_eq!(outcome, StepOutcome::Progress);
    assert_eq!(io.test_output(PortId(0)), Some(second));
}
